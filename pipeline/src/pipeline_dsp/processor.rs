//! Processeur de trame unique et accumulateur de trames MFCC.
//!
//! `FrameProcessor` : PreEmphasis → HannWindow → FFT → MelFilterbank → log\_mel → DCT → \[f32;13\]
//! `MfccAccumulator` : fenêtre glissante de 98 trames → matrice `[[f32;13];98]`

use std::collections::VecDeque;

use crate::pipeline_dsp::config::DspConfig;
use crate::pipeline_dsp::error::DspError;
use crate::pipeline_dsp::fft::VDspFft;
use crate::pipeline_dsp::mel_filterbank::MelFilterbank;
use crate::pipeline_dsp::mfcc::{log_mel_energies, MfccExtractor};
use crate::pipeline_dsp::preemphasis::PreEmphasis;
use crate::pipeline_dsp::windowing::HannWindow;

// ---------------------------------------------------------------------------
// P9.1 — FrameProcessor
// ---------------------------------------------------------------------------

/// Processeur de trame : agrège toutes les étapes DSP en un seul objet.
///
/// Ordre d'exécution dans `process_frame` :
/// 1. Pré-accentuation (`PreEmphasis`)
/// 2. Fenêtrage de Hann (`HannWindow`)
/// 3. FFT réelle → magnitudes (`VDspFft`)
/// 4. Filtres Mel (`MelFilterbank`)
/// 5. Logarithme naturel (`log_mel_energies`)
/// 6. DCT-II → 13 coefficients MFCC (`MfccExtractor`)
///
/// Tous les buffers intermédiaires sont pré-alloués dans `new` — zéro allocation en régime permanent.
pub struct FrameProcessor {
    preemphasis: PreEmphasis,
    window: HannWindow,
    fft: VDspFft,
    mel: MelFilterbank,
    mfcc: MfccExtractor,
    /// Taille d'une trame en échantillons.
    frame_size: usize,
    // Buffers pré-alloués — réutilisés à chaque appel de process_frame()
    buf_magnitudes: Vec<f32>,
    buf_mel: Vec<f32>,
}

impl FrameProcessor {
    /// Construit un `FrameProcessor` à partir de la configuration DSP.
    pub fn new(config: &DspConfig) -> Result<Self, DspError> {
        Ok(Self {
            preemphasis: PreEmphasis::new(config.alpha),
            window: HannWindow::new(config.frame_size),
            fft: VDspFft::new(config.n_fft)?,
            mel: MelFilterbank::new(config),
            mfcc: MfccExtractor::new(config)?,
            frame_size: config.frame_size,
            buf_magnitudes: vec![0.0f32; config.n_fft / 2],
            buf_mel: vec![0.0f32; config.n_mels],
        })
    }

    /// Traite une trame audio et retourne 13 coefficients MFCC.
    ///
    /// La trame est modifiée in-place (pré-accentuation, fenêtrage).
    /// Zéro allocation — tous les buffers intermédiaires sont pré-alloués dans `new`.
    ///
    /// # Panics
    /// Panique si `frame.len() != frame_size`.
    pub fn process_frame(&mut self, frame: &mut [f32]) -> [f32; 13] {
        assert_eq!(
            frame.len(),
            self.frame_size,
            "FrameProcessor::process_frame: frame.len()={} != frame_size={}",
            frame.len(),
            self.frame_size
        );

        // 1. Pré-accentuation
        self.preemphasis.apply(frame);

        // 2. Fenêtrage de Hann
        self.window.apply(frame);

        // 3. FFT → magnitudes (zéro allocation via forward_into)
        self.fft.forward_into(frame, &mut self.buf_magnitudes);

        // 4. Filtres Mel (zéro allocation via apply_into)
        self.mel.apply_into(&self.buf_magnitudes, &mut self.buf_mel);

        // 5. Logarithme naturel in-place
        log_mel_energies(&mut self.buf_mel);

        // 6. DCT-II → 13 MFCC (zéro allocation)
        self.mfcc.extract(&self.buf_mel)
    }
}

// ---------------------------------------------------------------------------
// P9.2 — MfccAccumulator
// ---------------------------------------------------------------------------

/// Fenêtre glissante de trames MFCC.
///
/// Accumule jusqu'à `capacity` trames de 13 coefficients.
/// Quand la capacité est atteinte, chaque nouveau `push` évince la trame la plus ancienne.
pub struct MfccAccumulator {
    frames: VecDeque<[f32; 13]>,
    capacity: usize,
}

