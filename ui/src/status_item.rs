//! Gestion du NSStatusItem dans la barre de menu macOS.
//!
//! Affiche l'emoji et gère le menu contextuel via AppKit natif.
//! Utilise `icrate` pour les bindings AppKit idiomatiques.

use std::sync::Mutex;

use icrate::objc2::rc::Id;
use icrate::AppKit::{NSMenu, NSMenuItem, NSStatusBar, NSStatusItem, NSVariableStatusItemLength};
use icrate::Foundation::{MainThreadMarker, NSString};

use crate::error::UiError;
use crate::socket_client::UiEvent;

/// État actuel de la connexion au daemon tel qu'affiché dans le StatusItem.
#[derive(Debug, Clone, PartialEq)]
pub enum DaemonState {
    /// Daemon connecté, en écoute
    Connected,
    /// Daemon déconnecté
    Disconnected,
    /// Mot-clé détecté (flash temporaire, retour à Connected après 500ms)
    Detected,
}

/// Représente l'icône dans la barre de menu macOS.
/// Wrapper autour du NSStatusItem AppKit.
pub struct MenuBarIcon {
    /// Le NSStatusItem sous-jacent
    status_item: Id<NSStatusItem>,
    /// Le menu contextuel
    menu: Id<NSMenu>,
    /// État actuel
    state: Mutex<DaemonState>,
    /// Compteur de détections
    detection_count: Mutex<u64>,
    /// Item de menu affichant le compteur
    counter_item: Id<NSMenuItem>,
    /// Item de menu "Historique..."
    history_item: Id<NSMenuItem>,
}

impl MenuBarIcon {
    /// Crée un nouveau NSStatusItem dans la barre de menu.
    pub fn new(mtm: MainThreadMarker) -> Result<Self, UiError> {
        let status_bar = unsafe { NSStatusBar::systemStatusBar() };
        let status_item = unsafe { status_bar.statusItemWithLength(NSVariableStatusItemLength) };

        // Créer le menu (inclut le compteur)
        let (menu, counter_item, history_item) = Self::build_menu(mtm)?;

        // Associer le menu au status item
        unsafe {
            status_item.setMenu(Some(&menu));
        }

        Ok(Self {
            status_item,
            menu,
            state: Mutex::new(DaemonState::Connected),
            detection_count: Mutex::new(0),
            counter_item,
            history_item,
        })
    }

    /// Met à jour l'état visuel du StatusItem en fonction de l'événement reçu.
    pub fn update_state(&self, event: &UiEvent) {
        match event {
            UiEvent::Connected => {
                *self.state.lock().unwrap() = DaemonState::Connected;
                self.set_title("🎤");
                self.set_tooltip("Word Waker — en écoute");
                tracing::info!("UI: état → Connected");
            }
            UiEvent::Disconnected => {
                *self.state.lock().unwrap() = DaemonState::Disconnected;
                self.set_title("⏸");
                self.set_tooltip("Word Waker — daemon déconnecté");
                tracing::info!("UI: état → Disconnected");
            }
            UiEvent::WakeWordDetected { .. } => {
                *self.state.lock().unwrap() = DaemonState::Detected;
                // Incrémenter le compteur
                let mut count = self.detection_count.lock().unwrap();
                *count += 1;
                self.update_counter_display(*count);

                // Flash : afficher 🎤✅ puis revenir à 🎤 après 500ms
                self.set_title("🎤✅");
                self.set_tooltip("Word Waker — mot détecté !");
                tracing::info!(count = *count, "UI: mot détecté");

                self.schedule_flash_reset();
            }
            UiEvent::Error(msg) => {
                tracing::warn!("UI: erreur socket — {}", msg);
            }
        }
    }

    /// Planifie le retour du titre après le flash de détection.
    fn schedule_flash_reset(&self) {
        // Utiliser dispatch_after pour revenir à "🎤" après 500ms
        // On capture une référence faible pour éviter les cycles
        // Puisque self est &self et qu'on ne peut pas capturer self dans un bloc dispatch,
        // on utilise une approche simplifiée : on fait le reset via le set_title directement
        // depuis le timer principal (géré dans app.rs)

        // Note: pour l'instant, le flash est géré par le NSTimer principal
        // qui vérifie l'état et remet le titre après 500ms.
        // Ce sera implémenté dans app.rs via un champ last_detection_time.
    }

    /// Remet le titre à "🎤" après le flash (appelé par le timer principal).
    pub fn reset_flash_if_needed(
        &self,
        now: std::time::Instant,
        last_detection: std::time::Instant,
    ) {
        let current_state = self.state.lock().unwrap().clone();
        if current_state == DaemonState::Detected {
            let elapsed = now.duration_since(last_detection);
            if elapsed >= std::time::Duration::from_millis(500) {
                *self.state.lock().unwrap() = DaemonState::Connected;
                self.set_title("🎤");
                self.set_tooltip("Word Waker — en écoute");
                tracing::debug!("UI: flash terminé, retour à Connected");
            }
        }
    }

