//! Thread DSP runner : lit des batches de samples depuis un channel,
//! appelle `DspPipeline::process_batch` et envoie les matrices MFCC produites.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender};

use crate::pipeline_dsp::config::DspConfig;
use crate::pipeline_dsp::error::DspError;
use crate::pipeline_dsp::pipeline::DspPipeline;

// ---------------------------------------------------------------------------
// DspRunner
// ---------------------------------------------------------------------------

/// Runner de thread DSP.
///
/// Reçoit des batches de samples PCM depuis `rx`, les traite via `DspPipeline`
/// et envoie les matrices MFCC résultantes via `tx`.
///
/// Le thread s'arrête proprement quand :
/// - `stop()` est appelé, ou
/// - le `Sender` associé à `rx` est fermé (tous les senders droppés).
pub struct DspRunner {
    running: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
}

impl DspRunner {
    /// Démarre le thread DSP.
    ///
    /// Crée un `DspPipeline` à partir de `config`, puis spawne un thread qui :
    /// 1. Attend des batches sur `rx`
    /// 2. Appelle `process_batch` pour chaque batch reçu
    /// 3. Envoie chaque matrice MFCC produite via `tx`
    ///
    /// # Erreurs
    /// Retourne `DspError` si la construction du `DspPipeline` échoue.
    pub fn start(
        config: DspConfig,
        rx: Receiver<Vec<f32>>,
        tx: Sender<[[f32; 13]; 98]>,
    ) -> Result<Self, DspError> {
        let mut pipeline = DspPipeline::new(config)?;
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);

        let handle = thread::spawn(move || {
            loop {
                if !running_clone.load(Ordering::Relaxed) {
                    break;
                }
                match rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(samples) => {
                        let matrices = pipeline.process_batch(&samples);
                        for m in matrices {
                            if tx.send(m).is_err() {
                                // Receiver fermé — arrêter silencieusement
                                return;
                            }
                        }
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                    Err(RecvTimeoutError::Timeout) => {
                        // Vérification périodique du flag running
                        continue;
                    }
                }
            }
        });

        Ok(Self {
            running,
            thread_handle: Some(handle),
        })
    }

    /// Demande l'arrêt du thread et attend sa terminaison.
    ///
    /// Idempotent : appels successifs sont sans effet.
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for DspRunner {
    fn drop(&mut self) {
        self.stop();
    }
}

// ---------------------------------------------------------------------------
// Tests d'intégration
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crossbeam_channel::RecvTimeoutError;

    use super::DspRunner;
    use crate::pipeline_dsp::config::DspConfig;

    /// Génère `n` samples d'un sinus à `freq` Hz, `sr` Hz de taux d'échantillonnage.
    fn sine_wave(freq: f32, sr: usize, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sr as f32).sin())
            .collect()
    }

    #[test]
    fn send_3s_sine_produces_at_least_2_matrices() {
        let cfg = DspConfig::default();
        let (data_tx, data_rx) = crossbeam_channel::unbounded::<Vec<f32>>();
        let (result_tx, result_rx) = crossbeam_channel::unbounded::<[[f32; 13]; 98]>();

        let runner = DspRunner::start(cfg, data_rx, result_tx).unwrap();

        // 3 secondes @ 16 kHz = 48 000 samples
        // Avec hop_size=160 → ~300 trames → ~203 matrices
        let samples = sine_wave(440.0, 16_000, 48_000);
        data_tx.send(samples).unwrap();
        drop(data_tx); // signaler la fin — le thread se terminera

        let mut count = 0;
        loop {
            match result_rx.recv_timeout(Duration::from_secs(5)) {
                Ok(_) => count += 1,
                Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => break,
            }
        }
        drop(runner);

        assert!(count >= 2, "Attendu ≥ 2 matrices, reçu {}", count);
    }

    #[test]
    fn sender_closed_thread_exits_cleanly() {
        let cfg = DspConfig::default();
        let (data_tx, data_rx) = crossbeam_channel::unbounded::<Vec<f32>>();
        let (result_tx, result_rx) = crossbeam_channel::unbounded::<[[f32; 13]; 98]>();

        let mut runner = DspRunner::start(cfg, data_rx, result_tx).unwrap();

        // Fermer le sender immédiatement → le thread doit s'arrêter sans panique
        drop(data_tx);
        drop(result_rx);

        // stop() doit compléter sans deadlock
        runner.stop();
        // Pas de panique = succès
    }

    #[test]
    fn drop_without_explicit_stop_no_zombie() {
        let cfg = DspConfig::default();
        let (data_tx, data_rx) = crossbeam_channel::unbounded::<Vec<f32>>();
        let (result_tx, _result_rx) = crossbeam_channel::unbounded::<[[f32; 13]; 98]>();

        {
            let _runner = DspRunner::start(cfg, data_rx, result_tx).unwrap();
            // Fermer le sender pour permettre au thread de se terminer
            drop(data_tx);
            // _runner est dropé ici → Drop::drop → stop() → join()
        }
        // Si on arrive ici sans blocage, pas de thread zombie
    }
}
