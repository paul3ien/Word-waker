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
