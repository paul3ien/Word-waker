//! Câblage et gestion du cycle de vie du pipeline complet.
//!
//! Assemble les 4 crates via des channels crossbeam bornés (capacité 8)
//! et expose deux fonctions : `start_pipeline` et `shutdown`.
//!
//! Ordre de démarrage (consommateur → producteur) :
//! 1. `TriggerModule::start(rx_score)`
//! 2. `InferenceEngine::start(rx_mfcc, tx_score)`
//! 3. `DspRunner::start(rx_pcm, tx_mfcc)` (spawne son thread)
//! 4. `AudioCapture::start(tx_pcm)`
//!
//! Ordre d'arrêt (producteur → consommateur) :
//! 1. `AudioCapture::stop()` — ferme `tx_pcm`
//! 2. `DspRunner::stop()`
//! 3. `InferenceEngine::stop()`
//! 4. `TriggerModule::stop()`

use anyhow::{Context, Result};
use crossbeam_channel::bounded;
use tracing::{info, warn};

use audio_capture::{AudioCapture, AudioCaptureConfig};
use inference_ml::{InferenceConfig, InferenceEngine};
use pipeline_dsp::pipeline_dsp::{config::DspConfig, runner::DspRunner};
use trigger::{TriggerConfig, TriggerModule};

use crate::config::DaemonConfig;

/// Thread de pont entre `std::sync::mpsc` (AudioCapture) et `crossbeam_channel` (DspRunner).
///
/// `AudioCapture::start` exige un `std::sync::mpsc::Sender`. On fait suivre
/// chaque batch vers le `crossbeam_channel::Sender` attendu par `DspRunner`.
fn spawn_pcm_bridge(
    mpsc_rx: std::sync::mpsc::Receiver<Vec<f32>>,
    cb_tx: crossbeam_channel::Sender<Vec<f32>>,
) {
    std::thread::Builder::new()
        .name("pcm-bridge".into())
        .spawn(move || {
            for batch in &mpsc_rx {
                if cb_tx.send(batch).is_err() {
                    break;
                }
            }
        })
        .expect("spawn pcm-bridge");
}

/// Handles vers les modules actifs du pipeline.
pub struct PipelineHandles {
    capture: AudioCapture,
    dsp: DspRunner,
    engine: InferenceEngine,
    trigger: TriggerModule,
}

/// Initialise et démarre tous les modules du pipeline.
///
/// Les channels bornés (capacité 8) absorbent les micro-variations de latence
/// sans bloquer le thread producteur en régime permanent.
pub fn start_pipeline(config: &DaemonConfig) -> Result<PipelineHandles> {
    // Channels inter-étages
    // PCM : std::sync::mpsc (exigé par AudioCapture) → bridge → crossbeam (exigé par DspRunner)
    let (tx_pcm_mpsc, rx_pcm_mpsc) = std::sync::mpsc::channel::<Vec<f32>>();
    let (tx_pcm_cb, rx_pcm_cb) = bounded::<Vec<f32>>(8);
    let (tx_mfcc, rx_mfcc) = bounded::<[[f32; 13]; 98]>(8);
    let (tx_score, rx_score) = bounded::<f32>(8);

    // 1. Trigger (consommateur terminal)
    let trigger_config = TriggerConfig {
        socket_path: config.socket_path.clone(),
        score_threshold: config.score_threshold,
        cooldown_ms: config.cooldown_ms,
        ..TriggerConfig::default()
    };
    let mut trigger = TriggerModule::new(trigger_config).context("Initialisation TriggerModule")?;
    trigger.start(rx_score).context("Démarrage TriggerModule")?;
    info!("TriggerModule démarré — socket : {}", config.socket_path);

    // 2. Inférence CoreML
    let inference_config = InferenceConfig {
        model_path: config.model_path.clone(),
        ..InferenceConfig::default()
    };
    let mut engine =
        InferenceEngine::new(inference_config).context("Chargement du modèle CoreML")?;
    engine
        .start(rx_mfcc, tx_score)
        .context("Démarrage InferenceEngine")?;
    info!("InferenceEngine démarré — modèle : {}", config.model_path);

    // 3. Pipeline DSP (spawne son propre thread dans start())
    let dsp = DspRunner::start(DspConfig::default(), rx_pcm_cb, tx_mfcc)
        .context("Démarrage DspRunner")?;
    info!("DspRunner démarré");

    // 4. Capture audio (dernière — commence à produire des samples)
    //    Pont mpsc → crossbeam démarré avant la capture.
    spawn_pcm_bridge(rx_pcm_mpsc, tx_pcm_cb);
    let mut capture =
        AudioCapture::new(AudioCaptureConfig::default()).context("Initialisation AudioCapture")?;
    capture.start(tx_pcm_mpsc).context("Démarrage AudioCapture")?;
    info!("AudioCapture démarrée — 16 kHz Float32 mono");

    Ok(PipelineHandles {
        capture,
        dsp,
        engine,
        trigger,
    })
}

/// Arrête proprement tous les modules dans l'ordre inverse du démarrage.
///
/// Chaque erreur est loggée mais n'interrompt pas les étapes suivantes.
pub fn shutdown(mut handles: PipelineHandles) {
    info!("Arrêt du pipeline en cours...");

    // 1. Arrêt de la capture — ferme tx_pcm → DspRunner se terminera seul
    if let Err(e) = handles.capture.stop() {
        warn!("AudioCapture::stop() : {e}");
    } else {
        info!("AudioCapture arrêtée");
    }

    // 2. Arrêt du DSP
    handles.dsp.stop();
    info!("DspRunner arrêté");

    // 3. Arrêt de l'inférence
    if let Err(e) = handles.engine.stop() {
        warn!("InferenceEngine::stop() : {e}");
    } else {
        info!("InferenceEngine arrêté");
    }

    // 4. Arrêt du trigger
    if let Err(e) = handles.trigger.stop() {
        warn!("TriggerModule::stop() : {e}");
    } else {
        info!("TriggerModule arrêté");
    }
}
