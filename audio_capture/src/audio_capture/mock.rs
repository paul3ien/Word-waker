//! Module de mock audio (activé par la feature `mock_audio`).
//!
//! Fournit un ring buffer pré-rempli avec un signal sinusoïdal synthétique,
//! utilisé pour les tests de régression sans microphone physique.

use crossbeam::queue::ArrayQueue;
use std::f32::consts::TAU;
use std::sync::Arc;

/// Fréquence du signal sinusoïdal synthétique (Hz).
pub const MOCK_FREQ_HZ: f32 = 440.0;
/// Sample rate de référence pour la génération du signal.
pub const MOCK_SAMPLE_RATE: f32 = 16_000.0;

/// Génère `n_samples` échantillons d'un signal sinusoïdal à `freq_hz` Hz,
/// normalisé dans [-1.0, 1.0].
pub fn generate_sine(freq_hz: f32, sample_rate: f32, n_samples: usize) -> Vec<f32> {
    (0..n_samples)
        .map(|i| (TAU * freq_hz * i as f32 / sample_rate).sin())
        .collect()
}

/// Crée un `ArrayQueue<f32>` pré-rempli avec `n_samples` échantillons
/// du signal sinusoïdal de référence.
///
/// Utilisé dans les tests mock pour simuler un producteur RT.
pub fn filled_mock_queue(n_samples: usize) -> Arc<ArrayQueue<f32>> {
    let queue = Arc::new(ArrayQueue::new(n_samples));
    for sample in generate_sine(MOCK_FREQ_HZ, MOCK_SAMPLE_RATE, n_samples) {
        // On ignore les éventuels échecs (queue pleine) — impossibles ici car
        // capacity == n_samples.
        let _ = queue.push(sample);
    }
    queue
}
