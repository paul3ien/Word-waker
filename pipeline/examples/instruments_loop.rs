//! Boucle continue pour validation CPU via Instruments (P11.5).
//!
//! Fait tourner le pipeline DSP en boucle pendant 60 secondes sur des batches
//! de 1 seconde de signal synthétique, puis affiche les statistiques.
//!
//! Usage :
//! ```
//! cargo build --example instruments_loop --release
//! # Puis lancer dans Instruments → CPU Profiler (ou Time Profiler)
//! cargo run --example instruments_loop --release
//! ```
//!
//! Résultat attendu : CPU < 0,1 % au repos entre les batches (thread runner
//! bloqué sur `recv_timeout`), < 5 % en traitement actif.

use std::time::{Duration, Instant};

use pipeline_dsp::pipeline_dsp::config::DspConfig;
use pipeline_dsp::pipeline_dsp::pipeline::DspPipeline;

fn main() {
    let sample_rate = 16_000usize;
    let duration_s = 60u64;

    // Génère 1 seconde de sinus 440 Hz (réutilisé à chaque itération)
    let samples_1s: Vec<f32> = (0..sample_rate)
        .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sample_rate as f32).sin())
        .collect();

    let config = DspConfig::default();
    let mut pipeline = DspPipeline::new(config).expect("DspPipeline::new");

    println!(
        "Démarrage boucle {}s — pipeline DSP (sinus 440 Hz, 1 batch/s)",
        duration_s
    );

    let t_start = Instant::now();
    let mut n_batches = 0usize;
    let mut n_matrices = 0usize;
    let mut total_proc_ns = 0u128;

    while t_start.elapsed() < Duration::from_secs(duration_s) {
        let t0 = Instant::now();
        let matrices = pipeline.process_batch(&samples_1s);
        total_proc_ns += t0.elapsed().as_nanos();
        n_matrices += matrices.len();
        n_batches += 1;

        // Pause ~900 ms entre les batches (simule l'arrivée temps-réel de l'audio)
        let proc_elapsed = t0.elapsed();
        let target = Duration::from_millis(1000);
        if proc_elapsed < target {
            std::thread::sleep(target - proc_elapsed);
        }
    }

    let total_elapsed = t_start.elapsed().as_secs_f64();
    let avg_proc_ms = (total_proc_ns as f64 / n_batches as f64) / 1_000_000.0;
    let cpu_pct = (total_proc_ns as f64 / 1e9) / total_elapsed * 100.0;

    println!("─────────────────────────────────────────");
    println!("Durée réelle       : {:.1} s", total_elapsed);
    println!("Batches traités    : {}", n_batches);
    println!("Matrices produites : {}", n_matrices);
    println!("Latence moy/batch  : {:.3} ms", avg_proc_ms);
    println!(
        "CPU estimé (proc)  : {:.4} %  (attendu < 0.1 %)",
        cpu_pct
    );

    assert!(
        avg_proc_ms < 5.0,
        "Latence moyenne {:.3} ms > 5 ms",
        avg_proc_ms
    );
    assert!(
        cpu_pct < 5.0,
        "CPU processing {:.4} % > 5 % (seuil large pour batch actif)",
        cpu_pct
    );

    println!("✅ Validation P11.5 CPU OK");
}
