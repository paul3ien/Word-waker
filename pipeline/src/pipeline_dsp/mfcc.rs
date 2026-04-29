//! Log des énergies Mel, DCT-II via vDSP, extraction des coefficients MFCC.

use libc::c_void;

use crate::pipeline_dsp::config::DspConfig;
use crate::pipeline_dsp::error::DspError;
use crate::pipeline_dsp::ffi::{vDSP_DCT_CreateSetup, vDSP_DCT_Execute, V_DSP_DCT_II};

// ---------------------------------------------------------------------------
// P8.1 — Log des énergies Mel
// ---------------------------------------------------------------------------

/// Applique le logarithme naturel in-place sur les énergies Mel.
///
/// Les valeurs ≤ 0 sont remplacées par `f32::EPSILON` avant le log pour éviter `-inf`.
pub fn log_mel_energies(mel_energies: &mut [f32]) {
    for e in mel_energies.iter_mut() {
        if *e <= 0.0 {
            *e = f32::EPSILON;
        }
        *e = e.ln();
    }
}

// ---------------------------------------------------------------------------
// P8.2 — Wrapper DCT-II
// ---------------------------------------------------------------------------

/// Retourne le plus petit entier valide pour `vDSP_DCT_CreateSetup` qui est ≥ `n`.
///
/// Les tailles valides sont de la forme `f·2^k` avec `f ∈ {1, 3, 5, 15}` et `k ≥ 4`.
fn next_valid_dct_size(n: usize) -> usize {
    const FACTORS: [usize; 4] = [1, 3, 5, 15];
    let mut best = usize::MAX;
    for &f in &FACTORS {
        let mut size = f * 16; // f * 2^4 (k_min = 4)
        while size < n {
            size *= 2;
        }
        if size < best {
            best = size;
        }
    }
    best
}

/// Wrapper pour `vDSP_DCT_CreateSetup` / `vDSP_DCT_Execute`.
///
/// Si la taille demandée `n` n'est pas directement supportée par vDSP DCT
/// (f·2^k avec f ∈ {1,3,5,15} et k ≥ 4), le setup est créé pour la prochaine
/// taille valide ≥ n et l'entrée est transparentement zero-paddée.
pub struct VDspDct {
    setup: *mut c_void,
    /// Taille logique (visible de l'appelant).
    n: usize,
    /// Taille effective utilisée par vDSP (≥ n, valide pour vDSP DCT).
    n_padded: usize,
}

unsafe impl Send for VDspDct {}

impl VDspDct {
    /// Crée un setup DCT-II pour `n` valeurs.
    ///
    /// Sélectionne automatiquement la prochaine taille valide pour vDSP si nécessaire.
    pub fn new(n: usize) -> Result<Self, DspError> {
        let n_padded = next_valid_dct_size(n);
        let setup = unsafe { vDSP_DCT_CreateSetup(std::ptr::null_mut(), n_padded, V_DSP_DCT_II) };
        if setup.is_null() {
            return Err(DspError::DctSetupFailed);
        }
        Ok(Self { setup, n, n_padded })
    }

    /// Exécute la DCT-II sur `input` (longueur `n`) et écrit le résultat dans `output` (longueur `n`).
    ///
    /// Alloue des buffers temporaires si n != n_padded. Préférer `execute_with_bufs` pour
    /// zéro allocation en régime permanent.
    ///
    /// # Panics
    /// Panique si les longueurs ne correspondent pas à `n`.
    pub fn execute(&self, input: &[f32], output: &mut [f32]) {
        assert_eq!(
            input.len(),
            self.n,
            "VDspDct::execute: input.len()={} != n={}",
            input.len(),
            self.n
        );
        assert_eq!(
            output.len(),
            self.n,
            "VDspDct::execute: output.len()={} != n={}",
            output.len(),
            self.n
        );

        if self.n == self.n_padded {
            unsafe {
                vDSP_DCT_Execute(self.setup, input.as_ptr(), output.as_mut_ptr());
            }
        } else {
            let mut padded_in = vec![0.0f32; self.n_padded];
            padded_in[..self.n].copy_from_slice(input);
            let mut padded_out = vec![0.0f32; self.n_padded];
            unsafe {
                vDSP_DCT_Execute(self.setup, padded_in.as_ptr(), padded_out.as_mut_ptr());
            }
            output.copy_from_slice(&padded_out[..self.n]);
        }
    }

