use crate::trigger::TriggerError;

/// Paramètres configurables du module trigger.
#[derive(Debug, Clone)]
pub struct TriggerConfig {
    /// Seuil individuel : un score doit être **strictement supérieur** à cette valeur pour compter comme vote positif.
    pub score_threshold: f32,
    /// Nombre de votes positifs requis dans la fenêtre pour déclencher.
    pub vote_threshold: usize,
    /// Taille de la fenêtre glissante (en nombre d'inférences).
    pub window_size: usize,
    /// Délai minimal entre deux détections (en millisecondes).
    pub cooldown_ms: u64,
    /// Chemin du socket Unix Domain Socket IPC.
    pub socket_path: String,
}

impl Default for TriggerConfig {
    fn default() -> Self {
        Self {
            score_threshold: 0.80,
            vote_threshold: 3,
            window_size: 5,
            cooldown_ms: 2000,
            socket_path: "/tmp/wakeword_daemon.sock".to_string(),
        }
    }
}

impl TriggerConfig {
    /// Valide la cohérence des paramètres.
    ///
    /// Retourne `Err(TriggerError::InvalidConfig)` si un paramètre est incohérent.
    pub fn validate(&self) -> Result<(), TriggerError> {
        if self.score_threshold <= 0.0 || self.score_threshold > 1.0 {
            return Err(TriggerError::InvalidConfig(format!(
                "score_threshold ({}) doit être dans (0.0, 1.0]",
                self.score_threshold
            )));
        }
        if self.window_size == 0 {
            return Err(TriggerError::InvalidConfig(
                "window_size doit être > 0".to_string(),
            ));
        }
        if self.vote_threshold > self.window_size {
            return Err(TriggerError::InvalidConfig(format!(
                "vote_threshold ({}) ne peut pas dépasser window_size ({})",
                self.vote_threshold, self.window_size
            )));
        }
        if self.cooldown_ms == 0 {
            return Err(TriggerError::InvalidConfig(
                "cooldown_ms doit être > 0".to_string(),
            ));
        }
        if self.socket_path.is_empty() {
            return Err(TriggerError::InvalidConfig(
                "socket_path ne peut pas être vide".to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        assert!(TriggerConfig::default().validate().is_ok());
    }

    #[test]
    fn vote_threshold_greater_than_window_size_is_invalid() {
        let cfg = TriggerConfig {
            vote_threshold: 6,
            window_size: 5,
            ..TriggerConfig::default()
        };
        assert!(matches!(
            cfg.validate(),
            Err(TriggerError::InvalidConfig(_))
        ));
    }

    #[test]
    fn score_threshold_zero_is_invalid() {
        let cfg = TriggerConfig {
            score_threshold: 0.0,
            ..TriggerConfig::default()
        };
        assert!(matches!(
            cfg.validate(),
            Err(TriggerError::InvalidConfig(_))
        ));
    }

    #[test]
    fn score_threshold_above_one_is_invalid() {
        let cfg = TriggerConfig {
            score_threshold: 1.5,
            ..TriggerConfig::default()
        };
        assert!(matches!(
            cfg.validate(),
            Err(TriggerError::InvalidConfig(_))
        ));
    }

    #[test]
    fn window_size_zero_is_invalid() {
        let cfg = TriggerConfig {
            window_size: 0,
            vote_threshold: 0,
            ..TriggerConfig::default()
        };
        assert!(matches!(
            cfg.validate(),
            Err(TriggerError::InvalidConfig(_))
        ));
    }

    #[test]
    fn empty_socket_path_is_invalid() {
        let cfg = TriggerConfig {
            socket_path: "".to_string(),
            ..TriggerConfig::default()
        };
        assert!(matches!(
            cfg.validate(),
            Err(TriggerError::InvalidConfig(_))
        ));
    }
}
