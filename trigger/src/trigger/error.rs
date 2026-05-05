use std::fmt;

/// Erreurs possibles du module `trigger`.
#[derive(Debug)]
pub enum TriggerError {
    /// Le `Receiver<f32>` a été fermé (l'émetteur a été droppé).
    ChannelClosed,
    /// Échec d'écriture sur le socket Unix après connexion réussie.
    IpcSendFailed(String),
    /// Échec de création / bind du socket Unix côté serveur.
    SocketBindFailed(String),
    /// Paramètres de configuration incohérents.
    InvalidConfig(String),
}

impl fmt::Display for TriggerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TriggerError::ChannelClosed => {
                write!(f, "trigger: le channel de scores a été fermé")
            }
            TriggerError::IpcSendFailed(msg) => {
                write!(f, "trigger: échec d'envoi IPC — {}", msg)
            }
            TriggerError::SocketBindFailed(msg) => {
                write!(f, "trigger: échec de bind du socket — {}", msg)
            }
            TriggerError::InvalidConfig(msg) => {
                write!(f, "trigger: configuration invalide — {}", msg)
            }
        }
    }
}

impl std::error::Error for TriggerError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_messages_are_non_empty_and_distinct() {
        let variants = vec![
            TriggerError::ChannelClosed,
            TriggerError::IpcSendFailed("io error".to_string()),
            TriggerError::SocketBindFailed("addr in use".to_string()),
            TriggerError::InvalidConfig("bad threshold".to_string()),
        ];

        let messages: Vec<String> = variants.iter().map(|e| e.to_string()).collect();

        // Tous non vides
        for msg in &messages {
            assert!(!msg.is_empty(), "Display message should not be empty");
        }

        // Tous distincts
        for i in 0..messages.len() {
            for j in (i + 1)..messages.len() {
                assert_ne!(
                    messages[i], messages[j],
                    "Display messages should be distinct: '{}' vs '{}'",
                    messages[i], messages[j]
                );
            }
        }
    }

    #[test]
    fn trigger_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TriggerError>();
    }
}
