//! Configuration runtime du daemon.
//!
//! Lit les variables d'environnement avec des valeurs par défaut sensées.
//! La seule variable obligatoire est `WAKEWORD_MODEL_PATH`.

use anyhow::{bail, Result};

/// Paramètres de configuration du daemon Word Waker.
pub struct DaemonConfig {
    /// Chemin absolu vers le modèle CoreML (`.mlmodelc`). Obligatoire.
    pub model_path: String,
    /// Chemin du socket Unix Domain Socket. Défaut : `/tmp/wakeword_daemon.sock`.
    pub socket_path: String,
    /// Seuil de score individuel pour un vote positif. Défaut : `0.80`.
    pub score_threshold: f32,
    /// Délai minimal entre deux détections en millisecondes. Défaut : `2000`.
    pub cooldown_ms: u64,
}

impl DaemonConfig {
    /// Construit la configuration depuis les variables d'environnement.
    ///
    /// | Variable               | Défaut                          |
    /// |------------------------|---------------------------------|
    /// | `WAKEWORD_MODEL_PATH`  | *(obligatoire)*                 |
    /// | `WAKEWORD_SOCKET_PATH` | `/tmp/wakeword_daemon.sock`     |
    /// | `WAKEWORD_THRESHOLD`   | `0.80`                          |
    /// | `WAKEWORD_COOLDOWN_MS` | `2000`                          |
    pub fn from_env() -> Result<Self> {
        let model_path = std::env::var("WAKEWORD_MODEL_PATH").unwrap_or_default();
        if model_path.is_empty() {
            bail!(
                "Variable d'environnement WAKEWORD_MODEL_PATH manquante.\n\
                 Exemple : WAKEWORD_MODEL_PATH=/chemin/vers/WakeWord.mlmodelc word-waker"
            );
        }

        let socket_path = std::env::var("WAKEWORD_SOCKET_PATH")
            .unwrap_or_else(|_| "/tmp/wakeword_daemon.sock".to_string());

        let score_threshold: f32 = std::env::var("WAKEWORD_THRESHOLD")
            .unwrap_or_else(|_| "0.80".to_string())
            .parse()
            .map_err(|_| anyhow::anyhow!("WAKEWORD_THRESHOLD doit être un nombre flottant (ex: 0.80)"))?;

        if !(0.0..=1.0).contains(&score_threshold) {
            bail!("WAKEWORD_THRESHOLD doit être dans [0.0, 1.0], valeur reçue : {score_threshold}");
        }

        let cooldown_ms: u64 = std::env::var("WAKEWORD_COOLDOWN_MS")
            .unwrap_or_else(|_| "2000".to_string())
            .parse()
            .map_err(|_| anyhow::anyhow!("WAKEWORD_COOLDOWN_MS doit être un entier positif"))?;

        Ok(DaemonConfig {
            model_path,
            socket_path,
            score_threshold,
            cooldown_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_env_defaults_without_model_path_returns_error() {
        std::env::remove_var("WAKEWORD_MODEL_PATH");
        assert!(DaemonConfig::from_env().is_err());
    }

    #[test]
    fn from_env_with_model_path_uses_defaults() {
        std::env::set_var("WAKEWORD_MODEL_PATH", "/tmp/fake.mlmodelc");
        std::env::remove_var("WAKEWORD_SOCKET_PATH");
        std::env::remove_var("WAKEWORD_THRESHOLD");
        std::env::remove_var("WAKEWORD_COOLDOWN_MS");

        let cfg = DaemonConfig::from_env().expect("from_env failed");
        assert_eq!(cfg.socket_path, "/tmp/wakeword_daemon.sock");
        assert!((cfg.score_threshold - 0.80).abs() < 1e-5);
        assert_eq!(cfg.cooldown_ms, 2000);

        std::env::remove_var("WAKEWORD_MODEL_PATH");
    }

    #[test]
    fn from_env_invalid_threshold_returns_error() {
        std::env::set_var("WAKEWORD_MODEL_PATH", "/tmp/fake.mlmodelc");
        std::env::set_var("WAKEWORD_THRESHOLD", "not_a_float");
        assert!(DaemonConfig::from_env().is_err());
        std::env::remove_var("WAKEWORD_MODEL_PATH");
        std::env::remove_var("WAKEWORD_THRESHOLD");
    }

    #[test]
    fn from_env_threshold_out_of_range_returns_error() {
        std::env::set_var("WAKEWORD_MODEL_PATH", "/tmp/fake.mlmodelc");
        std::env::set_var("WAKEWORD_THRESHOLD", "1.5");
        assert!(DaemonConfig::from_env().is_err());
        std::env::remove_var("WAKEWORD_MODEL_PATH");
        std::env::remove_var("WAKEWORD_THRESHOLD");
    }
}
