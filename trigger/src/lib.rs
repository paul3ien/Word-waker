//! # trigger
//!
//! Module de déclenchement wake-word avec vote glissant anti-faux-positifs et
//! notification via Unix Domain Socket.
//!
//! ## Composants principaux
//!
//! - [`TriggerConfig`] — paramètres configurables (seuils, fenêtre, cooldown, socket)
//! - [`TriggerEngine`] — moteur de vote glissant (pur, sans I/O)
//! - [`IpcNotifier`] — envoi de notification sur Unix Domain Socket
//! - [`TriggerRunner`] — thread trigger (boucle bloquante sur `Receiver<f32>`)
//! - [`TriggerModule`] — façade publique à utiliser depuis le daemon
//! - [`TriggerError`] — type d'erreur du module
pub mod trigger;

pub use trigger::{
    IpcNotifier, TriggerConfig, TriggerEngine, TriggerError, TriggerModule, TriggerRunner,
};
