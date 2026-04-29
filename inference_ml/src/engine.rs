use crossbeam_channel::{Receiver, Sender};

use crate::config::InferenceConfig;
use crate::error::InferenceError;
use crate::model::CoreMLModel;
use crate::runner::InferenceRunner;

/// Façade publique du module d'inférence.
///
/// Regroupe la configuration, le modèle chargé et le thread d'inférence
/// derrière une API simple : `new → start → (envoyer des matrices) → stop`.
pub struct InferenceEngine {
    runner: InferenceRunner,
    #[allow(dead_code)]
    config: InferenceConfig,
}

impl InferenceEngine {
    /// Valide la config, charge le modèle CoreML et crée le runner.
    ///
    /// Retourne `Err` si la config est invalide ou si le chargement échoue.
    pub fn new(config: InferenceConfig) -> Result<Self, InferenceError> {
        config.validate()?;
        let model = CoreMLModel::load(&config)?;
        let runner = InferenceRunner::new(model);
        Ok(InferenceEngine { runner, config })
    }

    /// Démarre la boucle d'inférence.
    ///
    /// `rx` : canal d'entrée de matrices MFCC `[[f32;13];98]`.
    /// `tx` : canal de sortie pour les scores `f32` ∈ [0.0, 1.0].
    pub fn start(
        &mut self,
        rx: Receiver<[[f32; 13]; 98]>,
        tx: Sender<f32>,
    ) -> Result<(), InferenceError> {
        self.runner.start(rx, tx)
    }

    /// Arrête la boucle d'inférence et attend la fin du thread.
    pub fn stop(&mut self) -> Result<(), InferenceError> {
        self.runner.stop();
        Ok(())
    }
}

impl Drop for InferenceEngine {
    fn drop(&mut self) {
        // Ignore l'éventuelle erreur (runner déjà stoppé = sans effet).
        self.runner.stop();
    }
}
