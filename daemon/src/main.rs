//! Word Waker — Daemon de détection de mot-clé en temps réel.
//!
//! # Utilisation
//!
//! ```bash
//! WAKEWORD_MODEL_PATH=/chemin/vers/WakeWord.mlmodelc word-waker
//! ```
//!
//! Variables d'environnement disponibles :
//!
//! | Variable               | Défaut                      | Description                          |
//! |------------------------|-----------------------------|--------------------------------------|
//! | `WAKEWORD_MODEL_PATH`  | *(obligatoire)*             | Chemin vers le modèle `.mlmodelc`    |
//! | `WAKEWORD_SOCKET_PATH` | `/tmp/wakeword_daemon.sock` | Chemin du socket IPC                 |
//! | `WAKEWORD_THRESHOLD`   | `0.80`                      | Seuil de score individuel            |
//! | `WAKEWORD_COOLDOWN_MS` | `2000`                      | Délai entre deux détections (ms)     |

mod config;
mod pipeline;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use tracing::{info, Level};

fn main() -> Result<()> {
    // Initialisation des logs (niveau INFO par défaut, surchargeable via RUST_LOG)
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Lecture de la configuration depuis l'environnement
    let config = config::DaemonConfig::from_env()?;

    info!("╔══════════════════════════════════════╗");
    info!("║       Word Waker daemon démarré      ║");
    info!("╚══════════════════════════════════════╝");
    info!("Modèle  : {}", config.model_path);
    info!("Socket  : {}", config.socket_path);
    info!("Seuil   : {:.2}", config.score_threshold);
    info!("Cooldown: {} ms", config.cooldown_ms);

    // Flag d'arrêt partagé avec le handler de signal
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_signal = Arc::clone(&shutdown);

    ctrlc::set_handler(move || {
        shutdown_signal.store(true, Ordering::Relaxed);
    })
    .expect("Impossible d'installer le handler SIGINT/SIGTERM");

    // Démarrage du pipeline complet
    let handles = pipeline::start_pipeline(&config)?;
    info!("Pipeline démarré — en attente de détections...");
    info!("(Ctrl+C pour arrêter)");

    // Boucle principale — attend le signal d'arrêt
    while !shutdown.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(100));
    }

    info!("Signal d'arrêt reçu — fermeture du pipeline...");
    pipeline::shutdown(handles);
    info!("Arrêt propre — au revoir.");

    Ok(())
}
