//! Application principale UI Word Waker.
//!
//! Gère l'initialisation de NSApplication, la création du NSStatusItem,
//! le bridge thread socket → main thread UI, et la boucle d'événements.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use crossbeam_channel::Receiver;
use icrate::AppKit::{NSApplication, NSApplicationActivationPolicyAccessory};
use icrate::Foundation::MainThreadMarker;
use tracing;

use crate::config::UiConfig;
use crate::error::UiError;
use crate::panel::DetectionsPanel;
use crate::socket_client::UiEvent;
use crate::status_item::MenuBarIcon;

/// Représente l'application UI Word Waker.
pub struct UiApp {
    #[allow(dead_code)]
    config: UiConfig,
    app: icrate::objc2::rc::Id<NSApplication>,
    status_item: Arc<MenuBarIcon>,
    last_detection: Arc<Mutex<Option<Instant>>>,
    /// Panneau d'historique des détections
    panel: Arc<DetectionsPanel>,
}

impl UiApp {
    /// Crée une nouvelle instance de l'application UI.
    pub fn new(config: UiConfig, mtm: MainThreadMarker) -> Result<Self, UiError> {
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicyAccessory);

        let status_item = MenuBarIcon::new(mtm)?;
        status_item.update_state(&UiEvent::Connected);

        // Créer le panel d'historique
        let panel = DetectionsPanel::new(mtm)?;

        tracing::info!("NSApplication initialisée, StatusItem et Panel créés");

        Ok(Self {
            config,
            app,
            status_item: Arc::new(status_item),
            last_detection: Arc::new(Mutex::new(None)),
            panel: Arc::new(panel),
        })
    }

    /// Lance la boucle d'événements AppKit.
    pub fn run(&self, event_rx: Receiver<UiEvent>) -> anyhow::Result<()> {
        tracing::info!("Démarrage de la boucle d'événements AppKit");

        let status_item = Arc::clone(&self.status_item);
        let last_detection = Arc::clone(&self.last_detection);
        let panel = Arc::clone(&self.panel);

        // Thread de fond : poll le channel et met à jour l'UI
        std::thread::Builder::new()
            .name("event-processor".into())
            .spawn(move || {
                let poll_interval = std::time::Duration::from_millis(100);
                loop {
                    let events: Vec<UiEvent> = {
                        let mut all = Vec::new();
                        loop {
                            match event_rx.try_recv() {
                                Ok(event) => all.push(event),
                                Err(crossbeam_channel::TryRecvError::Empty) => break,
                                Err(crossbeam_channel::TryRecvError::Disconnected) => return,
                            }
                        }
                        all
                    };

                    if events.is_empty() {
                        std::thread::sleep(poll_interval);
                        continue;
                    }

                    let now = Instant::now();

                    for event in &events {
                        status_item.update_state(event);
                        if let UiEvent::WakeWordDetected { timestamp } = event {
                            *last_detection.lock().unwrap() = Some(now);
                            panel.add_detection(*timestamp);
                        }
                    }

                    // Gestion du flash
                    if let Some(last_det) = *last_detection.lock().unwrap() {
                        status_item.reset_flash_if_needed(now, last_det);
                        if status_item.current_state() != crate::status_item::DaemonState::Detected
                        {
                            *last_detection.lock().unwrap() = None;
                        }
                    }

                    std::thread::sleep(poll_interval);
                }
            })?;

        // Lancer la RunLoop AppKit (bloquant)
        unsafe {
            self.app.run();
        }

        tracing::info!("RunLoop AppKit terminée");
        Ok(())
    }

    /// Affiche le panel d'historique.
    pub fn show_history_panel(&self) {
        self.panel.show();
    }
}
