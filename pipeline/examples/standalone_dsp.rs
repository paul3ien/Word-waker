//! Exemple standalone : génère 3 secondes de sinus 440 Hz, traite tout le signal
//! via `DspPipeline`, affiche les MFCC de la première matrice et les statistiques.
//!
//! ```
//! cargo run --example standalone_dsp --features standalone
//! ```

use std::time::Instant;

use pipeline_dsp::pipeline_dsp::config::DspConfig;
use pipeline_dsp::pipeline_dsp::pipeline::DspPipeline;

fn main() {
    // --- Génération du signal : sinus 440 Hz, 3 s @ 16 kHz ---
    let sample_rate = 16_000usize;
    let duration_s = 3usize;
    let n_samples = sample_rate * duration_s;
    let samples: Vec<f32> = (0..n_samples)
        .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sample_rate as f32).sin())
        .collect();

    println!(
        "Signal : {} samples ({} s @ {} Hz)",
        n_samples, duration_s, sample_rate
    );

    // --- Pipeline DSP ---
    let config = DspConfig::default();
    let mut pipeline = DspPipeline::new(config).expect("DspPipeline::new doit réussir");

    let t0 = Instant::now();
    let matrices = pipeline.process_batch(&samples);
    let elapsed = t0.elapsed();

    let n_matrices = matrices.len();
    println!(
        "Matrices MFCC produites : {}  (temps de traitement : {:.2} ms)",
        n_matrices,
        elapsed.as_secs_f64() * 1_000.0
    );

    if n_matrices == 0 {
        println!("Aucune matrice produite — signal trop court ou config incorrecte.");
        return;
    }

    // --- Affichage de la première matrice ---
    println!("\nPremière matrice MFCC [98 trames × 13 coefficients] :");
    let first = &matrices[0];
    for (i, frame) in first.iter().enumerate().take(5) {
        let coeffs: Vec<String> = frame.iter().map(|v| format!("{:8.3}", v)).collect();
        println!("  trame {:3} : [{}]", i, coeffs.join(", "));
    }
    println!("  ... ({} trames au total)", first.len());

    // --- Validation : aucune valeur NaN/inf ---
    let mut has_invalid = false;
    for m in &matrices {
        for frame in m {
            for &v in frame {
                if !v.is_finite() {
                    eprintln!("ERREUR : valeur non finie détectée : {}", v);
                    has_invalid = true;
                }
            }
        }
    }
    if !has_invalid {
        println!("\nValidation : toutes les valeurs MFCC sont finies (pas de NaN / inf). OK");
    }

    // --- Statistiques ---
    let latency_per_matrix_ms = if n_matrices > 0 {
        elapsed.as_secs_f64() * 1_000.0 / n_matrices as f64
    } else {
        0.0
    };
    println!(
        "\nStatistiques :\n  Matrices     : {}\n  Durée totale : {:.2} ms\n  Latence/mat  : {:.3} ms",
        n_matrices,
        elapsed.as_secs_f64() * 1_000.0,
        latency_per_matrix_ms
    );
}
