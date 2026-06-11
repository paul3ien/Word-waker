//! Configuration de l'UI Word Waker.
//!
//! Gère les paramètres de connexion au daemon et de comportement de l'UI,
//! lisibles depuis les variables d'environnement.

/// Configuration de l'UI Word Waker.
#[derive(Clone, Debug)]
pub struct UiConfig {
    /// Chemin du socket IPC (défaut `/tmp/wakeword_daemon.sock`)
    pub socket_path: String,
    /// Délai entre tentatives de reconnexion, en millisecondes (défaut `2000`)
    pub reconnect_delay_ms: u64,
    /// Intervalle de polling des événements, en millisecondes (défaut `100`)
    pub poll_interval_ms: u64,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            socket_path: "/tmp/wakeword_daemon.sock".to_string(),
            reconnect_delay_ms: 2000,
            poll_interval_ms: 100,
        }
    }
}

impl UiConfig {
    /// Construit une configuration à partir des variables d'environnement,
    /// avec des valeurs par défaut pour celles non définies.
    pub fn from_env() -> Self {
        let mut config = Self::default();
        if let Ok(path) = std::env::var("WAKEWORD_SOCKET_PATH") {
            config.socket_path = path;
        }
        if let Ok(delay) = std::env::var("WAKEWORD_UI_RECONNECT_MS") {
            if let Ok(parsed) = delay.parse() {
                config.reconnect_delay_ms = parsed;
            }
        }
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = UiConfig::default();
        assert_eq!(config.socket_path, "/tmp/wakeword_daemon.sock");
        assert_eq!(config.reconnect_delay_ms, 2000);
        assert_eq!(config.poll_interval_ms, 100);
    }

    #[test]
    fn test_from_env_empty_uses_defaults() {
        // Sauvegarde et nettoyage des variables
        std::env::remove_var("WAKEWORD_SOCKET_PATH");
        std::env::remove_var("WAKEWORD_UI_RECONNECT_MS");

        let config = UiConfig::from_env();
        assert_eq!(config.socket_path, "/tmp/wakeword_daemon.sock");
        assert_eq!(config.reconnect_delay_ms, 2000);
    }

    #[test]
    fn test_from_env_custom_socket_path() {
        std::env::set_var("WAKEWORD_SOCKET_PATH", "/tmp/custom.sock");
        std::env::remove_var("WAKEWORD_UI_RECONNECT_MS");

        let config = UiConfig::from_env();
        assert_eq!(config.socket_path, "/tmp/custom.sock");

        std::env::remove_var("WAKEWORD_SOCKET_PATH");
    }

    #[test]
    fn test_from_env_custom_reconnect_delay() {
        std::env::remove_var("WAKEWORD_SOCKET_PATH");
        std::env::set_var("WAKEWORD_UI_RECONNECT_MS", "5000");

        let config = UiConfig::from_env();
        assert_eq!(config.reconnect_delay_ms, 5000);

        std::env::remove_var("WAKEWORD_UI_RECONNECT_MS");
    }
}
