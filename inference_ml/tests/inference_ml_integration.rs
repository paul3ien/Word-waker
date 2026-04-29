use std::ffi::CString;
use inference_ml::ffi;
use inference_ml::{CoreMLModel, InferenceConfig};

/// Chemin absolu vers le .mlmodelc compilé dans les fixtures du crate.
fn mock_model_path() -> CString {
    let path = format!(
        "{}/fixtures/mock_model/WakeWordMock.mlmodelc",
        env!("CARGO_MANIFEST_DIR")
    );
    CString::new(path).expect("chemin invalide")
}

/// Config pointant sur le modèle mock.
fn mock_config() -> InferenceConfig {
    InferenceConfig {
        model_path: format!(
            "{}/fixtures/mock_model/WakeWordMock.mlmodelc",
            env!("CARGO_MANIFEST_DIR")
        ),
        ..Default::default()
    }
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

// ─── P5.1 ────────────────────────────────────────────────────────────────────

/// CoreMLModel::load avec le modèle mock → Ok.
#[test]
fn model_load_mock_returns_ok() {
    let config = mock_config();
    let model = CoreMLModel::load(&config);
    assert!(model.is_ok(), "load échoué : {:?}", model.err());
}

/// CoreMLModel::load avec un chemin inexistant → Err(ModelNotFound).
#[test]
fn model_load_nonexistent_returns_model_not_found() {
    use inference_ml::InferenceError;
    let config = InferenceConfig {
        model_path: "/tmp/inexistant.mlmodelc".into(),
        ..Default::default()
    };
    let err = CoreMLModel::load(&config).err().unwrap();
    assert!(
        matches!(err, InferenceError::ModelNotFound(_)),
        "attendu ModelNotFound, obtenu {err}"
    );
}

// ─── P5.2 ────────────────────────────────────────────────────────────────────

/// Inférence sur matrice de zéros → score dans [0.0, 1.0].
#[test]
fn model_infer_zeros_returns_score_in_range() {
    let model = CoreMLModel::load(&mock_config()).expect("load échoué");
    let mfcc = [[0.0f32; 13]; 98];
    let score = model.infer(&mfcc).expect("infer échoué");
    assert!(
        (0.0f32..=1.0f32).contains(&score),
        "score={score} hors [0.0, 1.0]"
    );
}

/// Inférence sur matrice de valeurs aléatoires → score dans [0.0, 1.0].
#[test]
fn model_infer_random_values_returns_score_in_range() {
    let model = CoreMLModel::load(&mock_config()).expect("load échoué");
    // Valeurs arbitraires non nulles pour exercer le chemin de calcul.
    let mut mfcc = [[0.0f32; 13]; 98];
    for (i, row) in mfcc.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = ((i * 13 + j) as f32) * 0.001 - 0.5;
        }
    }
    let score = model.infer(&mfcc).expect("infer échoué");
    assert!(
        (0.0f32..=1.0f32).contains(&score),
        "score={score} hors [0.0, 1.0]"
    );
}

// ─── P5.3 ────────────────────────────────────────────────────────────────────

/// Drop explicite dans un scope : aucun crash (pas de double-free, pas de leak visible).
#[test]
fn model_drop_after_load_no_crash() {
    {
        let model = CoreMLModel::load(&mock_config()).expect("load échoué");
        let _ = model.infer(&[[0.0f32; 13]; 98]);
        // drop implicite ici
    }
    // Si on arrive ici sans crash, le Drop a fonctionné correctement.
}

/// Créer et dropper 100 instances successives → pas de crash / double-free.
#[test]
fn model_repeated_load_drop_no_crash() {
    for _ in 0..100 {
        let model = CoreMLModel::load(&mock_config()).expect("load échoué");
        let _ = model.infer(&[[0.0f32; 13]; 98]);
        // drop implicite à chaque itération
    }
}

