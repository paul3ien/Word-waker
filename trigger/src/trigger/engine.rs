use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::trigger::config::TriggerConfig;

/// Moteur de vote glissant anti-faux-positifs.
///
/// Maintient une fenêtre FIFO des derniers scores reçus et déclenche
/// lorsque suffisamment de votes positifs sont présents dans la fenêtre.
///
/// # Sémantique du vote
///
/// - Un score est **positif** si il est **strictement supérieur** à `score_threshold`.
/// - La détection se produit quand `votes >= vote_threshold`.
/// - Le cooldown est vérifié **avant** le comptage des votes.
/// - Après une détection, `history` est vidée pour éviter les cascades.
pub struct TriggerEngine {
    history: VecDeque<f32>,
    window_size: usize,
    score_threshold: f32,
    vote_threshold: usize,
    cooldown_ms: u64,
    last_trigger: Instant,
}

impl TriggerEngine {
    /// Crée un nouveau moteur à partir d'une `TriggerConfig`.
    ///
    /// Le cooldown est initialisé comme déjà écoulé afin que la première
    /// détection soit possible immédiatement.
    pub fn new(config: &TriggerConfig) -> Self {
        let cooldown_ms = config.cooldown_ms;
        Self {
            history: VecDeque::with_capacity(config.window_size),
            window_size: config.window_size,
            score_threshold: config.score_threshold,
            vote_threshold: config.vote_threshold,
            cooldown_ms,
            // Cooldown déjà expiré au démarrage
            last_trigger: Instant::now()
                .checked_sub(Duration::from_millis(cooldown_ms + 1))
                .unwrap_or_else(Instant::now),
        }
    }

    /// Ajoute un score à la fenêtre et retourne `true` si un wake-word est détecté.
    ///
    /// Ordre d'évaluation :
    /// 1. Ajout du score dans la fenêtre (éviction du plus ancien si pleine).
    /// 2. Vérification du cooldown — retourne `false` immédiatement si actif.
    /// 3. Comptage des votes positifs.
    /// 4. Si `votes >= vote_threshold` : mise à jour du cooldown, vidage de
    ///    l'historique, retourne `true`.
    pub fn push(&mut self, score: f32) -> bool {
        // 1. Glissement de fenêtre
        self.history.push_back(score);
        if self.history.len() > self.window_size {
            self.history.pop_front();
        }

        // 2. Cooldown
        if self.last_trigger.elapsed().as_millis() < self.cooldown_ms as u128 {
            return false;
        }

        // 3. Votes
        let votes = self
            .history
            .iter()
            .filter(|&&s| s > self.score_threshold)
            .count();

        // 4. Détection
        if votes >= self.vote_threshold {
            self.last_trigger = Instant::now();
            self.history.clear();
            return true;
        }

        false
    }

    /// Remet le moteur à zéro : vide l'historique et réinitialise le cooldown
    /// comme déjà écoulé.
    pub fn reset(&mut self) {
        self.history.clear();
        self.last_trigger = Instant::now()
            .checked_sub(Duration::from_millis(self.cooldown_ms + 1))
            .unwrap_or_else(Instant::now);
    }

    /// Retourne le nombre de votes positifs actuellement dans la fenêtre.
    pub fn pending_votes(&self) -> usize {
        self.history
            .iter()
            .filter(|&&s| s > self.score_threshold)
            .count()
    }

