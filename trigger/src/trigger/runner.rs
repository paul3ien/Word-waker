use std::thread::{self, JoinHandle};

use crossbeam_channel::{Receiver, Sender};

use crate::trigger::{
    config::TriggerConfig, engine::TriggerEngine, error::TriggerError, ipc::IpcNotifier,
};

/// Thread trigger — orchestre le moteur de vote et le notifier IPC.
///
/// La boucle principale bloque sur `select!` entre le channel de scores et le
/// channel de stop : **zéro polling, zéro CPU en idle**.
pub struct TriggerRunner {
    config: TriggerConfig,
    thread_handle: Option<JoinHandle<()>>,
    stop_tx: Option<Sender<()>>,
}

impl TriggerRunner {
    /// Crée un runner à partir d'une configuration.
    pub fn new(config: &TriggerConfig) -> Self {
        Self {
            config: config.clone(),
            thread_handle: None,
            stop_tx: None,
        }
    }

    /// Démarre le thread trigger qui consomme les scores du `Receiver`.
    ///
    /// Le thread se termine si :
    /// - Le `Sender<f32>` est droppé (channel fermé).
    /// - `stop()` est appelé.
    pub fn start(&mut self, rx: Receiver<f32>) -> Result<(), TriggerError> {
        let mut engine = TriggerEngine::new(&self.config);
        let notifier = IpcNotifier::new(self.config.socket_path.clone());
        let (stop_tx, stop_rx) = crossbeam_channel::bounded::<()>(1);
        self.stop_tx = Some(stop_tx);

        let handle = thread::spawn(move || loop {
            crossbeam_channel::select! {
                recv(rx) -> msg => match msg {
                    Ok(score) => {
                        tracing::trace!("TriggerRunner: score = {:.4}", score);
                        if engine.push(score) {
                            tracing::info!(
                                "WAKE-WORD DETECTED — score window cleared, notifying IPC"
                            );
                            if let Err(e) = notifier.notify() {
                                tracing::warn!("IPC notify failed: {}", e);
                            }
                        }
                    }
                    Err(_) => {
                        tracing::debug!("TriggerRunner: score channel closed, exiting");
                        break;
                    }
                },
                recv(stop_rx) -> _ => {
                    tracing::debug!("TriggerRunner: stop requested, exiting");
                    break;
                },
            }
        });

        self.thread_handle = Some(handle);
        Ok(())
    }

