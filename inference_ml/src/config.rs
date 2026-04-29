use crate::InferenceError;

/// Configuration du module d'inférence.
#[derive(Debug, Clone)]
pub struct InferenceConfig {
    /// Chemin vers le répertoire `.mlmodelc` compilé.
    pub model_path: String,
    /// Nom de la feature d'entrée dans le modèle Core ML.
    pub input_name: String,
    /// Nom de la feature de sortie dans le modèle Core ML.
    pub output_name: String,
    /// Nombre de trames MFCC (axe temporel).
    pub n_frames: usize,
    /// Nombre de coefficients MFCC par trame.
    pub n_mfcc: usize,
}

impl Default for InferenceConfig {
    fn default() -> Self {
        Self {
            model_path: String::new(),
            input_name: "mfcc_input".into(),
            output_name: "classLabel_probs".into(),
            n_frames: 98,
            n_mfcc: 13,
        }
    }
}

impl InferenceConfig {
    /// Vérifie que la configuration est cohérente.
    pub fn validate(&self) -> Result<(), InferenceError> {
        if self.model_path.is_empty() {
            return Err(InferenceError::ModelNotFound(
                "model_path est vide".into(),
            ));
        }
        if self.n_frames == 0 {
            return Err(InferenceError::InvalidInputShape {
                expected: (1, 1, 1, self.n_mfcc),
                got: 0,
            });
        }
        if self.n_mfcc == 0 {
            return Err(InferenceError::InvalidInputShape {
                expected: (1, 1, self.n_frames, 1),
                got: 0,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_config_ok() {
        let cfg = InferenceConfig {
            model_path: "fixtures/mock_model/WakeWordMock.mlmodelc".into(),
            ..InferenceConfig::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn empty_model_path_returns_model_not_found() {
        let cfg = InferenceConfig::default(); // model_path vide
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, InferenceError::ModelNotFound(_)));
    }

    #[test]
    fn n_frames_zero_returns_err() {
        let cfg = InferenceConfig {
            model_path: "some/path".into(),
            n_frames: 0,
            ..InferenceConfig::default()
        };
        assert!(cfg.validate().is_err());
    }
}
