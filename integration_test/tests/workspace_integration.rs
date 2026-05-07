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

// ─── P8.3 trigger — Chaîne complète workspace ────────────────────────────────
//
// audio_capture (mock) → pipeline_dsp → inference_ml (mock) → trigger → socket
//
// Chaque maillon tourne dans son propre thread (runners). Le test injecte 3 s
// de signal sinusoïdal 440 Hz via le mock audio, attend qu'au moins un message
// WAKEWORD_DETECTED soit reçu sur le socket, et vérifie que tout s'arrête
// proprement.

use audio_capture::audio_capture::mock;
use pipeline_dsp::pipeline_dsp::runner::DspRunner;
use pipeline_dsp::pipeline_dsp::config::DspConfig;

/// Test P8.3 — Chaîne complète workspace : mock audio → DSP → inference → trigger → socket.
#[test]
fn full_pipeline_mock_audio_to_socket() {
    let socket_path = unique_socket_path();
    cleanup(&socket_path);

    // ── UnixListener côté "client" ────────────────────────────────────────
    let listener = std::os::unix::net::UnixListener::bind(&socket_path)
        .expect("bind listener");
    listener.set_nonblocking(true).expect("set_nonblocking");

    // ── TriggerModule (seuil bas pour garantir une détection sur mock) ────
    let trigger_config = TriggerConfig {
        socket_path: socket_path.clone(),
        score_threshold: 0.01,
        vote_threshold: 1,
        window_size: 3,
        cooldown_ms: 500,
    };
    let (tx_score, rx_score) = bounded::<f32>(16);
    let mut trigger_mod = TriggerModule::new(trigger_config).expect("TriggerModule::new");
    trigger_mod.start(rx_score).expect("TriggerModule::start");

    // ── InferenceEngine (mock_model) ─────────────────────────────────────
    let mut engine = InferenceEngine::new(mock_config()).expect("InferenceEngine::new");
    let (tx_mfcc, rx_mfcc) = bounded::<[[f32; 13]; 98]>(8);
    let (tx_inf_score, rx_inf_score) = bounded::<f32>(16);
    engine.start(rx_mfcc, tx_inf_score).expect("engine start");

    // Thread relais score inference → trigger
    let tx_score_relay = tx_score.clone();
    let relay = std::thread::spawn(move || {
        while let Ok(score) = rx_inf_score.recv() {
            let _ = tx_score_relay.send(score);
        }
    });

    // ── DspRunner (pipeline_dsp) ─────────────────────────────────────────
    let (tx_samples, rx_samples) = bounded::<Vec<f32>>(8);
    let dsp_runner = DspRunner::start(DspConfig::default(), rx_samples, tx_mfcc)
        .expect("DspRunner::start");

    // ── Injection des samples (mock audio : sinus 440 Hz, 3 s à 16 kHz) ──
    // Envoyer par batches de 16 000 samples (1 s) × 3
    let samples_1s = mock::generate_sine(440.0, 16_000.0, 16_000);
    for _ in 0..3 {
        tx_samples.send(samples_1s.clone()).expect("send samples");
    }
    drop(tx_samples); // signal EOF au DspRunner

    // ── Attendre la réception du message socket (timeout 10 s) ───────────
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut received = false;
    while std::time::Instant::now() < deadline {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let mut buf = Vec::new();
                let _ = stream.read_to_end(&mut buf);
                assert_eq!(
                    buf, b"WAKEWORD_DETECTED\n",
                    "message inattendu : {:?}", buf
                );
                received = true;
                break;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => panic!("accept error: {e}"),
        }
    }
    assert!(received, "aucun message socket reçu dans le délai imparti");

    // ── Arrêt propre ─────────────────────────────────────────────────────
    drop(tx_score);
    drop(dsp_runner);
    let _ = relay.join();
    engine.stop().expect("engine stop");
    trigger_mod.stop().expect("trigger stop");
    cleanup(&socket_path);
}