    /// Arrête proprement le thread trigger et attend sa terminaison.
    pub fn stop(&mut self) {
        // Dropper stop_tx ferme le channel → select! se débloque et sort
        drop(self.stop_tx.take());
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for TriggerRunner {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn unique_socket_path() -> String {
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        format!("/tmp/wakeword_runner_test_{}.sock", id)
    }

    fn cleanup(path: &str) {
        let _ = std::fs::remove_file(path);
    }

    fn test_config(socket_path: &str) -> TriggerConfig {
        TriggerConfig {
            socket_path: socket_path.to_string(),
            window_size: 5,
            vote_threshold: 3,
            score_threshold: 0.80,
            cooldown_ms: 200, // court pour les tests
        }
    }

    fn read_message(listener: &UnixListener) -> Vec<u8> {
        let (mut stream, _) = listener.accept().expect("accept failed");
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).expect("read failed");
        buf
    }

    // ─── Test 1 : détection → socket notifié ─────────────────────────────────

    #[test]
    fn runner_detects_and_sends_to_socket() {
        let path = unique_socket_path();
        cleanup(&path);

        let listener = UnixListener::bind(&path).expect("bind failed");
        let mut runner = TriggerRunner::new(&test_config(&path));
        let (tx, rx) = crossbeam_channel::unbounded::<f32>();
        runner.start(rx).expect("start failed");

        tx.send(0.9).unwrap();
        tx.send(0.5).unwrap();
        tx.send(0.9).unwrap();
        tx.send(0.5).unwrap();
        tx.send(0.9).unwrap(); // 3e vote positif → détection

        assert_eq!(read_message(&listener), b"WAKEWORD_DETECTED\n");
        drop(tx);
        cleanup(&path);
    }

    // ─── Test 2 : aucune détection sous le seuil ─────────────────────────────

    #[test]
    fn runner_no_detection_below_threshold() {
        let path = unique_socket_path();
        cleanup(&path);

        let listener = UnixListener::bind(&path).expect("bind failed");
        listener.set_nonblocking(true).expect("set_nonblocking failed");

        let mut runner = TriggerRunner::new(&test_config(&path));
        let (tx, rx) = crossbeam_channel::unbounded::<f32>();
        runner.start(rx).expect("start failed");

        for _ in 0..10 {
            tx.send(0.3).unwrap();
        }

        // Attendre que le thread ait tout traité avant de vérifier
        drop(tx);
        runner.stop();

        assert!(
            listener.accept().is_err(),
            "Aucune connexion ne devrait avoir été établie"
        );
        cleanup(&path);
    }

    // ─── Test 3 : fermeture du channel → thread se termine proprement ────────

    #[test]
    fn runner_channel_close_terminates_cleanly() {
        let path = unique_socket_path();
        cleanup(&path);

        let mut runner = TriggerRunner::new(&test_config(&path));
        let (tx, rx) = crossbeam_channel::unbounded::<f32>();
        runner.start(rx).expect("start failed");

        drop(tx); // ferme le channel
        runner.stop(); // doit retourner sans panique ni hang
        cleanup(&path);
    }

    // ─── Test 4 : Drop sans stop → zéro thread zombie ────────────────────────

    #[test]
    fn runner_drop_without_stop_no_zombie() {
        let path = unique_socket_path();
        cleanup(&path);

        let mut runner = TriggerRunner::new(&test_config(&path));
        let (tx, rx) = crossbeam_channel::unbounded::<f32>();
        runner.start(rx).expect("start failed");

        drop(tx);
        drop(runner); // Drop impl appelle stop() → zéro zombie
        cleanup(&path);
    }

    // ─── Test 5 : cooldown bloque le second déclenchement immédiat ───────────

    #[test]
    fn runner_cooldown_blocks_rapid_second_trigger() {
        let path = unique_socket_path();
        cleanup(&path);

        let listener = UnixListener::bind(&path).expect("bind failed");
        let mut runner = TriggerRunner::new(&test_config(&path));
        let (tx, rx) = crossbeam_channel::unbounded::<f32>();
        runner.start(rx).expect("start failed");

        // Première détection
        tx.send(0.9).unwrap();
        tx.send(0.9).unwrap();
        tx.send(0.9).unwrap();
        assert_eq!(read_message(&listener), b"WAKEWORD_DETECTED\n");

        // Immédiatement : 5 nouveaux scores positifs → cooldown bloque
        for _ in 0..5 {
            tx.send(0.9).unwrap();
        }

        drop(tx);
        runner.stop(); // garantit que tout est traité

        listener.set_nonblocking(true).expect("set_nonblocking failed");
        assert!(
            listener.accept().is_err(),
            "Le cooldown aurait dû bloquer le second déclenchement"
        );
        cleanup(&path);
    }

    // ─── Test 6 : double détection après expiration du cooldown ──────────────

    #[test]
    fn runner_double_detection_after_cooldown() {
        let path = unique_socket_path();
        cleanup(&path);

        let listener = UnixListener::bind(&path).expect("bind failed");
        let cfg = TriggerConfig {
            cooldown_ms: 100,
            ..test_config(&path)
        };
        let mut runner = TriggerRunner::new(&cfg);
        let (tx, rx) = crossbeam_channel::unbounded::<f32>();
        runner.start(rx).expect("start failed");

        // Première détection
        tx.send(0.9).unwrap();
        tx.send(0.9).unwrap();
        tx.send(0.9).unwrap();
        assert_eq!(read_message(&listener), b"WAKEWORD_DETECTED\n");

        // Attendre l'expiration du cooldown
        std::thread::sleep(Duration::from_millis(200));

        // Deuxième détection
        tx.send(0.9).unwrap();
        tx.send(0.9).unwrap();
        tx.send(0.9).unwrap();
        assert_eq!(read_message(&listener), b"WAKEWORD_DETECTED\n");

        drop(tx);
        cleanup(&path);
    }
}
