//! Thread consommateur : lit le ring buffer périodiquement et envoie les batches
//! via un `Sender<Vec<f32>>` standard.

use crossbeam::queue::ArrayQueue;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::audio_capture::error::AudioCaptureError;
use crate::audio_capture::ring_buffer::drain_available;

/// Consommateur périodique du ring buffer.
///
/// Tourne dans son propre thread et envoie les batches de samples disponibles
/// au `Sender` fourni lors du démarrage. Le thread s'arrête proprement dès
/// que `stop()` est appelé ou que la struct est droppée.
pub struct AudioConsumer {
    queue: Arc<ArrayQueue<f32>>,
    poll_interval_ms: u64,
    running: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
}

impl AudioConsumer {
    /// Crée un consommateur pour la `queue` donnée.
    ///
    /// `poll_interval_ms` : intervalle de polling en millisecondes.
    pub fn new(queue: Arc<ArrayQueue<f32>>, poll_interval_ms: u64) -> Self {
        Self {
            queue,
            poll_interval_ms,
            running: Arc::new(AtomicBool::new(false)),
            thread_handle: None,
        }
    }

    /// Démarre le thread de consommation.
    ///
    /// Les batches non vides sont envoyés via `sender`. Un deuxième appel
    /// à `start` sans `stop` intermédiaire retourne une erreur.
    pub fn start(&mut self, sender: Sender<Vec<f32>>) -> Result<(), AudioCaptureError> {
        if self.running.load(Ordering::SeqCst) {
            return Err(AudioCaptureError::UnitStartFailed(-1));
        }

        self.running.store(true, Ordering::SeqCst);

        let queue = Arc::clone(&self.queue);
        let running = Arc::clone(&self.running);
        let interval = self.poll_interval_ms;

        let handle = thread::spawn(move || {
            while running.load(Ordering::Relaxed) {
                let batch = drain_available(&queue);
                if !batch.is_empty() {
                    // Si le receiver a été droppé on sort proprement.
                    if sender.send(batch).is_err() {
                        break;
                    }
                }
                thread::sleep(Duration::from_millis(interval));
            }
        });

        self.thread_handle = Some(handle);
        Ok(())
    }