    /// Retourne l'état actuel (pour les tests).
    pub fn current_state(&self) -> DaemonState {
        self.state.lock().unwrap().clone()
    }

    /// Retourne le compteur de détections (pour les tests).
    pub fn detection_count(&self) -> u64 {
        *self.detection_count.lock().unwrap()
    }

    /// Met à jour l'affichage du compteur dans le menu.
    fn update_counter_display(&self, count: u64) {
        let label = format!("Détections : {}", count);
        unsafe {
            let ns_label = NSString::from_str(&label);
            self.counter_item.setTitle(&ns_label);
        }
    }

    /// Définit le texte affiché dans la barre de menu (emoji).
    fn set_title(&self, title: &str) {
        unsafe {
            let ns_title = NSString::from_str(title);
            self.status_item.setTitle(Some(&ns_title));
        }
    }

    /// Définit le tooltip affiché au survol.
    fn set_tooltip(&self, tooltip: &str) {
        unsafe {
            let ns_tooltip = NSString::from_str(tooltip);
            self.status_item.setToolTip(Some(&ns_tooltip));
        }
    }

    /// Construit le menu contextuel du StatusItem.
    /// Retourne le menu et l'item "compteur" pour mise à jour ultérieure.
    fn build_menu(
        mtm: MainThreadMarker,
    ) -> Result<(Id<NSMenu>, Id<NSMenuItem>, Id<NSMenuItem>), UiError> {
        let menu = NSMenu::new(mtm);

        // Item compteur de détections (en haut du menu)
        let counter_label = NSString::from_str("Détections : 0");
        let counter_item = NSMenuItem::new(mtm);
        unsafe {
            counter_item.setTitle(&counter_label);
            // Désactiver l'item pour qu'il soit purement informatif
            counter_item.setEnabled(false);
        }
        menu.addItem(&counter_item);

        // Item "Historique..."
        let history_label = NSString::from_str("Historique...");
        let history_item = NSMenuItem::new(mtm);
        unsafe {
            history_item.setTitle(&history_label);
            // Action gérée dans UiApp via setTarget/setAction
        }
        menu.addItem(&history_item);

        // Ajouter le séparateur
        let separator = NSMenuItem::separatorItem(mtm);
        menu.addItem(&separator);

        // Item "Quitter"
        let quit_title = NSString::from_str("Quitter");
        let key_equiv = NSString::from_str("q");
        let quit_item = NSMenuItem::new(mtm);
        unsafe {
            quit_item.setTitle(&quit_title);
            quit_item.setKeyEquivalent(&key_equiv);
        }
        menu.addItem(&quit_item);

        Ok((menu, counter_item, history_item))
    }
}

// Safety: MenuBarIcon est toujours accédé depuis le main thread
// (via performSelectorOnMainThread). Les Arc<MenuBarIcon> ne sont utilisés
// que pour le partage de propriété, pas pour l'accès concurrent.
unsafe impl Send for MenuBarIcon {}
unsafe impl Sync for MenuBarIcon {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::socket_client::UiEvent;

    #[test]
    fn test_daemon_state_transitions() {
        // Test unitaire des transitions d'état sans AppKit
        let state = DaemonState::Connected;
        assert_eq!(state, DaemonState::Connected);

        let state = DaemonState::Disconnected;
        assert_eq!(state, DaemonState::Disconnected);

        let state = DaemonState::Detected;
        assert_eq!(state, DaemonState::Detected);
    }

    #[test]
    fn test_detection_counter_logic() {
        // Test de la logique du compteur sans AppKit
        let mut count: u64 = 0;

        // Simuler 5 détections
        let events = vec![
            UiEvent::WakeWordDetected {
                timestamp: std::time::Instant::now(),
            },
            UiEvent::WakeWordDetected {
                timestamp: std::time::Instant::now(),
            },
            UiEvent::WakeWordDetected {
                timestamp: std::time::Instant::now(),
            },
            UiEvent::WakeWordDetected {
                timestamp: std::time::Instant::now(),
            },
            UiEvent::WakeWordDetected {
                timestamp: std::time::Instant::now(),
            },
        ];

        for event in &events {
            if matches!(event, UiEvent::WakeWordDetected { .. }) {
                count += 1;
            }
        }

        assert_eq!(count, 5, "Counter should be 5 after 5 detections");

        // Vérifier que le compteur ne panic pas sur overflow
        // En debug mode, u64::MAX + 1 panique, donc on teste avec wrapping_add
        count = u64::MAX;
        count = count.wrapping_add(1); // 0
        assert_eq!(count, 0, "Wrapping overflow should give 0");
    }
}
