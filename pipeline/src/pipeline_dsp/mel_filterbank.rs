//! Banc de filtres Mel : matrice row-major `n_mels × (n_fft/2)`, application via `cblas_sgemv`.

use crate::pipeline_dsp::config::DspConfig;
use crate::pipeline_dsp::ffi::{cblas_sgemv, CBLAS_NO_TRANS, CBLAS_ROW_MAJOR};

// ---------------------------------------------------------------------------
// Conversions Hz ↔ Mel
// ---------------------------------------------------------------------------

/// Convertit une fréquence en Hz vers l'échelle Mel.
/// `mel = 2595 · log10(1 + hz / 700)`
pub fn hz_to_mel(hz: f32) -> f32 {
    2595.0 * (1.0 + hz / 700.0).log10()
}

/// Convertit une valeur Mel vers Hz.
/// `hz = 700 · (10^(mel / 2595) − 1)`
pub fn mel_to_hz(mel: f32) -> f32 {
    700.0 * (10.0_f32.powf(mel / 2595.0) - 1.0)
}

// ---------------------------------------------------------------------------
// MelFilterbank
// ---------------------------------------------------------------------------

/// Matrice de filtres triangulaires Mel stockée row-major (`n_mels × n_fft_bins`).
///
/// Chaque ligne correspond à un filtre triangulaire centré sur une fréquence Mel.
pub struct MelFilterbank {
    /// Coefficients de la matrice (row-major).
    pub matrix: Vec<f32>,
    /// Nombre de filtres Mel.
    pub n_mels: usize,
    /// Nombre de bins FFT utilisés (= n_fft / 2).
    pub n_fft_bins: usize,
}

impl MelFilterbank {
    /// Construit la matrice de filtres Mel à partir de la configuration DSP.
    ///
    /// Les centres sont linéairement espacés sur l'échelle Mel entre `fmin` et `fmax`.
    /// Les filtres sont normalisés à hauteur 1.0 (pas de normalisation HTK).
    pub fn new(config: &DspConfig) -> Self {
        let n_mels = config.n_mels;
        let n_fft_bins = config.n_fft / 2;
        let sr = config.sample_rate as f32;
        let fmin = config.mel_fmin;
        let fmax = config.mel_fmax;

        // n_mels + 2 points Mel linéairement espacés entre mel_fmin et mel_fmax
        let mel_min = hz_to_mel(fmin);
        let mel_max = hz_to_mel(fmax);
        let n_points = n_mels + 2;
        let mel_points: Vec<f32> = (0..n_points)
            .map(|i| mel_min + (mel_max - mel_min) * i as f32 / (n_points - 1) as f32)
            .collect();

        // Convertir les centres Mel en indices de bins FFT
        // bin = hz * n_fft / sr  → ici n_fft = 2 * n_fft_bins
        let n_fft = config.n_fft as f32;
        let bin_centers: Vec<f32> = mel_points
            .iter()
            .map(|&m| mel_to_hz(m) * n_fft / sr)
            .collect();

        // Construire la matrice row-major : n_mels × n_fft_bins
        let mut matrix = vec![0.0f32; n_mels * n_fft_bins];
        for m in 0..n_mels {
            let left = bin_centers[m];
            let center = bin_centers[m + 1];
            let right = bin_centers[m + 2];
            for k in 0..n_fft_bins {
                let fk = k as f32;
                let val = if fk >= left && fk <= center {
                    (fk - left) / (center - left).max(f32::EPSILON)
                } else if fk > center && fk <= right {
                    (right - fk) / (right - center).max(f32::EPSILON)
                } else {
                    0.0
                };
                matrix[m * n_fft_bins + k] = val;
            }
        }

        Self {
            matrix,
            n_mels,
            n_fft_bins,
        }
    }

