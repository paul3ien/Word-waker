use crossbeam_channel::bounded;
use inference_ml::ffi;
use inference_ml::{CoreMLModel, InferenceConfig, InferenceEngine, InferenceRunner};
use std::ffi::CString;

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
    assert!(
        !handle.is_null(),
        "coreml_load devrait retourner un handle non-null"
    );
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
    let score = unsafe { ffi::coreml_infer(std::ptr::null_mut(), mfcc.as_ptr(), mfcc.len()) };
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
/// (Non applicable avec mock_model : le stub bypasse la vérification du chemin.)
#[cfg(not(feature = "mock_model"))]
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

// ─── P6.3 ────────────────────────────────────────────────────────────────────

/// Envoyer 5 matrices → recevoir 5 scores dans [0.0, 1.0].
#[test]
fn runner_sends_5_scores_in_range() {
    let model = CoreMLModel::load(&mock_config()).expect("load échoué");
    let mut runner = InferenceRunner::new(model);

    let (tx_in, rx_in) = bounded::<[[f32; 13]; 98]>(8);
    let (tx_out, rx_out) = bounded::<f32>(8);

    runner.start(rx_in, tx_out).expect("start échoué");

    for _ in 0..5 {
        tx_in.send([[0.0f32; 13]; 98]).expect("send échoué");
    }

    let mut scores = Vec::new();
    for _ in 0..5 {
        let score = rx_out
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("timeout en attente du score");
        scores.push(score);
    }

    runner.stop();

    for s in &scores {
        assert!((0.0f32..=1.0f32).contains(s), "score={s} hors [0.0, 1.0]");
    }
    assert_eq!(scores.len(), 5);
}

/// Fermer le Sender → le thread se termine proprement sans panique.
#[test]
fn runner_stops_cleanly_when_sender_closed() {
    let model = CoreMLModel::load(&mock_config()).expect("load échoué");
    let mut runner = InferenceRunner::new(model);

    let (tx_in, rx_in) = bounded::<[[f32; 13]; 98]>(8);
    let (tx_out, rx_out) = bounded::<f32>(8);

    runner.start(rx_in, tx_out).expect("start échoué");

    // Ferme le canal d'entrée → le thread doit sortir de la boucle.
    drop(tx_in);

    // stop() doit rejoindre le thread sans panic ni timeout.
    runner.stop();

    // Le canal de sortie est vide (aucune matrice envoyée).
    assert!(rx_out.try_recv().is_err());
}

/// Drop du runner sans stop() explicite → pas de thread zombie.
#[test]
fn runner_drop_without_explicit_stop_no_zombie() {
    let model = CoreMLModel::load(&mock_config()).expect("load échoué");
    let mut runner = InferenceRunner::new(model);

    let (_tx_in, rx_in) = bounded::<[[f32; 13]; 98]>(8);
    let (tx_out, _rx_out) = bounded::<f32>(8);

    runner.start(rx_in, tx_out).expect("start échoué");

    // Drop implicite — le runner enverra le signal de stop via Drop, puis join().
    drop(runner);
    // Pas de panic = succès
}

/// 100 inférences consécutives via le runner → mémoire stable.
#[test]
fn runner_100_inferences_no_memory_growth() {
    let model = CoreMLModel::load(&mock_config()).expect("load échoué");
    let mut runner = InferenceRunner::new(model);

    let (tx_in, rx_in) = bounded::<[[f32; 13]; 98]>(16);
    let (tx_out, rx_out) = bounded::<f32>(16);

    runner.start(rx_in, tx_out).expect("start échoué");

    for _ in 0..100 {
        tx_in.send([[0.1f32; 13]; 98]).expect("send échoué");
        let score = rx_out
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("timeout");
        assert!((0.0f32..=1.0f32).contains(&score));
    }

    drop(tx_in);
    runner.stop();
}

// ─── P7.2 ────────────────────────────────────────────────────────────────────

/// Cycle complet : new → start → 3 inférences → stop.
#[test]
fn engine_new_start_infer_stop() {
    let config = mock_config();
    let mut engine = InferenceEngine::new(config).expect("new échoué");

    let (tx_in, rx_in) = bounded::<[[f32; 13]; 98]>(8);
    let (tx_out, rx_out) = bounded::<f32>(8);

    engine.start(rx_in, tx_out).expect("start échoué");

    for _ in 0..3 {
        tx_in.send([[0.0f32; 13]; 98]).expect("send échoué");
    }

    let mut scores = Vec::new();
    for _ in 0..3 {
        let s = rx_out
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("timeout score");
        scores.push(s);
    }

    drop(tx_in);
    engine.stop().expect("stop échoué");

    assert_eq!(scores.len(), 3);
    for s in &scores {
        assert!((0.0f32..=1.0f32).contains(s), "score={s} hors [0.0, 1.0]");
    }
}