    /// Exécute la DCT-II sans aucune allocation — buffers temporaires fournis par l'appelant.
    ///
    /// `tmp_in` et `tmp_out` doivent avoir une longueur ≥ `n_padded`.
    ///
    /// # Panics
    /// Panique si `input.len() != n` ou `output.len() != n`.
    pub fn execute_with_bufs(
        &self,
        input: &[f32],
        output: &mut [f32],
        tmp_in: &mut [f32],
        tmp_out: &mut [f32],
    ) {
        assert_eq!(input.len(), self.n);
        assert_eq!(output.len(), self.n);

        if self.n == self.n_padded {
            unsafe {
                vDSP_DCT_Execute(self.setup, input.as_ptr(), output.as_mut_ptr());
            }
        } else {
            debug_assert!(tmp_in.len() >= self.n_padded);
            debug_assert!(tmp_out.len() >= self.n_padded);
            tmp_in[..self.n].copy_from_slice(input);
            tmp_in[self.n..self.n_padded].fill(0.0);
            unsafe {
                vDSP_DCT_Execute(self.setup, tmp_in.as_ptr(), tmp_out.as_mut_ptr());
            }
            output.copy_from_slice(&tmp_out[..self.n]);
        }
    }
}

impl Drop for VDspDct {
    fn drop(&mut self) {
        // Note : Apple ne documente pas de fonction `vDSP_DCT_DestroySetup` dans l'API publique.
        // Les setups DCT sont des sous-types de DFT setups ; la fuite mémoire unitaire est
        // négligeable (< 1 KB par setup). À revisiter via Instruments / Leaks si nécessaire.
        let _ = self.setup;
    }
}

/// Extrait les `n_mfcc` premiers coefficients MFCC à partir des log-énergies Mel via DCT-II.
///
/// Tous les buffers intermédiaires sont pré-alloués dans `new` pour zéro allocation en régime permanent.
pub struct MfccExtractor {
    dct: VDspDct,
    /// Nombre de coefficients MFCC à retourner (≤ 13).
    n_mfcc: usize,
    /// Dimension du vecteur log-Mel en entrée (= n_mels).
    n_mels: usize,
    // Buffers pré-alloués pour execute_with_bufs
    buf_dct_out: Vec<f32>,
    buf_dct_tmp_in: Vec<f32>,
    buf_dct_tmp_out: Vec<f32>,
}

impl MfccExtractor {
    /// Construit un `MfccExtractor` à partir de la configuration DSP.
    pub fn new(config: &DspConfig) -> Result<Self, DspError> {
        let n_padded = {
            // Recalculer n_padded pour dimensionner les buffers tmp
            let n = config.n_mels;
            const FACTORS: [usize; 4] = [1, 3, 5, 15];
            let mut best = usize::MAX;
            for &f in &FACTORS {
                let mut size = f * 16;
                while size < n {
                    size *= 2;
                }
                if size < best {
                    best = size;
                }
            }
            best
        };
        let dct = VDspDct::new(config.n_mels)?;
        Ok(Self {
            dct,
            n_mfcc: config.n_mfcc,
            n_mels: config.n_mels,
            buf_dct_out: vec![0.0f32; config.n_mels],
            buf_dct_tmp_in: vec![0.0f32; n_padded],
            buf_dct_tmp_out: vec![0.0f32; n_padded],
        })
    }

