//! Configuration du pipeline DSP.

use crate::pipeline_dsp::error::DspError;

/// Paramètres de configuration du pipeline DSP.
///
/// Utilisez `DspConfig::default()` pour les valeurs recommandées (16 kHz, MFCC 13×98).
/// Appelez ensuite `validate()` pour vérifier la cohérence des paramètres.
#[derive(Debug, Clone, PartialEq)]
pub struct DspConfig {
    /// Taux d'échantillonnage du signal d'entrée en Hz.
    pub sample_rate: f64,
    /// Nombre de samples par trame (fenêtre d'analyse).
    pub frame_size: usize,
    /// Décalage entre deux trames consécutives en samples.
    pub hop_size: usize,
    /// Taille de la FFT (doit être une puissance de 2 et ≥ `frame_size`).
    pub n_fft: usize,
    /// Nombre de filtres Mel dans le filterbank.
    pub n_mels: usize,
    /// Nombre de coefficients MFCC retenus (≤ `n_mels`).
    pub n_mfcc: usize,
    /// Coefficient de pré-accentuation (typiquement 0.97).
    pub alpha: f32,
    /// Fréquence minimale du filterbank Mel en Hz.
    pub mel_fmin: f32,
    /// Fréquence maximale du filterbank Mel en Hz (≤ `sample_rate / 2`).
    pub mel_fmax: f32,
    /// Nombre de trames accumulées avant d'envoyer une matrice MFCC.
    pub n_frames: usize,
}

impl Default for DspConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16_000.0,
            frame_size: 400,
            hop_size: 160,
            n_fft: 512,
            n_mels: 40,
            n_mfcc: 13,
            alpha: 0.97,
            mel_fmin: 20.0,
            mel_fmax: 8_000.0,
            n_frames: 98,
        }
    }
}

impl DspConfig {
    /// Vérifie la cohérence des paramètres.
    ///
    /// # Erreurs
    /// - `DspError::InvalidSampleRate` si `sample_rate` ≤ 0
    /// - `DspError::FftSetupFailed` si `n_fft` n'est pas une puissance de 2
    /// - `DspError::InvalidFrameSize` si `n_fft` < `frame_size`
    /// - `DspError::DctSetupFailed` si `n_mfcc` > `n_mels`
    /// - `DspError::InvalidSampleRate` si `mel_fmax` > `sample_rate / 2`
    pub fn validate(&self) -> Result<(), DspError> {
        if self.sample_rate <= 0.0 {
            return Err(DspError::InvalidSampleRate(self.sample_rate));
        }
        if !self.n_fft.is_power_of_two() {
            return Err(DspError::FftSetupFailed);
        }
        if self.n_fft < self.frame_size {
            return Err(DspError::InvalidFrameSize {
                expected: self.n_fft,
                got: self.frame_size,
            });
        }
        if self.n_mfcc > self.n_mels {
            return Err(DspError::DctSetupFailed);
        }
        if self.mel_fmax as f64 > self.sample_rate / 2.0 {
            return Err(DspError::InvalidSampleRate(self.sample_rate));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::DspConfig;

    #[test]
    fn default_validates_ok() {
        assert!(DspConfig::default().validate().is_ok());
    }

    #[test]
    fn non_power_of_two_fft_fails() {
        let cfg = DspConfig {
            n_fft: 300,
            ..DspConfig::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn n_mfcc_greater_than_n_mels_fails() {
        let cfg = DspConfig {
            n_mfcc: 50,
            n_mels: 40,
            ..DspConfig::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn mel_fmax_above_nyquist_fails() {
        let cfg = DspConfig {
            mel_fmax: 9000.0,
            sample_rate: 16_000.0,
            ..DspConfig::default()
        };
        assert!(cfg.validate().is_err());
    }
}
