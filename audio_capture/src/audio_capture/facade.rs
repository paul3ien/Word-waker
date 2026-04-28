//! Façade publique du module `audio_capture`.
//!
//! `AudioCapture` orchestre l'ensemble des composants internes :
//! device CoreAudio → `AudioUnitCapture` → `AudioRingBuffer` → `AudioConsumer`.
//!
//! # Exemple minimal
//!
//! ```no_run
//! use audio_capture::{AudioCapture, AudioCaptureConfig};
//! use std::sync::mpsc;
//!
//! let (tx, rx) = mpsc::channel();
//! let mut cap = AudioCapture::new(AudioCaptureConfig::default()).unwrap();
//! cap.start(tx).unwrap();
//! // … lire rx dans un autre thread …
//! cap.stop().unwrap();
//! ```

use std::sync::mpsc::Sender;

use crate::audio_capture::{
    config::AudioCaptureConfig, consumer::AudioConsumer, device::get_default_input_device,
    error::AudioCaptureError, ring_buffer::AudioRingBuffer, unit::AudioUnitCapture,
};

/// Point d'entrée unique pour la capture audio.
///
/// Agrège un `AudioUnitCapture` (HAL CoreAudio), un `AudioRingBuffer` et un
/// `AudioConsumer`. Les samples PCM Float32 mono 16 kHz sont transmis via le
/// `Sender<Vec<f32>>` passé à `start`.
pub struct AudioCapture {
    unit: AudioUnitCapture,
    ring: AudioRingBuffer,
    consumer: AudioConsumer,
}

impl AudioCapture {
    /// Initialise la capture audio avec la configuration donnée.
    ///
    /// - Détecte le device d'entrée par défaut.
    /// - Configure l'AUHAL et le ring buffer.
    /// - Prépare le thread consommateur (non encore démarré).
    pub fn new(config: AudioCaptureConfig) -> Result<Self, AudioCaptureError> {
        config.validate()?;

        let device_id = get_default_input_device()?;
        let ring = AudioRingBuffer::new(config.ring_capacity);

        let mut unit = AudioUnitCapture::new(device_id, &config)?;
        unit.register_input_callback(
            ring.producer_handle(),
            ring.dropped_samples_handle(),
            config.buffer_size_frames as usize,
        )?;

        let consumer = AudioConsumer::new(ring.consumer_handle(), 10);

        Ok(Self {
            unit,
            ring,
            consumer,
        })
    }

    /// Démarre la capture et le thread consommateur.
    ///
    /// Les batches de samples sont envoyés via `sender`. Appeler `start` deux
    /// fois sans `stop` intermédiaire retourne une erreur.
    pub fn start(&mut self, sender: Sender<Vec<f32>>) -> Result<(), AudioCaptureError> {
        self.consumer.start(sender)?;
        self.unit.start()?;
        Ok(())
    }

    /// Arrête la capture proprement.
    ///
    /// L'ordre est important : on arrête d'abord le consommateur (join du
    /// thread), puis l'unité HAL. Idempotent.
    pub fn stop(&mut self) -> Result<(), AudioCaptureError> {
        self.consumer.stop();
        self.unit.stop()?;
        Ok(())
    }

    /// Retourne le nombre de samples perdus depuis le démarrage.
    pub fn dropped_samples(&self) -> usize {
        self.ring.dropped_count()
    }
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        // On ignore l'erreur éventuelle de stop() pour ne pas paniquer dans Drop.
        let _ = self.stop();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    /// Cycle complet new → start → 1 s → stop : samples valides reçus.
    #[cfg(not(feature = "mock_audio"))]
    #[test]
    fn facade_cycle_complet_new_start_stop() {
        let config = AudioCaptureConfig::default();
        let (tx, rx) = mpsc::channel::<Vec<f32>>();

        let mut cap = AudioCapture::new(config).expect("new failed");
        cap.start(tx).expect("start failed");

        std::thread::sleep(Duration::from_millis(1000));

        cap.stop().expect("stop failed");

        let mut all: Vec<f32> = Vec::new();
        while let Ok(batch) = rx.try_recv() {
            all.extend(batch);
        }

        assert!(!all.is_empty(), "Aucun sample reçu après 1 s de capture");
        for &s in &all {
            assert!(
                (-1.0..=1.0).contains(&s),
                "Sample hors plage [-1.0, 1.0] : {s}"
            );
        }
    }

    /// Deux cycles start/stop successifs : idempotence de la réinitialisation.
    /// NOTE : AudioUnitCapture ne supporte pas le redémarrage à chaud (le proc_id
    /// est lié au device). On vérifie donc que le deuxième `new` + cycle fonctionne.
    #[cfg(not(feature = "mock_audio"))]
    #[test]
    fn facade_deux_instances_successives() {
        let config = AudioCaptureConfig::default();

        for _ in 0..2 {
            let (tx, rx) = mpsc::channel::<Vec<f32>>();
            let mut cap = AudioCapture::new(config.clone()).expect("new failed");
            cap.start(tx).expect("start failed");
            std::thread::sleep(Duration::from_millis(200));
            cap.stop().expect("stop failed");

            let count = {
                let mut n = 0usize;
                while rx.try_recv().is_ok() {
                    n += 1;
                }
                n
            };
            assert!(count > 0, "Aucun batch reçu lors d'un cycle");
        }
    }

    /// Drop sans stop explicite : pas de hang.
    #[cfg(not(feature = "mock_audio"))]
    #[test]
    fn facade_drop_sans_stop_explicite() {
        let config = AudioCaptureConfig::default();
        let (tx, _rx) = mpsc::channel::<Vec<f32>>();
        let mut cap = AudioCapture::new(config).expect("new failed");
        cap.start(tx).expect("start failed");
        std::thread::sleep(Duration::from_millis(100));
        drop(cap); // Drop doit tout nettoyer sans hang.
    }
}
