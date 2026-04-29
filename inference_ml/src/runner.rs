use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Instant;

use crossbeam_channel::{bounded, Receiver, Sender};

use crate::error::InferenceError;
use crate::model::CoreMLModel;

/// Thread d'inférence dédié.
///
/// Reçoit des matrices MFCC via `rx`, appelle `CoreMLModel::infer` et envoie
/// le score via `tx`. La boucle s'arrête proprement quand :
/// - le `Sender` côté producteur est fermé (rx.recv() retourne Err), OU
/// - `stop()` est appelé (envoie un signal via un canal interne).
pub struct InferenceRunner {
    model: Arc<CoreMLModel>,
    running: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
    /// Canal de stop interne : envoyer () débloque le thread depuis recv().
    stop_tx: Option<Sender<()>>,
}

impl InferenceRunner {
    /// Crée un runner sans le démarrer.
    pub fn new(model: CoreMLModel) -> Self {
        InferenceRunner {
            model: Arc::new(model),
            running: Arc::new(AtomicBool::new(false)),
            thread_handle: None,
            stop_tx: None,
        }
    }

    /// Démarre la boucle d'inférence dans un thread dédié.
    ///
    /// `rx` : canal d'entrée de matrices MFCC.
    /// `tx` : canal de sortie pour les scores.
    ///
    /// Retourne `Err` si le runner est déjà démarré.
    pub fn start(
        &mut self,
        rx: Receiver<[[f32; 13]; 98]>,
        tx: Sender<f32>,
    ) -> Result<(), InferenceError> {
        if self.running.load(Ordering::SeqCst) {
            return Err(InferenceError::InferenceFailed(
                "runner déjà démarré".into(),
            ));
        }

        self.running.store(true, Ordering::SeqCst);

        // Canal de signal de stop (capacité 1 — non-bloquant côté émetteur).
        let (stop_tx, stop_rx) = bounded::<()>(1);
        self.stop_tx = Some(stop_tx);

        let model = Arc::clone(&self.model);
        let running = Arc::clone(&self.running);

        let handle = thread::Builder::new()
            .name("inference-runner".into())
            .spawn(move || {
                tracing::debug!("inference-runner : démarré");

                loop {
                    // select! : attend soit une matrice, soit le signal de stop,
                    // soit la fermeture du canal d'entrée — sans polling.
                    crossbeam_channel::select! {
                        recv(rx) -> msg => {
                            match msg {
                                Ok(mfcc) => {
                                    let t0 = Instant::now();
                                    match model.infer(&mfcc) {
                                        Ok(score) => {
                                            let latency_us = t0.elapsed().as_micros();
                                            tracing::debug!(
                                                score,
                                                latency_us,
                                                "inference-runner : score calculé"
                                            );
                                            let _ = tx.send(score);
                                        }
                                        Err(e) => {
                                            tracing::warn!(
                                                "inference-runner : erreur d'inférence — {e}"
                                            );
                                        }
                                    }
                                }
                                Err(_) => {
                                    tracing::debug!(
                                        "inference-runner : canal d'entrée fermé, arrêt"
                                    );
                                    break;
                                }
                            }
                        }
                        recv(stop_rx) -> _ => {
                            tracing::debug!("inference-runner : signal de stop reçu");
                            break;
                        }
                    }

                    if !running.load(Ordering::SeqCst) {
                        break;
                    }
                }

                running.store(false, Ordering::SeqCst);
                tracing::debug!("inference-runner : arrêté");
            })
            .map_err(|e| InferenceError::LoadFailed(e.to_string()))?;

        self.thread_handle = Some(handle);
        Ok(())
    }

    /// Arrête la boucle et attend la fin du thread.
    /// Sans effet si le runner n'est pas démarré.
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        // Envoie le signal de stop pour débloquer le recv() éventuel.
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for InferenceRunner {
    fn drop(&mut self) {
        self.stop();
    }
}