    /// Exécute la DCT-II sur `log_mel` et retourne les 13 premiers coefficients MFCC.
    ///
    /// Zéro allocation — utilise les buffers pré-alloués dans `new`.
    ///
    /// # Panics
    /// Panique si `log_mel.len() != n_mels`.
    pub fn extract(&mut self, log_mel: &[f32]) -> [f32; 13] {
        assert_eq!(
            log_mel.len(),
            self.n_mels,
            "MfccExtractor::extract: log_mel.len()={} != n_mels={}",
            log_mel.len(),
            self.n_mels
        );
        self.dct.execute_with_bufs(
            log_mel,
            &mut self.buf_dct_out,
            &mut self.buf_dct_tmp_in,
            &mut self.buf_dct_tmp_out,
        );
        let mut mfcc = [0.0f32; 13];
        let n = self.n_mfcc.min(13).min(self.n_mels);
        mfcc[..n].copy_from_slice(&self.buf_dct_out[..n]);
        mfcc
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{log_mel_energies, MfccExtractor, VDspDct};
    use crate::pipeline_dsp::config::DspConfig;

    // --- P8.1 ---

    #[test]
    fn log_energy_one_gives_zero() {
        let mut energies = vec![1.0f32];
        log_mel_energies(&mut energies);
        assert!(
            (energies[0] - 0.0).abs() < 1e-6,
            "ln(1.0) = {} (attendu 0.0)",
            energies[0]
        );
    }

    #[test]
    fn log_energy_zero_no_panic_no_inf() {
        let mut energies = vec![0.0f32];
        log_mel_energies(&mut energies);
        assert!(
            energies[0].is_finite(),
            "ln de zéro doit rester fini (pas de -inf)"
        );
        let expected = f32::EPSILON.ln();
        assert!(
            (energies[0] - expected).abs() < 1e-3,
            "energies[0]={} (attendu ln(EPSILON)={})",
            energies[0],
            expected
        );
    }

    #[test]
    fn log_energy_negative_replaced_by_epsilon() {
        let mut energies = vec![-5.0f32];
        log_mel_energies(&mut energies);
        let expected = f32::EPSILON.ln();
        assert!(
            (energies[0] - expected).abs() < 1e-3,
            "énergie négative → ln(EPSILON)={}, got={}",
            expected,
            energies[0]
        );
    }

    // --- P8.2 ---

    #[test]
    fn vdsp_dct_new_40_succeeds() {
        // 40 = 5·2^3 n'est pas valide pour vDSP DCT (k < 4).
        // new() doit réussir en arrondissant à la prochaine taille valide (48 = 3·2^4).
        let dct = VDspDct::new(40);
        assert!(dct.is_ok(), "VDspDct::new(40) doit réussir");
    }

    #[test]
    fn dct_impulse_first_coefficients() {
        // Pour un signal impulsion x=[1,0,...,0] de taille N=16 (valide directement),
        // la DCT-II donne X[k] = cos(π·k / (2·N)) = cos(πk/32).
        //   X[0] = cos(0)    = 1.0
        //   X[8] = cos(π/4) ≈ 0.7071
        let dct = VDspDct::new(16).expect("VDspDct::new(16) doit réussir");
        let mut input = vec![0.0f32; 16];
        input[0] = 1.0;
        let mut output = vec![0.0f32; 16];
        dct.execute(&input, &mut output);

        assert!(
            (output[0] - 1.0).abs() < 1e-4,
            "X[0]={} (attendu 1.0)",
            output[0]
        );
        assert!(
            (output[8].abs() - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-3,
            "X[8]={} (attendu ≈ √2/2 ≈ 0.7071)",
            output[8]
        );
    }

    // --- P8.3 ---

    #[test]
    fn mfcc_extractor_new_default_succeeds() {
        let cfg = DspConfig::default();
        let ext = MfccExtractor::new(&cfg);
        assert!(
            ext.is_ok(),
            "MfccExtractor::new doit réussir avec la config par défaut"
        );
    }

    #[test]
    fn constant_log_mel_dct0_dominant() {
        // Pour un signal constant [c, ..., c] (40 valeurs),
        // X[0] = somme des éléments non-nuls = 40·c  (la composante DC).
        // Les coefficients k>0 doivent être d'amplitude plus faible que X[0].
        let cfg = DspConfig::default(); // n_mels=40, n_mfcc=13
        let mut ext = MfccExtractor::new(&cfg).unwrap();
        let log_mel = vec![1.0f32; 40];
        let mfcc = ext.extract(&log_mel);

        assert!(
            mfcc[0].abs() > 10.0,
            "MFCC[0]={} (attendu > 10 pour un signal constant unité sur 40 points)",
            mfcc[0]
        );
        for k in 1..13 {
            assert!(
                mfcc[k].abs() < mfcc[0].abs(),
                "MFCC[{}]={:.4} devrait être < |MFCC[0]|={:.4}",
                k,
                mfcc[k].abs(),
                mfcc[0].abs()
            );
        }
    }
}
