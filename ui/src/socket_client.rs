//! Client socket IPC pour communiquer avec le daemon Word Waker.
//!
//! Se connecte au daemon via Unix Domain Socket, lit les notifications
//! de détection et les transmet au thread UI principal.

use crate::config::UiConfig;

/// Client socket IPC qui reçoit les événements du daemon.
pub struct IpcClient {
    config: UiConfig,
}

impl IpcClient {
    /// Crée un nouveau client IPC avec la configuration donnée.
    pub fn new(config: UiConfig) -> Self {
        Self { config }
    }

    /// Lance la boucle de connexion et de réception d'événements.
    /// Cette méthode est destinée à être exécutée dans un thread dédié.
    pub fn run(&self) -> anyhow::Result<()> {
        // Sera implémenté dans P1
        Ok(())
    }
}