    /// Retourne le nombre de scores actuellement dans la fenêtre.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Retourne le temps restant avant la fin du cooldown en millisecondes.
    /// Retourne 0 si le cooldown est expiré.
    pub fn cooldown_remaining_ms(&self) -> u64 {
        let elapsed = self.last_trigger.elapsed().as_millis() as u64;
        self.cooldown_ms.saturating_sub(elapsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_engine() -> TriggerEngine {
        TriggerEngine::new(&TriggerConfig::default())
    }

    // ─── Cas nominaux ─────────────────────────────────────────────────────────

    #[test]
    fn nominal_detection_3_positive_in_5() {
        let mut engine = default_engine();
        assert!(!engine.push(0.9));
        assert!(!engine.push(0.5));
        assert!(!engine.push(0.9));
        assert!(!engine.push(0.5));
        assert!(engine.push(0.9)); // 3 votes positifs → détection
    }

    #[test]
    fn no_detection_with_only_2_positive() {
        let mut engine = default_engine();
        for score in [0.9, 0.5, 0.9, 0.5, 0.5] {
            assert!(!engine.push(score));
        }
    }

    #[test]
    fn exact_threshold_does_not_count_as_vote() {
        // 0.80 exact → ne compte PAS (filtre strict >)
        let mut engine = default_engine();
        for _ in 0..5 {
            assert!(!engine.push(0.80));
        }
    }

    #[test]
    fn just_above_threshold_counts_as_vote() {
        let mut engine = default_engine();
        assert!(!engine.push(0.801));
        assert_eq!(engine.pending_votes(), 1);
    }

    // ─── Fenêtre glissante ────────────────────────────────────────────────────

    #[test]
    fn window_does_not_exceed_window_size() {
        let mut engine = default_engine();
        for _ in 0..10 {
            engine.push(0.5);
        }
        assert!(engine.history_len() <= 5);
    }

    #[test]
    fn cooldown_blocks_immediate_second_trigger() {
        let mut engine = default_engine();
        // Première détection
        for score in [0.9, 0.9, 0.9, 0.1, 0.1] {
            engine.push(score);
        }
        // Immédiatement après : 3 scores positifs → cooldown bloque
        for _ in 0..3 {
            assert!(!engine.push(0.9));
        }
    }

    #[test]
    fn single_positive_score_does_not_trigger() {
        let mut engine = default_engine();
        assert!(!engine.push(0.9)); // 1 vote < 3 requis
    }

    // ─── Cooldown ─────────────────────────────────────────────────────────────

    #[test]
    fn cooldown_active_after_detection() {
        let mut engine = default_engine();
        // Déclencher
        for score in [0.9, 0.9, 0.9, 0.1, 0.1] {
            engine.push(score);
        }
        // Immédiatement : cooldown actif
        assert!(!engine.push(0.9));
        assert!(!engine.push(0.9));
        assert!(!engine.push(0.9));
    }

    #[test]
    fn first_detection_possible_immediately_after_init() {
        let mut engine = default_engine();
        // Le cooldown est pré-expiré à l'initialisation
        assert!(!engine.push(0.9));
        assert!(!engine.push(0.9));
        assert!(engine.push(0.9)); // window_size=5, vote_threshold=3 → 3 votes sur 3 → ok
    }

    #[test]
    fn cooldown_remaining_nonzero_just_after_detection() {
        let mut engine = default_engine();
        for score in [0.9, 0.9, 0.9, 0.1, 0.1] {
            engine.push(score);
        }
        let remaining = engine.cooldown_remaining_ms();
        // Doit être ≈ 2000 ms, tolérance ±50 ms
        assert!(remaining > 1950, "remaining was {}", remaining);
        assert!(remaining <= 2000, "remaining was {}", remaining);
    }

    #[test]
    fn cooldown_remaining_zero_before_any_detection() {
        let engine = default_engine();
        assert_eq!(engine.cooldown_remaining_ms(), 0);
    }

    // ─── Comportement post-détection ──────────────────────────────────────────

    #[test]
    fn history_cleared_after_detection() {
        let mut engine = default_engine();
        assert!(!engine.push(0.9));
        assert!(!engine.push(0.9));
        let detected = engine.push(0.9); // 3 votes → détection, clear immédiat
        assert!(detected);
        assert_eq!(engine.history_len(), 0);
    }

    #[test]
    fn reset_clears_state() {
        let mut engine = default_engine();
        engine.push(0.9);
        engine.push(0.9);
        engine.reset();
        assert_eq!(engine.pending_votes(), 0);
        assert_eq!(engine.history_len(), 0);
    }

    #[test]
    fn detection_possible_after_reset() {
        let mut engine = default_engine();
        // Déclencher une première fois
        for score in [0.9, 0.9, 0.9, 0.1, 0.1] {
            engine.push(score);
        }
        // Reset remet le cooldown à expiré
        engine.reset();
        // Nouvelle détection possible
        assert!(!engine.push(0.9));
        assert!(!engine.push(0.9));
        assert!(engine.push(0.9));
    }

    // ─── Configurations limites ───────────────────────────────────────────────

    #[test]
    fn window_1_vote_1_detects_immediately() {
        let cfg = TriggerConfig {
            window_size: 1,
            vote_threshold: 1,
            ..TriggerConfig::default()
        };
        let mut engine = TriggerEngine::new(&cfg);
        assert!(engine.push(0.9));
    }

    #[test]
    fn vote_threshold_equals_window_size() {
        let cfg = TriggerConfig {
            window_size: 3,
            vote_threshold: 3,
            ..TriggerConfig::default()
        };
        let mut engine = TriggerEngine::new(&cfg);
        assert!(!engine.push(0.9));
        assert!(!engine.push(0.9));
        assert!(engine.push(0.9)); // 3/3 positifs → détection
    }

    #[test]
    fn high_threshold_only_near_one_counts() {
        let cfg = TriggerConfig {
            score_threshold: 0.99,
            window_size: 3,
            vote_threshold: 3,
            ..TriggerConfig::default()
        };
        let mut engine = TriggerEngine::new(&cfg);
        // 0.95 ne dépasse pas 0.99
        for _ in 0..3 {
            assert!(!engine.push(0.95));
        }
        // 0.999 dépasse bien 0.99
        engine.reset();
        assert!(!engine.push(0.999));
        assert!(!engine.push(0.999));
        assert!(engine.push(0.999));
    }
}
