use std::fmt;

/// Erreurs possibles du module de capture audio.
#[derive(Debug)]
pub enum AudioCaptureError {
    /// Aucun device d'entrée par défaut trouvé.
    DeviceNotFound,
    /// Le device ne supporte pas le format requis (Float32 mono 16 kHz).
    FormatUnsupported,
    /// Échec de création de l'AudioUnit (code d'erreur CoreAudio).
    UnitCreationFailed(i32),
    /// Échec du démarrage de l'AudioUnit.
    UnitStartFailed(i32),
    /// Échec de l'arrêt de l'AudioUnit.
    UnitStopFailed(i32),
    /// Échec de la configuration d'une propriété CoreAudio.
    PropertySetFailed(i32),
    /// Le ring buffer est plein (overflow détecté).
    RingBufferFull,
}

impl fmt::Display for AudioCaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioCaptureError::DeviceNotFound => {
                write!(f, "Aucun device d'entrée audio par défaut trouvé")
            }
            AudioCaptureError::FormatUnsupported => {
                write!(f, "Le device ne supporte pas le format Float32 mono 16 kHz")
            }
            AudioCaptureError::UnitCreationFailed(code) => {
                write!(f, "Échec de création de l'AudioUnit (OSStatus: {})", code)
            }
            AudioCaptureError::UnitStartFailed(code) => {
                write!(f, "Échec du démarrage de l'AudioUnit (OSStatus: {})", code)
            }
            AudioCaptureError::UnitStopFailed(code) => {
                write!(f, "Échec de l'arrêt de l'AudioUnit (OSStatus: {})", code)
            }
            AudioCaptureError::PropertySetFailed(code) => {
                write!(
                    f,
                    "Échec de la configuration d'une propriété CoreAudio (OSStatus: {})",
                    code
                )
            }
            AudioCaptureError::RingBufferFull => {
                write!(f, "Le ring buffer est plein — des samples ont été perdus")
            }
        }
    }
}

impl std::error::Error for AudioCaptureError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_non_vide_pour_toutes_les_variantes() {
        let variantes: &[AudioCaptureError] = &[
            AudioCaptureError::DeviceNotFound,
            AudioCaptureError::FormatUnsupported,
            AudioCaptureError::UnitCreationFailed(-1),
            AudioCaptureError::UnitStartFailed(-2),
            AudioCaptureError::UnitStopFailed(-3),
            AudioCaptureError::PropertySetFailed(-4),
            AudioCaptureError::RingBufferFull,
        ];
        for variante in variantes {
            let msg = variante.to_string();
            assert!(!msg.is_empty(), "Display vide pour {:?}", variante);
        }
    }

    #[test]
    fn audio_capture_error_est_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AudioCaptureError>();
    }
}
