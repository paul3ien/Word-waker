use std::ffi::CString;
use std::path::Path;

use crate::config::InferenceConfig;
use crate::error::InferenceError;
use crate::ffi;

/// Wrapper Rust autour du handle CoreML.
/// Charge le modèle en mémoire et expose une méthode d'inférence synchrone.
/// Thread-safe : Send + Sync sont sûrs car le handle est opaque et
/// CoreML garantit la thread-safety pour les prédictions read-only.
pub struct CoreMLModel {    handle: ffi::CoreMLHandle,
}

// Le MLModel* ObjC est safe à envoyer entre threads et à utiliser en parallèle
// (prédictions en lecture seule).
unsafe impl Send for CoreMLModel {}
unsafe impl Sync for CoreMLModel {}

impl CoreMLModel {
    /// Charge le `.mlmodelc` pointé par `config.model_path`.
    ///
    /// Retourne `Err(ModelNotFound)` si le chemin n'existe pas,
    /// `Err(NullHandle)` si CoreML échoue à charger le modèle.
    pub fn load(config: &InferenceConfig) -> Result<Self, InferenceError> {
        // Vérification précoce du chemin pour un message d'erreur clair.
        if !Path::new(&config.model_path).exists() {
            return Err(InferenceError::ModelNotFound(config.model_path.clone()));
        }

        let cpath = CString::new(config.model_path.as_str())
            .map_err(|e| InferenceError::LoadFailed(e.to_string()))?;

        let handle = unsafe { ffi::coreml_load(cpath.as_ptr()) };

        if handle.is_null() {
            return Err(InferenceError::NullHandle);
        }

        Ok(CoreMLModel { handle })
    }

    /// Lance une inférence synchrone sur la matrice MFCC fournie.
    ///
    /// `mfcc` : 98 trames × 13 coefficients.
    /// Retourne le score wake-word dans [0.0, 1.0].
    pub fn infer(&self, mfcc: &[[f32; 13]; 98]) -> Result<f32, InferenceError> {
        // Aplatir la matrice en slice contigu.
        let flat: Vec<f32> = mfcc.iter().flatten().copied().collect();

        let score = unsafe { ffi::coreml_infer(self.handle, flat.as_ptr(), flat.len()) };

        if !(0.0f32..=1.0f32).contains(&score) {
            return Err(InferenceError::InferenceFailed(format!(
                "score hors [0.0, 1.0] : {score}"
            )));
        }

        Ok(score)
    }
}

impl Drop for CoreMLModel {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { ffi::coreml_free(self.handle) };
        }
    }
}

// ─── Tests unitaires ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// CoreMLModel doit être Send + Sync (vérification statique).
    #[test]
    fn coreml_model_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<CoreMLModel>();
    }

    /// Un chemin vide dans la config doit retourner ModelNotFound
    /// (Path::exists retourne false sur une chaîne vide).
    #[test]
    fn load_empty_path_returns_model_not_found() {
        let config = InferenceConfig {
            model_path: String::new(),
            ..Default::default()
        };
        let err = CoreMLModel::load(&config).err().unwrap();
        assert!(
            matches!(err, InferenceError::ModelNotFound(_)),
            "attendu ModelNotFound, obtenu {err}"
        );
    }

    /// Un chemin inexistant doit retourner ModelNotFound.
    #[test]
    fn load_nonexistent_path_returns_model_not_found() {
        let config = InferenceConfig {
            model_path: "/tmp/nonexistent_model.mlmodelc".into(),
            ..Default::default()
        };
        let err = CoreMLModel::load(&config).err().unwrap();
        assert!(
            matches!(err, InferenceError::ModelNotFound(_)),
            "attendu ModelNotFound, obtenu {err}"
        );
    }
}
