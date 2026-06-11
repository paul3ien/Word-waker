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
use crate::socket_client::UiEvent;
use crate::status_item::MenuBarIcon;

/// Représente l'application UI Word Waker.
pub struct UiApp {
    #[allow(dead_code)]
    config: UiConfig,
    app: icrate::objc2::rc::Id<NSApplication>,
    status_item: Arc<MenuBarIcon>,
    last_detection: Arc<Mutex<Option<Instant>>>,
}

impl UiApp {
    /// Crée une nouvelle instance de l'application UI.
    pub fn new(config: UiConfig, mtm: MainThreadMarker) -> Result<Self, UiError> {
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicyAccessory);

        let status_item = MenuBarIcon::new(mtm)?;
        status_item.update_state(&UiEvent::Connected);

        tracing::info!("NSApplication initialisée, StatusItem créé");

        Ok(Self {
            config,
            app,
            status_item: Arc::new(status_item),
            last_detection: Arc::new(Mutex::new(None)),
        })
    }

    /// Lance la boucle d'événements AppKit.
    /// Le bridge socket → UI se fait via un thread de fond qui met à jour
    /// le StatusItem. Pour le POC, les mutations UI depuis un thread non-main
    /// sont acceptables.
    pub fn run(&self, event_rx: Receiver<UiEvent>) -> anyhow::Result<()> {
        tracing::info!("Démarrage de la boucle d'événements AppKit");

        let status_item = Arc::clone(&self.status_item);
        let last_detection = Arc::clone(&self.last_detection);

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
                        if matches!(event, UiEvent::WakeWordDetected { .. }) {
                            *last_detection.lock().unwrap() = Some(now);
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
}
