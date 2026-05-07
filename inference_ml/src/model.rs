use crate::config::InferenceConfig;
use crate::error::InferenceError;
use crate::ffi;
use std::ffi::CString;
use std::path::Path;

/// Wrapper Rust autour d'un handle `MLModel` Core ML.
///
/// Charge le modèle `.mlmodelc` en mémoire via le bridge Objective-C++
/// et expose une méthode d'inférence synchrone.
///
/// # Thread-safety
///
/// `CoreMLModel` implémente `Send + Sync` car :
/// - Le handle `MLModel *` Obj-C est un pointeur opaque géré par ARC.
/// - Core ML garantit que `[MLModel predictionFromFeatures:error:]` est
///   thread-safe en lecture (plusieurs threads peuvent appeler `infer`
///   simultanément sur la même instance).
/// - `coreml_free` n'est appelé qu'une seule fois depuis `Drop` —
///   jamais depuis plusieurs threads simultanément.
pub struct CoreMLModel {
    handle: ffi::CoreMLHandle,
}

// SAFETY : voir doc-comment ci-dessus.
unsafe impl Send for CoreMLModel {}
unsafe impl Sync for CoreMLModel {}

/// Vérifie que `score` appartient à [0.0, 1.0].
/// Fonction pure extraite pour être testable indépendamment du bridge FFI.
fn check_score(score: f32) -> Result<f32, InferenceError> {
    if !(0.0f32..=1.0f32).contains(&score) {
        return Err(InferenceError::InferenceFailed(format!(
            "score hors [0.0, 1.0] : {score}"
        )));
    }
    Ok(score)
}

impl CoreMLModel {
    /// Charge le `.mlmodelc` pointé par `config.model_path`.
    ///
    /// Avec la feature `mock_model` : retourne immédiatement un stub sans
    /// appeler le bridge FFI (utile pour les tests sans modèle réel).
    ///
    /// Retourne `Err(ModelNotFound)` si le chemin n'existe pas,
    /// `Err(NullHandle)` si CoreML échoue à charger le modèle.
    pub fn load(config: &InferenceConfig) -> Result<Self, InferenceError> {
        Self::load_inner(config, false)
    }

    /// Identique à [`load`] mais force `MLComputeUnitsCPUOnly`.
    /// Utile pour les benchmarks mesurant le gain apporté par l'ANE.
    pub fn load_cpu_only(config: &InferenceConfig) -> Result<Self, InferenceError> {
        Self::load_inner(config, true)
    }

    fn load_inner(config: &InferenceConfig, cpu_only: bool) -> Result<Self, InferenceError> {
        // ── Feature mock_model : stub sans FFI ───────────────────────────────
        #[cfg(feature = "mock_model")]
        {
            let _ = config;
            let _ = cpu_only;
            return Ok(CoreMLModel {
                handle: std::ptr::null_mut(),
            });
        }

        // ── Chemin réel ───────────────────────────────────────────────────────
        #[cfg(not(feature = "mock_model"))]
        {
            if !Path::new(&config.model_path).exists() {
                return Err(InferenceError::ModelNotFound(config.model_path.clone()));
            }

            let cpath = CString::new(config.model_path.as_str())
                .map_err(|e| InferenceError::LoadFailed(e.to_string()))?;

            let handle = if cpu_only {
                unsafe { ffi::coreml_load_cpu_only(cpath.as_ptr()) }
            } else {
                unsafe { ffi::coreml_load(cpath.as_ptr()) }
            };

            if handle.is_null() {
                return Err(InferenceError::NullHandle);
            }

            Ok(CoreMLModel { handle })
        }
    }

    /// Lance une inférence synchrone sur la matrice MFCC fournie.
    ///
    /// `mfcc` : 98 trames × 13 coefficients.
    /// Retourne le score wake-word dans [0.0, 1.0].
    pub fn infer(&self, mfcc: &[[f32; 13]; 98]) -> Result<f32, InferenceError> {
        // ── Feature mock_model : stub retourne toujours 0.5 ─────────────────
        #[cfg(feature = "mock_model")]
        if self.handle.is_null() {
            return Ok(0.5);
        }

        // ── Inférence réelle ──────────────────────────────────────────────────
        let flat: Vec<f32> = mfcc.iter().flatten().copied().collect();
        let score = unsafe { ffi::coreml_infer(self.handle, flat.as_ptr(), flat.len()) };
        check_score(score)
    }
}

impl Drop for CoreMLModel {
    fn drop(&mut self) {
        // Le stub mock_model a un handle null : rien à libérer.
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
    #[cfg(not(feature = "mock_model"))]
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
    #[cfg(not(feature = "mock_model"))]
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

    /// Score invalide (< 0.0) → Err(InferenceFailed).
    #[test]
    fn score_below_zero_is_inference_failed() {
        let err = check_score(-0.1).unwrap_err();
        assert!(
            matches!(err, InferenceError::InferenceFailed(_)),
            "attendu InferenceFailed, obtenu {err}"
        );
    }

    /// Score invalide (> 1.0) → Err(InferenceFailed).
    #[test]
    fn score_above_one_is_inference_failed() {
        let err = check_score(1.1).unwrap_err();
        assert!(
            matches!(err, InferenceError::InferenceFailed(_)),
            "attendu InferenceFailed, obtenu {err}"
        );
    }

    /// Score valide aux bornes → Ok.
    #[test]
    fn score_at_bounds_is_ok() {
        assert!(check_score(0.0).is_ok());
        assert!(check_score(1.0).is_ok());
        assert!(check_score(0.5).is_ok());
    }

    /// Avec mock_model : load retourne Ok sans chemin valide.
    #[cfg(feature = "mock_model")]
    #[test]
    fn mock_load_returns_ok_without_real_path() {
        let config = InferenceConfig::default();
        assert!(CoreMLModel::load(&config).is_ok());
    }

    /// Avec mock_model : infer retourne toujours 0.5.
    #[cfg(feature = "mock_model")]
    #[test]
    fn mock_infer_returns_0_5() {
        let config = InferenceConfig::default();
        let model = CoreMLModel::load(&config).unwrap();
        let score = model.infer(&[[0.0f32; 13]; 98]).unwrap();
        assert_eq!(score, 0.5);
    }
}
