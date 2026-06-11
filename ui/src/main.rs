//! Word Waker UI — macOS menu bar application.
//!
//! Se connecte au daemon word-waker via Unix Domain Socket et affiche
//! l'état du pipeline de détection de mot-clé en temps réel dans la barre de menu.
//!
//! # Architecture
//!
//! ```text
//! daemon (socket server)  ──IPC──▶  ui (socket client)
//!                                       │
//!                                  NSStatusBar
//!                                  (emoji 🎤/⏸)
//! ```
//!
//! # Utilisation
//!
//! ```bash
//! # Lancer l'UI (le daemon doit déjà tourner)
//! cargo run --release -p ui
//!
//! # Avec un socket personnalisé
//! WAKEWORD_SOCKET_PATH=/tmp/mon_socket.sock cargo run --release -p ui
//! ```

mod app;
mod config;
mod delegate;
mod error;
mod socket_client;
mod status_item;

use config::UiConfig;
use icrate::Foundation::MainThreadMarker;
use socket_client::{IpcClient, UiEvent};
use tracing_subscriber::{fmt, EnvFilter};

fn main() -> anyhow::Result<()> {
    // Initialisation du logging
    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();

    let mtm = MainThreadMarker::new()
        .ok_or_else(|| anyhow::anyhow!("L'UI doit être lancée depuis le main thread"))?;

    let config = UiConfig::from_env();
    tracing::info!(
        socket_path = %config.socket_path,
        "Démarrage de Word Waker UI"
    );

    // Créer le channel de communication socket → UI
    let (tx_event, rx_event) = crossbeam_channel::unbounded::<UiEvent>();

    // Créer l'application UI (doit être sur le main thread)
    let app = app::UiApp::new(config.clone(), rx_event, mtm)?;

    // Enregistrer l'AppDelegate pour les callbacks de cycle de vie
    let _delegate = delegate::AppDelegate::new()?;

    // Lancer le thread client socket
    let socket_config = config;
    std::thread::Builder::new()
        .name("ipc-client".into())
        .spawn(move || {
            let client = IpcClient::new(socket_config);
            client.run(tx_event);
        })?;

    tracing::info!("Thread socket démarré, lancement de la boucle AppKit");
    tracing::info!("UI démarrée — applicationDidFinishLaunching");

    // Boucle principale AppKit (bloquant)
    app.run();

    tracing::info!("UI arrêtée — applicationWillTerminate");
    tracing::info!("Word Waker UI terminé");
    Ok(())
}
