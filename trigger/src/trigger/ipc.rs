use std::io::Write;
use std::os::unix::net::UnixStream;

use crate::trigger::TriggerError;

/// Notifier IPC — envoie un message sur un Unix Domain Socket.
///
/// La connexion est établie à chaque appel à `notify` (connexion courte).
/// Si aucun client n'écoute, l'échec est logué en `debug` et `Ok(())` est retourné
/// — **pas d'erreur fatale**.
pub struct IpcNotifier {
    socket_path: String,
}

impl IpcNotifier {
    /// Crée un notifier pointant vers `socket_path`.
    ///
    /// Aucune connexion n'est établie dans le constructeur.
    pub fn new(socket_path: String) -> Self {
        Self { socket_path }
    }

    /// Envoie `"WAKEWORD_DETECTED\n"` sur le socket.
    ///
    /// - Si aucun client n'écoute : log `debug`, retourne `Ok(())`.
    /// - Si l'écriture échoue après connexion : retourne `Err(IpcSendFailed)`.
    pub fn notify(&self) -> Result<(), TriggerError> {
        self.notify_with_payload(b"WAKEWORD_DETECTED\n")
    }

    /// Variante permettant un message customisé.
    ///
    /// Même sémantique que `notify` : silence gracieux si aucun client.
    pub fn notify_with_payload(&self, payload: &[u8]) -> Result<(), TriggerError> {
        match UnixStream::connect(&self.socket_path) {
            Ok(mut stream) => {
                stream.write_all(payload).map_err(|e| {
                    TriggerError::IpcSendFailed(format!(
                        "write_all failed on {}: {}",
                        self.socket_path, e
                    ))
                })
            }
            Err(e) => {
                tracing::debug!(
                    "IpcNotifier: no client on {} — {}",
                    self.socket_path,
                    e
                );
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::os::unix::net::UnixListener;

    fn tmp_socket_path(name: &str) -> String {
        format!("/tmp/wakeword_test_{}.sock", name)
    }

    fn cleanup(path: &str) {
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn new_does_not_connect() {
        // Aucune connexion dans new — ne doit pas paniquer même si le socket n'existe pas
        let _notifier = IpcNotifier::new("/tmp/nonexistent_test.sock".to_string());
    }

    #[test]
    fn notify_without_client_returns_ok() {
        let path = tmp_socket_path("no_client");
        cleanup(&path);
        let notifier = IpcNotifier::new(path);
        // Aucun listener → Ok silencieux
        assert!(notifier.notify().is_ok());
    }

    #[test]
    fn notify_with_client_sends_message() {
        let path = tmp_socket_path("with_client");
        cleanup(&path);

        let listener = UnixListener::bind(&path).expect("bind failed");

        let notifier = IpcNotifier::new(path.clone());
        notifier.notify().expect("notify failed");

        let (mut stream, _) = listener.accept().expect("accept failed");
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).expect("read failed");

        assert_eq!(buf, b"WAKEWORD_DETECTED\n");
        cleanup(&path);
    }

    #[test]
    fn notify_message_is_complete_and_exact() {
        let path = tmp_socket_path("exact_msg");
        cleanup(&path);

        let listener = UnixListener::bind(&path).expect("bind failed");

        let notifier = IpcNotifier::new(path.clone());
        notifier.notify().expect("notify failed");

        let (mut stream, _) = listener.accept().expect("accept failed");
        let mut buf = [0u8; 64];
        let n = stream.read(&mut buf).expect("read failed");

        assert_eq!(&buf[..n], b"WAKEWORD_DETECTED\n");
        assert_eq!(n, 18); // longueur exacte
        cleanup(&path);
    }

    #[test]
    fn notify_multiple_times_sends_multiple_messages() {
        let path = tmp_socket_path("multi_notify");
        cleanup(&path);

        let listener = UnixListener::bind(&path).expect("bind failed");
        let notifier = IpcNotifier::new(path.clone());

        for _ in 0..5 {
            notifier.notify().expect("notify failed");
        }

        let mut received = 0usize;
        // Accepter les 5 connexions
        for _ in 0..5 {
            let (mut stream, _) = listener.accept().expect("accept failed");
            let mut buf = Vec::new();
            stream.read_to_end(&mut buf).expect("read failed");
            assert_eq!(buf, b"WAKEWORD_DETECTED\n");
            received += 1;
        }
        assert_eq!(received, 5);
        cleanup(&path);
    }
}
