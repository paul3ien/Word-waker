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
use tracing_subscriber::{fmt, EnvFilter};

fn main() -> anyhow::Result<()> {
    // Initialisation du logging
    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();

    let config = UiConfig::from_env();
    tracing::info!(socket_path = %config.socket_path, "Démarrage de Word Waker UI");
    tracing::info!("Configuration chargée : {:?}", std::env::args());

    // Placeholder — sera remplacé par l'initialisation NSApplication + IpcClient
    println!("Word Waker UI — en développement");
    println!("Socket cible : {}", config.socket_path);
    println!("Voir ui/stack.md et ui/backlog.md pour le plan d'implémentation.");

    Ok(())
}
