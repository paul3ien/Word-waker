use libc::{c_char, c_void, size_t};

/// Handle opaque vers un `MLModel` Objective-C géré côté Rust.
/// Null = handle invalide.
pub type CoreMLHandle = *mut c_void;

extern "C" {
    /// Charge le modèle `.mlmodelc` situé à `path` (null-terminé).
    /// Utilise `MLComputeUnitsAll` (ANE prioritaire).
    /// Retourne un handle non-null en cas de succès, null sinon.
    pub fn coreml_load(path: *const c_char) -> CoreMLHandle;

    /// Idem que `coreml_load` mais force `MLComputeUnitsCPUOnly`.
    /// Utilisé pour les benchmarks de comparaison CPU vs ANE.
    pub fn coreml_load_cpu_only(path: *const c_char) -> CoreMLHandle;

    /// Lance une inférence synchrone.
    /// `mfcc_flat` : pointeur vers `len` floats (matrice MFCC aplatie).
    /// Retourne le score wake-word ∈ [0.0, 1.0], ou 0.0 en cas d'erreur.
    pub fn coreml_infer(
        handle: CoreMLHandle,
        mfcc_flat: *const f32,
        len: size_t,
    ) -> f32;

    /// Libère le `MLModel` ARC associé au handle.
    pub fn coreml_free(handle: CoreMLHandle);
}