    /// Applique le filterbank sur `magnitudes` via `cblas_sgemv`.
    ///
    /// Retourne un vecteur de `n_mels` énergies Mel.
    ///
    /// # Panics
    /// Panique si `magnitudes.len() != n_fft_bins`.
    pub fn apply(&self, magnitudes: &[f32]) -> Vec<f32> {
        assert_eq!(
            magnitudes.len(),
            self.n_fft_bins,
            "MelFilterbank::apply: magnitudes.len()={} != n_fft_bins={}",
            magnitudes.len(),
            self.n_fft_bins
        );
        let mut mel_energies = vec![0.0f32; self.n_mels];
        // y = 1.0 · matrix · magnitudes + 0.0 · y
        unsafe {
            cblas_sgemv(
                CBLAS_ROW_MAJOR,
                CBLAS_NO_TRANS,
                self.n_mels as i32,
                self.n_fft_bins as i32,
                1.0, // alpha
                self.matrix.as_ptr(),
                self.n_fft_bins as i32, // lda
                magnitudes.as_ptr(),
                1,   // incx
                0.0, // beta
                mel_energies.as_mut_ptr(),
                1, // incy
            );
        }
        mel_energies
    }
}

#[cfg(test)]
mod tests {
    use super::{hz_to_mel, mel_to_hz, MelFilterbank};
    use crate::pipeline_dsp::config::DspConfig;

    #[test]
    fn hz_to_mel_700() {
        // hz_to_mel(700) = 2595 * log10(1 + 700/700) = 2595 * log10(2) ≈ 781.17
        let mel = hz_to_mel(700.0);
        assert!(
            (mel - 781.17).abs() < 0.1,
            "hz_to_mel(700) = {} (attendu ≈ 781.17)",
            mel
        );
    }

    #[test]
    fn mel_hz_bijection() {
        let hz = 1000.0_f32;
        let roundtrip = mel_to_hz(hz_to_mel(hz));
        assert!(
            (roundtrip - hz).abs() < 1e-3,
            "mel_to_hz(hz_to_mel(1000)) = {} (attendu ≈ 1000)",
            roundtrip
        );
    }

    #[test]
    fn matrix_size() {
        let cfg = DspConfig::default(); // n_mels=40, n_fft=512 → n_fft_bins=256
        let fb = MelFilterbank::new(&cfg);
        assert_eq!(
            fb.matrix.len(),
            40 * 256,
            "matrice: {} éléments (attendu 10240)",
            fb.matrix.len()
        );
    }

    #[test]
    fn filters_non_negative_and_non_zero() {
        let cfg = DspConfig::default();
        let fb = MelFilterbank::new(&cfg);
        for m in 0..fb.n_mels {
            let row = &fb.matrix[m * fb.n_fft_bins..(m + 1) * fb.n_fft_bins];
            // Tous les coefficients sont ≥ 0
            for &v in row {
                assert!(v >= 0.0, "coefficient négatif dans le filtre {}", m);
            }
            // La somme est > 0 (le filtre couvre au moins un bin)
            let sum: f32 = row.iter().sum();
            assert!(sum > 0.0, "filtre {} a une somme nulle", m);
        }
    }

    #[test]
    fn apply_flat_spectrum_gives_positive_energies() {
        let cfg = DspConfig::default();
        let fb = MelFilterbank::new(&cfg);
        let magnitudes = vec![1.0f32; fb.n_fft_bins];
        let energies = fb.apply(&magnitudes);
        assert_eq!(energies.len(), 40);
        for (m, &e) in energies.iter().enumerate() {
            assert!(e > 0.0, "énergie Mel {} = {} (attendu > 0)", m, e);
        }
    }

    #[test]
    fn apply_zero_spectrum_gives_zero_energies() {
        let cfg = DspConfig::default();
        let fb = MelFilterbank::new(&cfg);
        let magnitudes = vec![0.0f32; fb.n_fft_bins];
        let energies = fb.apply(&magnitudes);
        for (m, &e) in energies.iter().enumerate() {
            assert!(e.abs() < 1e-10, "énergie Mel {} = {} (attendu 0)", m, e);
        }
    }
}
