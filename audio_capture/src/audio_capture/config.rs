use crate::audio_capture::error::AudioCaptureError;

/// Paramètres de configuration du module de capture audio.
#[derive(Clone)]
pub struct AudioCaptureConfig {
    /// Fréquence d'échantillonnage en Hz (nominale : 16 000 Hz).
    pub sample_rate: f64,
    /// Taille du buffer HAL en frames (nominale : 256, doit être une puissance de 2).
    pub buffer_size_frames: u32,
    /// Capacité du ring buffer en samples (nominale : 32 000 ≈ 2 s à 16 kHz).
    pub ring_capacity: usize,
}

impl Default for AudioCaptureConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16_000.0,
            buffer_size_frames: 256,
            ring_capacity: 32_000,
        }
    }
}

impl AudioCaptureConfig {
    /// Vérifie les invariants de la configuration.
    ///
    /// Retourne `Err(AudioCaptureError::FormatUnsupported)` si un paramètre est invalide.
    pub fn validate(&self) -> Result<(), AudioCaptureError> {
        if self.sample_rate <= 0.0 {
            return Err(AudioCaptureError::FormatUnsupported);
        }
        if self.buffer_size_frames == 0 || !self.buffer_size_frames.is_power_of_two() {
            return Err(AudioCaptureError::FormatUnsupported);
        }
        if self.ring_capacity == 0 {
            return Err(AudioCaptureError::FormatUnsupported);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_valide() {
        assert!(AudioCaptureConfig::default().validate().is_ok());
    }

    #[test]
    fn sample_rate_zero_rejete() {
        let cfg = AudioCaptureConfig {
            sample_rate: 0.0,
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn sample_rate_negatif_rejete() {
        let cfg = AudioCaptureConfig {
            sample_rate: -16_000.0,
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn buffer_size_non_puissance_de_2_rejete() {
        let cfg = AudioCaptureConfig {
            buffer_size_frames: 300,
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn buffer_size_zero_rejete() {
        let cfg = AudioCaptureConfig {
            buffer_size_frames: 0,
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn ring_capacity_zero_rejete() {
        let cfg = AudioCaptureConfig {
            ring_capacity: 0,
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }
}
