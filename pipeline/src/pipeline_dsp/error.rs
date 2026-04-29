//! Erreurs du pipeline DSP.

use std::fmt;

/// Erreurs possibles lors du traitement du signal par le pipeline DSP.
#[derive(Debug)]
pub enum DspError {
    /// Échec d'initialisation du plan FFT Accelerate.
    FftSetupFailed,
    /// Échec d'initialisation du plan DCT Accelerate.
    DctSetupFailed,
    /// Taille de trame invalide reçue du Framer.
    InvalidFrameSize {
        /// Taille attendue en samples.
        expected: usize,
        /// Taille effectivement reçue.
        got: usize,
    },
    /// Taux d'échantillonnage invalide (doit être > 0 et ≤ 192 000 Hz).
    InvalidSampleRate(f64),
    /// Dépassement numérique détecté à une étape donnée du pipeline.
    NumericalOverflow {
        /// Nom de l'étape où le dépassement s'est produit.
        step: &'static str,
    },
    /// Le channel d'entrée a été fermé prématurément.
    ChannelClosed,
}

impl fmt::Display for DspError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DspError::FftSetupFailed => write!(f, "Échec d'initialisation du plan FFT Accelerate"),
            DspError::DctSetupFailed => write!(f, "Échec d'initialisation du plan DCT Accelerate"),
            DspError::InvalidFrameSize { expected, got } => write!(
                f,
                "Taille de trame invalide : attendu {expected} samples, reçu {got}"
            ),
            DspError::InvalidSampleRate(sr) => write!(
                f,
                "Taux d'échantillonnage invalide : {sr} Hz (doit être > 0 et ≤ 192000)"
            ),
            DspError::NumericalOverflow { step } => {
                write!(f, "Dépassement numérique à l'étape « {step} »")
            }
            DspError::ChannelClosed => {
                write!(f, "Le channel d'entrée PCM a été fermé prématurément")
            }
        }
    }
}

impl std::error::Error for DspError {}

#[cfg(test)]
mod tests {
    use super::DspError;
    use std::collections::HashSet;

    fn all_variants() -> Vec<DspError> {
        vec![
            DspError::FftSetupFailed,
            DspError::DctSetupFailed,
            DspError::InvalidFrameSize { expected: 400, got: 256 },
            DspError::InvalidSampleRate(0.0),
            DspError::NumericalOverflow { step: "mel_filterbank" },
            DspError::ChannelClosed,
        ]
    }

    #[test]
    fn display_messages_non_empty() {
        for variant in all_variants() {
            let msg = variant.to_string();
            assert!(!msg.is_empty(), "Display vide pour {:?}", variant);
        }
    }

    #[test]
    fn display_messages_distinct() {
        let msgs: HashSet<String> = all_variants().iter().map(|v| v.to_string()).collect();
        assert_eq!(
            msgs.len(),
            all_variants().len(),
            "Des variantes partagent le même message Display"
        );
    }

    #[test]
    fn dsp_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DspError>();
    }
}
