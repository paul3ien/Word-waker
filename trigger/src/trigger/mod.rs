pub mod config;
pub mod engine;
pub mod error;
pub mod ipc;
pub mod runner;

pub use config::TriggerConfig;
pub use engine::TriggerEngine;
pub use error::TriggerError;
pub use ipc::IpcNotifier;
pub use runner::TriggerRunner;

use crossbeam_channel::Receiver;

/// Façade publique du module trigger.
///
/// Seule surface visible depuis le daemon. Encapsule la configuration,
/// le moteur de vote et le thread runner.
///
/// # Exemple minimal
///
/// ```no_run
/// use trigger::{TriggerModule, TriggerConfig};
///
/// let config = TriggerConfig::default();
/// let mut module = TriggerModule::new(config).unwrap();
/// let (tx, rx) = crossbeam_channel::unbounded();
/// module.start(rx).unwrap();
/// // … envoyer des scores via tx …
/// module.stop().unwrap();
/// ```
pub struct TriggerModule {
    runner: TriggerRunner,
    config: TriggerConfig,
}

impl TriggerModule {
    /// Crée un nouveau module trigger.
    ///
    /// Valide la configuration avant toute allocation.
    pub fn new(config: TriggerConfig) -> Result<Self, TriggerError> {
        config.validate()?;
        let runner = TriggerRunner::new(&config);
        Ok(Self { runner, config })
    }

    /// Démarre le thread trigger qui consomme les scores du `Receiver`.
    pub fn start(&mut self, rx: Receiver<f32>) -> Result<(), TriggerError> {
        self.runner.start(rx)
    }

    /// Arrête proprement le thread trigger.
    pub fn stop(&mut self) -> Result<(), TriggerError> {
        self.runner.stop();
        Ok(())
    }

    /// Retourne une référence à la configuration active.
    pub fn config(&self) -> &TriggerConfig {
        &self.config
    }
}

impl Drop for TriggerModule {
    fn drop(&mut self) {
        self.runner.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn unique_socket_path() -> String {
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        format!("/tmp/wakeword_module_test_{}.sock", id)
    }

    fn cleanup(path: &str) {
        let _ = std::fs::remove_file(path);
    }

    fn test_config(socket_path: &str) -> TriggerConfig {
        TriggerConfig {
            socket_path: socket_path.to_string(),
            cooldown_ms: 200,
            ..TriggerConfig::default()
        }
    }

    fn read_message(listener: &UnixListener) -> Vec<u8> {
        let (mut stream, _) = listener.accept().expect("accept failed");
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).expect("read failed");
        buf
    }

    // ─── Test 1 : cycle complet new → start → scores → stop ─────────────────

    #[test]
    fn full_cycle_detects_and_notifies() {
        let path = unique_socket_path();
        cleanup(&path);

        let listener = UnixListener::bind(&path).expect("bind failed");
        let mut module = TriggerModule::new(test_config(&path)).expect("new failed");
        let (tx, rx) = crossbeam_channel::unbounded::<f32>();

        module.start(rx).expect("start failed");

        tx.send(0.9).unwrap();
        tx.send(0.5).unwrap();
        tx.send(0.9).unwrap();
        tx.send(0.5).unwrap();
        tx.send(0.9).unwrap(); // 3e vote → détection

        assert_eq!(read_message(&listener), b"WAKEWORD_DETECTED\n");

        drop(tx);
        module.stop().expect("stop failed");
        cleanup(&path);
    }

    // ─── Test 2 : deux cycles start/stop consécutifs ─────────────────────────

    #[test]
    fn two_consecutive_start_stop_cycles() {
        let path = unique_socket_path();
        cleanup(&path);

        let listener = UnixListener::bind(&path).expect("bind failed");
        let mut module = TriggerModule::new(test_config(&path)).expect("new failed");

        // Premier cycle
        {
            let (tx, rx) = crossbeam_channel::unbounded::<f32>();
            module.start(rx).expect("first start failed");
            tx.send(0.9).unwrap();
            tx.send(0.9).unwrap();
            tx.send(0.9).unwrap();
            assert_eq!(read_message(&listener), b"WAKEWORD_DETECTED\n");
            drop(tx);
            module.stop().expect("first stop failed");
        }

        // Deuxième cycle — recréer le runner depuis une nouvelle instance
        let path2 = unique_socket_path();
        cleanup(&path2);
        let listener2 = UnixListener::bind(&path2).expect("bind2 failed");
        let mut module2 = TriggerModule::new(test_config(&path2)).expect("new2 failed");
        {
            let (tx, rx) = crossbeam_channel::unbounded::<f32>();
            module2.start(rx).expect("second start failed");
            tx.send(0.9).unwrap();
            tx.send(0.9).unwrap();
            tx.send(0.9).unwrap();
            assert_eq!(read_message(&listener2), b"WAKEWORD_DETECTED\n");
            drop(tx);
            module2.stop().expect("second stop failed");
        }

        cleanup(&path);
        cleanup(&path2);
    }

    // ─── Test 3 : Drop sans stop → propre ────────────────────────────────────

    #[test]
    fn drop_without_stop_is_clean() {
        let path = unique_socket_path();
        cleanup(&path);

        let mut module = TriggerModule::new(test_config(&path)).expect("new failed");
        let (tx, rx) = crossbeam_channel::unbounded::<f32>();
        module.start(rx).expect("start failed");

        drop(tx);
        drop(module); // Drop appelle stop() → zéro panic, zéro zombie
        cleanup(&path);
    }

    // ─── Test 4 : config invalide → new retourne Err ─────────────────────────

    #[test]
    fn invalid_config_returns_error() {
        let cfg = TriggerConfig {
            score_threshold: 0.0, // invalide
            ..TriggerConfig::default()
        };
        assert!(matches!(
            TriggerModule::new(cfg),
            Err(TriggerError::InvalidConfig(_))
        ));
    }
}
