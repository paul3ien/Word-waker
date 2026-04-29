use std::ffi::CString;
use inference_ml::ffi;

/// Chemin absolu vers le .mlmodelc compilé dans les fixtures du crate.
fn mock_model_path() -> CString {
    let path = format!(
        "{}/fixtures/mock_model/WakeWordMock.mlmodelc",
        env!("CARGO_MANIFEST_DIR")
    );
    CString::new(path).expect("chemin invalide")
}

// ─── P4.2 ────────────────────────────────────────────────────────────────────

/// coreml_load avec le modèle mock doit retourner un handle non-null.
#[test]
fn bridge_load_mock_model_returns_non_null() {
    let cpath = mock_model_path();
    let handle = unsafe { ffi::coreml_load(cpath.as_ptr()) };
    assert!(!handle.is_null(), "coreml_load devrait retourner un handle non-null");
    // Libération propre
    unsafe { ffi::coreml_free(handle) };
}

// ─── P4.3 ────────────────────────────────────────────────────────────────────

/// coreml_infer sur une matrice de zéros doit retourner un score dans [0.0, 1.0].
#[test]
fn bridge_infer_zeros_returns_score_in_range() {
    let cpath = mock_model_path();
    let handle = unsafe { ffi::coreml_load(cpath.as_ptr()) };
    assert!(!handle.is_null(), "load échoué");

    let mfcc = vec![0.0f32; 1 * 98 * 13];
    let score = unsafe { ffi::coreml_infer(handle, mfcc.as_ptr(), mfcc.len()) };

    assert!(
        (0.0f32..=1.0f32).contains(&score),
        "score={score} hors [0.0, 1.0]"
    );
    unsafe { ffi::coreml_free(handle) };
}

// ─── P4.4 ────────────────────────────────────────────────────────────────────

/// load → free sans inférence ne doit pas crasher.
#[test]
fn bridge_load_free_no_crash() {
    let cpath = mock_model_path();
    let handle = unsafe { ffi::coreml_load(cpath.as_ptr()) };
    assert!(!handle.is_null(), "load échoué");
    unsafe { ffi::coreml_free(handle) };
    // Pas de crash = succès
}

// ─── P4.5 null-checks ────────────────────────────────────────────────────────

/// coreml_load(nullptr) doit retourner nullptr sans crash.
#[test]
fn bridge_load_null_path_returns_null() {
    let handle = unsafe { ffi::coreml_load(std::ptr::null()) };
    assert!(handle.is_null(), "coreml_load(nullptr) doit retourner null");
}

/// coreml_infer(nullptr, …) doit retourner 0.0 sans crash.
#[test]
fn bridge_infer_null_handle_returns_zero() {
    let mfcc = vec![0.0f32; 1 * 98 * 13];
    let score = unsafe {
        ffi::coreml_infer(std::ptr::null_mut(), mfcc.as_ptr(), mfcc.len())
    };
    assert_eq!(score, 0.0f32, "coreml_infer(nullptr) doit retourner 0.0");
}