impl MfccAccumulator {
    /// Crée un accumulateur vide de capacité `capacity` (typiquement 98 trames).
    pub fn new(capacity: usize) -> Self {
        Self {
            frames: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Ajoute une trame MFCC ; si l'accumulateur est plein, évince la plus ancienne.
    pub fn push(&mut self, mfcc: [f32; 13]) {
        if self.frames.len() == self.capacity {
            self.frames.pop_front();
        }
        self.frames.push_back(mfcc);
    }

    /// Retourne `true` si l'accumulateur contient exactement `capacity` trames.
    pub fn is_ready(&self) -> bool {
        self.frames.len() == self.capacity
    }

    /// Retourne la matrice courante `[[f32;13];98]` (row-major, contiguë).
    ///
    /// # Panics
    /// Panique si `!is_ready()`.
    pub fn get_matrix(&self) -> [[f32; 13]; 98] {
        assert!(
            self.is_ready(),
            "MfccAccumulator::get_matrix: pas encore prêt"
        );
        let mut matrix = [[0.0f32; 13]; 98];
        for (i, frame) in self.frames.iter().enumerate() {
            matrix[i] = *frame;
        }
        matrix
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{FrameProcessor, MfccAccumulator};
    use crate::pipeline_dsp::config::DspConfig;

    // --- P9.1 ---

    #[test]
    fn frame_processor_new_default_succeeds() {
        let cfg = DspConfig::default();
        let fp = FrameProcessor::new(&cfg);
        assert!(
            fp.is_ok(),
            "FrameProcessor::new doit réussir avec la config par défaut"
        );
    }

    #[test]
    fn silence_frame_gives_finite_mfcc() {
        // Un signal silencieux → magnitudes ≈ 0 → log_mel ≈ ln(EPSILON) → MFCC finis
        let cfg = DspConfig::default();
        let mut fp = FrameProcessor::new(&cfg).unwrap();
        let mut frame = vec![0.0f32; cfg.frame_size];
        let mfcc = fp.process_frame(&mut frame);

        for (k, &v) in mfcc.iter().enumerate() {
            assert!(
                v.is_finite(),
                "MFCC[{}] = {} n'est pas fini (signal silence)",
                k,
                v
            );
        }

        // La valeur attendue : toutes les énergies Mel ≈ ln(EPSILON)
        let expected = f32::EPSILON.ln();
        // Le coefficient DCT[0] ≈ n_mels * ln(EPSILON) (somme non normalisée).
        // On vérifie que MFCC[0] est proche d'une valeur fortement négative.
        assert!(
            mfcc[0] < -100.0,
            "MFCC[0]={} devrait être << 0 pour un signal silence (ln(EPSILON)≈{})",
            mfcc[0],
            expected
        );
    }

    #[test]
    fn same_frame_twice_same_result() {
        // Deux trames identiques (stateless pour cette propriété) → résultats égaux.
        // Note : la pré-accentuation a un état `last_sample` ;
        // on réinitialise le processeur entre les deux appels.
        let cfg = DspConfig::default();

        let mut frame1 = vec![0.5f32; cfg.frame_size];
        let mut frame2 = vec![0.5f32; cfg.frame_size];

        let mut fp1 = FrameProcessor::new(&cfg).unwrap();
        let mut fp2 = FrameProcessor::new(&cfg).unwrap();

        let mfcc1 = fp1.process_frame(&mut frame1);
        let mfcc2 = fp2.process_frame(&mut frame2);

        for k in 0..13 {
            assert!(
                (mfcc1[k] - mfcc2[k]).abs() < 1e-5,
                "MFCC[{}] : {} vs {} (doivent être identiques)",
                k,
                mfcc1[k],
                mfcc2[k]
            );
        }
    }

    // --- P9.2 ---

    #[test]
    fn accumulator_97_frames_not_ready() {
        let mut acc = MfccAccumulator::new(98);
        let frame = [0.0f32; 13];
        for _ in 0..97 {
            acc.push(frame);
        }
        assert!(!acc.is_ready(), "97 trames → is_ready() doit être false");
    }

    #[test]
    fn accumulator_98_frames_ready() {
        let mut acc = MfccAccumulator::new(98);
        let frame = [0.0f32; 13];
        for _ in 0..98 {
            acc.push(frame);
        }
        assert!(acc.is_ready(), "98 trames → is_ready() doit être true");
    }

    #[test]
    fn accumulator_sliding_window_evicts_oldest() {
        let mut acc = MfccAccumulator::new(98);

        // Remplir avec des trames identifiables : trame i a coefficient[0] = i as f32
        for i in 0..98 {
            let mut frame = [0.0f32; 13];
            frame[0] = i as f32;
            acc.push(frame);
        }
        assert!(acc.is_ready());

        // Pousser une 99ème trame → la trame 0 doit être évincée
        let mut frame99 = [0.0f32; 13];
        frame99[0] = 99.0;
        acc.push(frame99);

        let matrix = acc.get_matrix();
        // La première trame de la matrice doit maintenant avoir coefficient[0] = 1.0
        assert!(
            (matrix[0][0] - 1.0).abs() < 1e-6,
            "matrix[0][0]={} (attendu 1.0 après éviction de la trame 0)",
            matrix[0][0]
        );
        // La dernière trame doit avoir coefficient[0] = 99.0
        assert!(
            (matrix[97][0] - 99.0).abs() < 1e-6,
            "matrix[97][0]={} (attendu 99.0)",
            matrix[97][0]
        );
    }

    #[test]
    fn get_matrix_is_contiguous_row_major() {
        let mut acc = MfccAccumulator::new(98);
        for i in 0..98 {
            let mut frame = [0.0f32; 13];
            frame[0] = i as f32;
            acc.push(frame);
        }
        let matrix = acc.get_matrix();

        // La matrice est un tableau Rust [[f32;13];98] → contiguë row-major par définition.
        // Vérification : les éléments sont accessibles via pointeur brut.
        let ptr = matrix.as_ptr() as *const f32;
        for i in 0..98usize {
            let val = unsafe { *ptr.add(i * 13) };
            assert!(
                (val - i as f32).abs() < 1e-6,
                "matrix[{}][0] via pointeur = {} (attendu {})",
                i,
                val,
                i
            );
        }
    }
}
