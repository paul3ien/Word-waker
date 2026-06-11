//! Types d'erreur pour l'UI Word Waker.

/// Erreurs spécifiques à l'UI Word Waker.
#[derive(Debug)]
pub enum UiError {
    /// Erreur de connexion au socket IPC du daemon.
    IpcConnection(String),
    /// Erreur liée à la barre de statut macOS.
    StatusBar(String),
    /// Erreur liée à AppKit.
    AppKit(String),
}

impl std::fmt::Display for UiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UiError::IpcConnection(msg) => write!(f, "IPC connection error: {}", msg),
            UiError::StatusBar(msg) => write!(f, "Status bar error: {}", msg),
            UiError::AppKit(msg) => write!(f, "AppKit error: {}", msg),
        }
    }
}

impl std::error::Error for UiError {}