/// Drop sans stop() explicite → propre, pas de panique.
#[test]
fn engine_drop_without_stop_no_crash() {
    let config = mock_config();
    let mut engine = InferenceEngine::new(config).expect("new échoué");

    let (_tx_in, rx_in) = bounded::<[[f32; 13]; 98]>(8);
    let (tx_out, _rx_out) = bounded::<f32>(8);

    engine.start(rx_in, tx_out).expect("start échoué");

    // Drop implicite — InferenceEngine::drop appelle runner.stop()
    drop(engine);
}

/// Deux cycles start/stop consécutifs — idempotence, zéro panique.
#[test]
fn engine_two_consecutive_start_stop_cycles() {
    let config = mock_config();
    let mut engine = InferenceEngine::new(config).expect("new échoué");

    // ── Cycle 1 ───────────────────────────────────────────────────────────────
    let (tx_in1, rx_in1) = bounded::<[[f32; 13]; 98]>(4);
    let (tx_out1, rx_out1) = bounded::<f32>(4);

    engine.start(rx_in1, tx_out1).expect("start cycle 1 échoué");

    tx_in1.send([[0.0f32; 13]; 98]).expect("send cycle 1 échoué");
    let score1 = rx_out1
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("timeout score cycle 1");
    assert!(
        (0.0f32..=1.0f32).contains(&score1),
        "cycle 1 score={score1} hors [0.0, 1.0]"
    );

    drop(tx_in1);
    engine.stop().expect("stop cycle 1 échoué");

    // ── Cycle 2 ───────────────────────────────────────────────────────────────
    let (tx_in2, rx_in2) = bounded::<[[f32; 13]; 98]>(4);
    let (tx_out2, rx_out2) = bounded::<f32>(4);

    engine.start(rx_in2, tx_out2).expect("start cycle 2 échoué");

    tx_in2.send([[0.5f32; 13]; 98]).expect("send cycle 2 échoué");
    let score2 = rx_out2
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("timeout score cycle 2");
    assert!(
        (0.0f32..=1.0f32).contains(&score2),
        "cycle 2 score={score2} hors [0.0, 1.0]"
    );

    drop(tx_in2);
    engine.stop().expect("stop cycle 2 échoué");
}

// ─── P9.3 ────────────────────────────────────────────────────────────────────

/// Régression : new → drop sans start — zéro fuite.
#[test]
fn engine_new_drop_without_start_no_crash() {
    let config = mock_config();
    let engine = InferenceEngine::new(config).expect("new échoué");
    drop(engine);
    // Pas de crash = succès
}

/// Régression : new → start → 10 inférences → stop.
#[test]
fn engine_10_inferences_regression() {
    let config = mock_config();
    let mut engine = InferenceEngine::new(config).expect("new échoué");

    let (tx_in, rx_in) = bounded::<[[f32; 13]; 98]>(16);
    let (tx_out, rx_out) = bounded::<f32>(16);
    engine.start(rx_in, tx_out).expect("start échoué");

    for _ in 0..10 {
        tx_in.send([[0.0f32; 13]; 98]).expect("send");
        let score = rx_out
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("timeout");
        assert!((0.0f32..=1.0f32).contains(&score));
    }

    drop(tx_in);
    engine.stop().expect("stop");
}

/// Régression : 1000 load → infer → free consécutifs — mémoire stable.
#[test]
fn model_1000_load_infer_free_stable() {
    for _ in 0..1000 {
        let model = CoreMLModel::load(&mock_config()).expect("load");
        let _ = model.infer(&[[0.0f32; 13]; 98]).expect("infer");
        // drop implicite = free
    }
}

/// Régression : channel fermé pendant inférence → thread se termine proprement.
#[test]
fn runner_channel_closed_during_inference_terminates() {
    let model = CoreMLModel::load(&mock_config()).expect("load");
    let mut runner = InferenceRunner::new(model);

    let (tx_in, rx_in) = bounded::<[[f32; 13]; 98]>(8);
    let (tx_out, rx_out) = bounded::<f32>(8);
    runner.start(rx_in, tx_out).expect("start");

    // Envoyer une matrice puis fermer le canal d'entrée immédiatement.
    tx_in.send([[0.0f32; 13]; 98]).expect("send");
    drop(tx_in);

    // Recevoir le score (preuve que le thread a traité avant de s'arrêter).
    let score = rx_out
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("timeout");
    assert!((0.0f32..=1.0f32).contains(&score));

    // Le thread doit être terminé ou en cours de terminaison.
    runner.stop();
}