    /// Arrête le thread de consommation et attend sa terminaison.
    ///
    /// Idempotent : appeler `stop()` plusieurs fois est sûr.
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for AudioConsumer {
    fn drop(&mut self) {
        self.stop();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio_capture::ring_buffer::{push_sample, AudioRingBuffer};
    use std::sync::atomic::AtomicUsize;
    use std::sync::mpsc;
    use std::time::Duration;

    /// Pré-remplir le ring buffer avec des samples connus, démarrer le
    /// consommateur, vérifier que le Receiver reçoit exactement ces valeurs.
    #[test]
    fn consommateur_transmet_samples_connus() {
        let ring = AudioRingBuffer::new(1024);
        let producer = ring.producer_handle();
        let dropped = Arc::new(AtomicUsize::new(0));

        // On pousse 10 samples connus.
        let expected: Vec<f32> = (0..10).map(|i| i as f32 * 0.1).collect();
        for &s in &expected {
            push_sample(&producer, &dropped, s);
        }

        let (tx, rx) = mpsc::channel();
        let mut consumer = AudioConsumer::new(ring.consumer_handle(), 10);
        consumer.start(tx).expect("start failed");

        // On attend qu'au moins un batch arrive.
        let received: Vec<f32> = rx
            .recv_timeout(Duration::from_millis(200))
            .expect("Aucun batch reçu en 200 ms");

        consumer.stop();

        assert_eq!(
            received, expected,
            "Les samples reçus diffèrent de ceux envoyés"
        );
    }

    /// Lancer le consommateur sur un ring buffer vide :
    /// aucun batch vide ne doit être envoyé.
    #[test]
    fn consommateur_buffer_vide_aucun_batch_envoye() {
        let ring = AudioRingBuffer::new(1024);
        let (tx, rx) = mpsc::channel();
        let mut consumer = AudioConsumer::new(ring.consumer_handle(), 10);
        consumer.start(tx).expect("start failed");

        // On laisse le consommateur tourner quelques cycles.
        std::thread::sleep(Duration::from_millis(50));
        consumer.stop();

        // Aucun message ne doit avoir été envoyé.
        assert!(
            rx.try_recv().is_err(),
            "Le consommateur a envoyé un batch alors que le ring buffer était vide"
        );
    }

    /// Double stop() : idempotent, pas de panic.
    #[test]
    fn consommateur_double_stop_idempotent() {
        let ring = AudioRingBuffer::new(1024);
        let (tx, _rx) = mpsc::channel();
        let mut consumer = AudioConsumer::new(ring.consumer_handle(), 10);
        consumer.start(tx).expect("start failed");
        consumer.stop();
        consumer.stop(); // doit être silencieux
    }

    /// Test d'intégration avec le vrai device : capture 500 ms,
    /// tous les batches reçus sont non vides et dans [-1.0, 1.0].
    #[cfg(not(feature = "mock_audio"))]
    #[test]
    fn consommateur_integre_avec_capture_reelle() {
        use crate::audio_capture::{
            config::AudioCaptureConfig, device::get_default_input_device,
            ring_buffer::AudioRingBuffer, unit::AudioUnitCapture,
        };

        let device_id = get_default_input_device().expect("Pas de device");
        let config = AudioCaptureConfig::default();
        let ring = AudioRingBuffer::new(config.ring_capacity);

        let mut unit = AudioUnitCapture::new(device_id, &config).expect("new failed");
        unit.register_input_callback(
            ring.producer_handle(),
            ring.dropped_samples_handle(),
            config.buffer_size_frames as usize,
        )
        .expect("register_input_callback failed");

        let (tx, rx) = mpsc::channel::<Vec<f32>>();
        let mut consumer = AudioConsumer::new(ring.consumer_handle(), 10);
        consumer.start(tx).expect("consumer start failed");
        unit.start().expect("unit start failed");

        std::thread::sleep(Duration::from_millis(500));

        unit.stop().expect("unit stop failed");
        consumer.stop();

        // Collecter tous les batches.
        let mut all_samples: Vec<f32> = Vec::new();
        while let Ok(batch) = rx.try_recv() {
            all_samples.extend(batch);
        }

        assert!(
            !all_samples.is_empty(),
            "Aucun sample reçu après 500 ms de capture"
        );
        for &s in &all_samples {
            assert!(
                (-1.0..=1.0).contains(&s),
                "Sample hors plage [-1.0, 1.0] : {s}"
            );
        }
    }

    /// Stop du consommateur pendant la capture : le thread se termine proprement.
    #[cfg(not(feature = "mock_audio"))]
    #[test]
    fn consommateur_stop_pendant_capture_propre() {
        use crate::audio_capture::{
            config::AudioCaptureConfig, device::get_default_input_device,
            ring_buffer::AudioRingBuffer, unit::AudioUnitCapture,
        };

        let device_id = get_default_input_device().expect("Pas de device");
        let config = AudioCaptureConfig::default();
        let ring = AudioRingBuffer::new(config.ring_capacity);

        let mut unit = AudioUnitCapture::new(device_id, &config).expect("new failed");
        unit.register_input_callback(
            ring.producer_handle(),
            ring.dropped_samples_handle(),
            config.buffer_size_frames as usize,
        )
        .expect("register_input_callback failed");

        let (tx, _rx) = mpsc::channel::<Vec<f32>>();
        let mut consumer = AudioConsumer::new(ring.consumer_handle(), 10);
        consumer.start(tx).expect("consumer start failed");
        unit.start().expect("unit start failed");

        std::thread::sleep(Duration::from_millis(100));

        // On arrête le consommateur AVANT l'unité — le thread doit se terminer proprement.
        consumer.stop();
        unit.stop().expect("unit stop failed");
        // Si on arrive ici sans hang ni panic, le test passe.
    }
}
