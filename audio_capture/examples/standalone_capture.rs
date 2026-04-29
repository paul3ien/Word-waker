//! Exemple standalone : capture N secondes (défaut : 3), affiche les statistiques.
//!
//! Lancer avec :
//! ```sh
//! cargo run --example standalone_capture --features standalone -p audio_capture
//! # Ou pour 30 secondes (Instruments) :
//! cargo run --example standalone_capture --features standalone -p audio_capture -- 30
//! ```

use audio_capture::{AudioCapture, AudioCaptureConfig};
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn main() {
    // Durée passée en premier argument (défaut 3 s).
    let secs: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(3);

    let config = AudioCaptureConfig::default();
    println!(
        "Configuration : {:.0} Hz, buffer {} frames, ring {} samples",
        config.sample_rate, config.buffer_size_frames, config.ring_capacity
    );

    let (tx, rx) = mpsc::channel::<Vec<f32>>();

    let mut cap = AudioCapture::new(config).expect("Impossible d'initialiser la capture audio");
    cap.start(tx).expect("Impossible de démarrer la capture");

    println!("Capture en cours… ({} secondes)", secs);
    let start = Instant::now();

    let mut total_samples = 0usize;
    let mut total_batches = 0usize;
    let mut out_of_range = 0usize;

    // On draine le receiver pendant `secs` secondes.
    while start.elapsed() < Duration::from_secs(secs) {
        while let Ok(batch) = rx.try_recv() {
            for &s in &batch {
                if !(-1.0..=1.0).contains(&s) {
                    out_of_range += 1;
                }
            }
            total_samples += batch.len();
            total_batches += 1;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    cap.stop().expect("Erreur lors de l'arrêt de la capture");

    // Vider le reste du channel après stop.
    while let Ok(batch) = rx.try_recv() {
        total_samples += batch.len();
        total_batches += 1;
    }

    let dropped = cap.dropped_samples();
    let total_produced = total_samples + dropped;
    let drop_rate = if total_produced > 0 {
        dropped as f64 / total_produced as f64 * 100.0
    } else {
        0.0
    };

    println!("─────────────────────────────────");
    println!("Samples reçus   : {total_samples}");
    println!("Batches reçus   : {total_batches}");
    println!("Samples perdus  : {dropped}  ({drop_rate:.3} %)");
    println!("Hors [-1, 1]    : {out_of_range}");
    println!("─────────────────────────────────");

    assert!(
        total_samples > 0,
        "Aucun sample reçu — vérifier le microphone"
    );
    assert_eq!(out_of_range, 0, "Des samples sont hors plage [-1.0, 1.0]");
    println!("OK — statistiques cohérentes.");
}
