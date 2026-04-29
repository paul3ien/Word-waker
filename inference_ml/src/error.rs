use std::fmt;

/// Erreurs possibles du module `inference_ml`.
#[derive(Debug)]
pub enum InferenceError {
    /// Le chemin du modèle est introuvable ou vide.
    ModelNotFound(String),
    /// Échec du chargement du modèle Core ML.
    LoadFailed(String),
    /// Le handle Core ML retourné par le bridge est null.
    NullHandle,
    /// La forme de l'entrée ne correspond pas à ce qu'attend le modèle.
    InvalidInputShape {
        expected: (usize, usize, usize, usize),
        got: usize,
    },
    /// Erreur lors de l'inférence (score hors plage, erreur bridge…).
    InferenceFailed(String),
    /// La sortie demandée est absente des features du modèle.
    OutputNotFound(String),
    /// Le channel de communication a été fermé.
    ChannelClosed,
}

impl fmt::Display for InferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InferenceError::ModelNotFound(path) => {
                write!(f, "modèle introuvable : '{path}'")
            }
            InferenceError::LoadFailed(msg) => {
                write!(f, "échec du chargement Core ML : {msg}")
            }
            InferenceError::NullHandle => {
                write!(f, "le bridge Core ML a retourné un handle null")
            }
            InferenceError::InvalidInputShape { expected, got } => {
                write!(
                    f,
                    "forme d'entrée invalide : attendu {}×{}×{}×{} = {} éléments, reçu {got}",
                    expected.0,
                    expected.1,
                    expected.2,
                    expected.3,
                    expected.0 * expected.1 * expected.2 * expected.3,
                )
            }
            InferenceError::InferenceFailed(msg) => {
                write!(f, "échec de l'inférence : {msg}")
            }
            InferenceError::OutputNotFound(name) => {
                write!(f, "sortie '{name}' absente des features du modèle")
            }
            InferenceError::ChannelClosed => {
                write!(f, "le channel de communication a été fermé")
            }
        }
    }
}

impl std::error::Error for InferenceError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_variants_display_non_empty_and_distinct() {
        let variants: Vec<InferenceError> = vec![
            InferenceError::ModelNotFound("some/path".into()),
            InferenceError::LoadFailed("timeout".into()),
            InferenceError::NullHandle,
            InferenceError::InvalidInputShape {
                expected: (1, 1, 98, 13),
                got: 42,
            },
            InferenceError::InferenceFailed("score hors plage".into()),
            InferenceError::OutputNotFound("classLabel_probs".into()),
            InferenceError::ChannelClosed,
        ];

        let mut messages = HashSet::new();
        for v in &variants {
            let msg = v.to_string();
            assert!(!msg.is_empty(), "Display vide pour {:?}", v);
            assert!(messages.insert(msg.clone()), "Message dupliqué : '{msg}'");
        }
        assert_eq!(messages.len(), 7);
    }

    #[test]
    fn inference_error_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<InferenceError>();
    }
}
