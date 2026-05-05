//! Test d'intégration workspace : simule le flux pipeline_dsp → inference_ml.
//!
//! Vérifie que le crate `inference_ml` peut être consommé depuis un autre
//! crate du workspace sans erreur de compilation ni de runtime.

use crossbeam_channel::bounded;
use inference_ml::{InferenceConfig, InferenceEngine};
use std::time::Duration;

/// Chemin vers le modèle mock dans les fixtures d'inference_ml.
fn mock_config() -> InferenceConfig {
    InferenceConfig {
        model_path: format!(
            "{}/../inference_ml/fixtures/mock_model/WakeWordMock.mlmodelc",
            env!("CARGO_MANIFEST_DIR")
        ),
        ..Default::default()
    }
}

/// Simule : pipeline_dsp envoie des matrices MFCC → inference_ml répond avec des scores.
#[test]
fn pipeline_dsp_to_inference_ml_sends_scores() {
    let mut engine = InferenceEngine::new(mock_config()).expect("InferenceEngine::new");

    let (tx_mfcc, rx_mfcc) = bounded::<[[f32; 13]; 98]>(8);
    let (tx_score, rx_score) = bounded::<f32>(8);
    engine.start(rx_mfcc, tx_score).expect("start");

    // Simule 3 envois de matrices MFCC synthétiques depuis pipeline_dsp.
    for i in 0..3u32 {
        let val = i as f32 * 0.1;
        let mfcc = [[val; 13]; 98];
        tx_mfcc.send(mfcc).expect("send mfcc");
        let score = rx_score
            .recv_timeout(Duration::from_secs(5))
            .expect("timeout attente score");
        assert!(
            (0.0f32..=1.0f32).contains(&score),
            "score hors [0.0, 1.0] : {score}"
        );
    }

    drop(tx_mfcc);
    engine.stop().expect("stop");
}

/// Vérifie qu'une config invalide (model_path vide) est refusée proprement.
#[test]
fn invalid_config_returns_error() {
    let config = InferenceConfig::default(); // model_path vide
    let err = InferenceEngine::new(config).err().unwrap();
    let msg = err.to_string();
    assert!(!msg.is_empty(), "message d'erreur vide");
}

// ─── P6.1 — Tests d'intégration inference_ml → trigger → socket ──────────────

use std::io::Read;
use std::os::unix::net::UnixListener;
use std::sync::atomic::{AtomicU32, Ordering};
use trigger::{TriggerConfig, TriggerModule};

static SOCKET_COUNTER: AtomicU32 = AtomicU32::new(0);

fn unique_socket_path() -> String {
    let id = SOCKET_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("/tmp/wakeword_workspace_test_{}.sock", id)
}

fn cleanup(path: &str) {
    let _ = std::fs::remove_file(path);
}

/// Test P6.1 — Chaîne complète : inference_ml (mock_model) → trigger → UnixSocket.
///
/// Envoie des matrices MFCC synthétiques à `InferenceEngine`, passe les scores
/// obtenus au `TriggerModule`, et vérifie la réception d'un message socket
/// de bout en bout.
#[test]
fn inference_ml_to_trigger_to_socket_end_to_end() {
    let socket_path = unique_socket_path();
    cleanup(&socket_path);

    // Listener IPC côté "client"
    let listener = UnixListener::bind(&socket_path).expect("bind failed");

    // TriggerModule avec seuil bas (0.01) pour s'assurer qu'un score quelconque déclenche
    let trigger_config = TriggerConfig {
        socket_path: socket_path.clone(),
        score_threshold: 0.01,
        vote_threshold: 1,
        window_size: 3,
        cooldown_ms: 500,
    };
    let mut module = TriggerModule::new(trigger_config).expect("TriggerModule::new failed");

    // Channel trigger : scores f32 de inference_ml → trigger
    let (tx_score, rx_score) = bounded::<f32>(8);
    module.start(rx_score).expect("TriggerModule::start failed");

    // InferenceEngine avec mock_model
    let mut engine = InferenceEngine::new(mock_config()).expect("InferenceEngine::new");
    let (tx_mfcc, rx_mfcc) = bounded::<[[f32; 13]; 98]>(8);
    let (tx_inf_score, rx_inf_score) = bounded::<f32>(8);
    engine.start(rx_mfcc, tx_inf_score).expect("InferenceEngine::start");

    // Envoyer des matrices MFCC et relayer les scores vers trigger
    let start = std::time::Instant::now();
    for _ in 0..5 {
        let mfcc = [[0.5f32; 13]; 98];
        tx_mfcc.send(mfcc).expect("send mfcc");
        let score = rx_inf_score
            .recv_timeout(Duration::from_secs(5))
            .expect("timeout score inference");
        tx_score.send(score).expect("relay score to trigger");
    }

    // Attendre la notification socket (timeout 5 s)
    listener
        .set_nonblocking(false)
        .expect("set_nonblocking failed");
    let (mut stream, _) = listener.accept().expect("accept timeout — aucun message reçu");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).expect("read failed");
    assert_eq!(buf, b"WAKEWORD_DETECTED\n", "message socket inattendu : {:?}", buf);

    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_millis(150),
        "latence bout-en-bout trop élevée : {:?}",
        elapsed
    );

    drop(tx_mfcc);
    drop(tx_score);
    engine.stop().expect("engine stop");
    module.stop().expect("module stop");
    cleanup(&socket_path);
}

