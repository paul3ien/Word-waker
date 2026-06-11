//! AppDelegate pour l'application Word Waker.
//!
//! Gère les callbacks du cycle de vie de NSApplication :
//! - `applicationDidFinishLaunching`
//! - `applicationWillTerminate`
//!
//! Pour le POC, le delegate se contente de logger les événements
//! du cycle de vie via des messages simples.

use icrate::Foundation::MainThreadMarker;
use tracing;

use crate::error::UiError;

/// Délégué de l'application NSApplication.
///
/// Pour le POC, on utilise une approche minimaliste : on n'enregistre pas
/// de delegate formel, on logge juste les événements depuis main().
/// Le vrai delegate sera implémenté dans P3 quand on aura besoin de cleanup.
pub struct AppDelegate;

impl AppDelegate {
    /// Enregistre les callbacks de cycle de vie.
    /// Pour le POC, cette fonction est un placeholder — le logging
    /// du cycle de vie se fera directement dans main().
    pub fn new() -> Result<Self, UiError> {
        tracing::info!("AppDelegate enregistré (mode POC — logging dans main)");
        Ok(Self)
    }
}
