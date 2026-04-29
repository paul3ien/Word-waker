//! Exemple standalone : charge le modèle mock, envoie 10 matrices MFCC
//! aléatoires et affiche les scores ainsi que la latence médiane.
//!
//! Usage : cargo run --example standalone_inference --features standalone -p inference_ml

use crossbeam_channel::bounded;
use inference_ml::{InferenceConfig, InferenceEngine};
use std::time::{Duration, Instant};

fn main() {
    // Initialise le logger tracing pour voir les messages de debug du runner.
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // Construit une config pointant sur le modèle mock dans les fixtures.
    let model_path = format!(
        "{}/fixtures/mock_model/WakeWordMock.mlmodelc",
        env!("CARGO_MANIFEST_DIR")
    );

    let config = InferenceConfig {
        model_path,
        ..Default::default()
    };

    let mut engine = InferenceEngine::new(config).expect("Impossible de charger le modèle");

    let (tx_in, rx_in) = bounded::<[[f32; 13]; 98]>(16);
    let (tx_out, rx_out) = bounded::<f32>(16);

    engine
        .start(rx_in, tx_out)
        .expect("Impossible de démarrer le runner");

    const N: usize = 10;
    let mut latencies = Vec::with_capacity(N);

    for i in 0..N {
        // Génère une matrice avec des valeurs pseudo-aléatoires déterministes.
        let mut mfcc = [[0.0f32; 13]; 98];
        for (r, row) in mfcc.iter_mut().enumerate() {
            for (c, v) in row.iter_mut().enumerate() {
                *v = ((i * 98 * 13 + r * 13 + c) as f32 * 0.001) - 0.5;
            }
        }

        let t0 = Instant::now();
        tx_in.send(mfcc).expect("send échoué");
        let score = rx_out
            .recv_timeout(Duration::from_secs(5))
            .expect("timeout en attente du score");
        let latency = t0.elapsed();

        latencies.push(latency);
        println!(
            "  [{i:2}] score = {score:.4}  latence = {} µs",
            latency.as_micros()
        );
    }

    drop(tx_in);
    engine.stop().expect("stop échoué");

    // Calcule la latence médiane.
    latencies.sort_unstable();
    let median = latencies[N / 2];
    println!(
        "\nLatence médiane : {} µs ({} ms)",
        median.as_micros(),
        median.as_millis()
    );
    println!("Terminé proprement.");
}
