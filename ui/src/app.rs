//! Application principale UI Word Waker.
//!
//! Gère l'initialisation de NSApplication, la création du NSStatusItem,
//! et la boucle d'événements principale.

use crossbeam_channel::Receiver;
use icrate::AppKit::{NSApplication, NSApplicationActivationPolicyAccessory};
use icrate::Foundation::MainThreadMarker;
use tracing;

use crate::config::UiConfig;
use crate::error::UiError;
use crate::socket_client::UiEvent;
use crate::status_item::MenuBarIcon;

/// Représente l'application UI Word Waker.
pub struct UiApp {
    #[allow(dead_code)]
    config: UiConfig,
    app: icrate::objc2::rc::Id<NSApplication>,
    status_item: MenuBarIcon,
    #[allow(dead_code)]
    event_rx: Receiver<UiEvent>,
}

impl UiApp {
    /// Crée une nouvelle instance de l'application UI.
    /// Initialise NSApplication et le NSStatusItem.
    ///
    /// Doit être appelé depuis le main thread.
    pub fn new(
        config: UiConfig,
        event_rx: Receiver<UiEvent>,
        mtm: MainThreadMarker,
    ) -> Result<Self, UiError> {
        // Initialiser NSApplication
        let app = NSApplication::sharedApplication(mtm);

        // LSUIElement = true → pas d'icône dans le Dock
        app.setActivationPolicy(NSApplicationActivationPolicyAccessory);

        // Créer l'icône dans la barre de menu
        let status_item = MenuBarIcon::new(mtm)?;

        // Appliquer l'état initial
        status_item.set_title("🎤");
        status_item.set_tooltip("Word Waker — en écoute");

        tracing::info!("NSApplication initialisée, StatusItem créé");

        Ok(Self {
            config,
            app,
            status_item,
            event_rx,
        })
    }

    /// Lance la boucle d'événements AppKit.
    pub fn run(&self) {
        tracing::info!("Démarrage de la boucle d'événements AppKit");
        unsafe {
            self.app.run();
        }
    }

    /// Retourne une référence au MenuBarIcon pour les mises à jour.
    pub fn status_item(&self) -> &MenuBarIcon {
        &self.status_item
    }
}
