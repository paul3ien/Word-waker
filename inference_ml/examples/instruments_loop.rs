//! Binary de boucle pour profiling Instruments.
//!
//! Lance des inférences en continu pendant 30 secondes, permettant de
//! capturer l'activité ANE, CPU et mémoire dans Xcode Instruments.
//!
//! Usage : cargo run --example instruments_loop -p inference_ml
//! Puis : Instruments → Attach to process → `instruments_loop`

use crossbeam_channel::bounded;
use inference_ml::{InferenceConfig, InferenceEngine};
use std::time::{Duration, Instant};

fn main() {
    let model_path = format!(
        "{}/fixtures/mock_model/WakeWordMock.mlmodelc",
        env!("CARGO_MANIFEST_DIR")
    );

    let config = InferenceConfig {
        model_path,
        ..Default::default()
    };
    let mut engine = InferenceEngine::new(config).expect("load échoué");

    let (tx_in, rx_in) = bounded::<[[f32; 13]; 98]>(32);
    let (tx_out, rx_out) = bounded::<f32>(32);
    engine.start(rx_in, tx_out).expect("start échoué");

    let duration = Duration::from_secs(30);
    let deadline = Instant::now() + duration;

    let mut count: u64 = 0;
    let mut sum_us: u64 = 0;
    let mfcc = [[0.1f32; 13]; 98];

    while Instant::now() < deadline {
        let t0 = Instant::now();
        tx_in.send(mfcc).expect("send");
        let _score = rx_out
            .recv_timeout(Duration::from_secs(1))
            .expect("timeout");
        sum_us += t0.elapsed().as_micros() as u64;
        count += 1;
    }

    drop(tx_in);
    engine.stop().expect("stop");

    let median_us = sum_us / count;
    let throughput = count as f64 / 30.0;
    println!(
        "Inférences : {count}  |  Latence moy : {median_us} µs  |  Débit : {throughput:.1} inf/s"
    );
}