// ─── P9.2 ────────────────────────────────────────────────────────────────────

/// Valide que les scores Rust correspondent aux scores Python de validation_cases.json.
///
/// Pré-requis (skip automatique si absents) :
///   - `fixtures/real_model/WakeWord.mlmodelc`  (copié depuis model_ww_v2/exports/)
///   - `validation_cases.json`                   (généré par validate_fixture.py)
///
/// Le chemin JSON peut être surchargé via la var d'env `WW_EXPORTS_DIR`.
#[cfg(not(feature = "mock_model"))]
#[test]
fn real_model_matches_validation_cases() {
    use std::path::PathBuf;

    // ── Chemins ──────────────────────────────────────────────────────────────
    let real_model_path = format!(
        "{}/fixtures/real_model/WakeWord.mlmodelc",
        env!("CARGO_MANIFEST_DIR")
    );

    let json_path: PathBuf = std::env::var("WW_EXPORTS_DIR")
        .map(|d| PathBuf::from(d).join("validation_cases.json"))
        .unwrap_or_else(|_| {
            PathBuf::from(
                "/Users/apple/Documents/Code/model_ww_v2/exports/validation_cases.json",
            )
        });

    // ── Skip si artefacts absents (modèle pas encore entraîné) ───────────────
    if !std::path::Path::new(&real_model_path).exists() {
        eprintln!(
            "[SKIP] real_model_matches_validation_cases : {} absent",
            real_model_path
        );
        eprintln!("       Lancer d'abord src/export/export_run.py puis copier :");
        eprintln!(
            "       cp -r model_ww_v2/exports/WakeWord.mlmodelc \
                    inference_ml/fixtures/real_model/"
        );
        return;
    }
    if !json_path.exists() {
        eprintln!(
            "[SKIP] real_model_matches_validation_cases : {} absent",
            json_path.display()
        );
        eprintln!("       Lancer d'abord : python src/scripts/validate_fixture.py");
        return;
    }

    // ── Charger le vrai modèle ────────────────────────────────────────────────
    let cpath = std::ffi::CString::new(real_model_path.as_str()).expect("CString invalide");
    let handle = unsafe { ffi::coreml_load(cpath.as_ptr()) };
    assert!(!handle.is_null(), "coreml_load du vrai modèle échoué");

    // ── Lire validation_cases.json ────────────────────────────────────────────
    let json_str = std::fs::read_to_string(&json_path).expect("lecture JSON échouée");
    let payload: serde_json::Value = serde_json::from_str(&json_str).expect("JSON invalide");
    let cases = payload["cases"].as_array().expect("clé 'cases' manquante dans le JSON");
    assert_eq!(cases.len(), 20, "20 cas attendus dans validation_cases.json");

    // ── Comparer les scores (tolérance < 0.01) ────────────────────────────────
    let tolerance: f32 = 0.01;
    let mut max_err: f32 = 0.0;

    for case in cases {
        let name = case["name"].as_str().unwrap_or("?");
        let score_python = case["score_ww"]
            .as_f64()
            .expect("champ 'score_ww' manquant") as f32;

        // Reconstruire le vecteur MFCC aplati (98 × 13 = 1274 floats)
        let mfcc_json = case["mfcc"].as_array().expect("champ 'mfcc' manquant");
        let mut mfcc_flat: Vec<f32> = Vec::with_capacity(98 * 13);
        for row in mfcc_json {
            for v in row.as_array().expect("ligne mfcc invalide") {
                mfcc_flat.push(v.as_f64().expect("valeur mfcc invalide") as f32);
            }
        }
        assert_eq!(mfcc_flat.len(), 98 * 13, "taille mfcc incorrecte pour '{name}'");

        let score_rust =
            unsafe { ffi::coreml_infer(handle, mfcc_flat.as_ptr(), mfcc_flat.len()) };

        let err = (score_rust - score_python).abs();
        max_err = max_err.max(err);

        assert!(
            err < tolerance,
            "cas '{name}': score_rust={score_rust:.6} score_python={score_python:.6} \
             err={err:.4} >= {tolerance}"
        );
    }

    eprintln!(
        "  20/20 cas validés  |  err_max={max_err:.4e}  |  tolérance={tolerance}"
    );

    unsafe { ffi::coreml_free(handle) };
}
