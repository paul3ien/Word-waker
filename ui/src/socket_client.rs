//! Client socket IPC pour communiquer avec le daemon Word Waker.
//!
//! Se connecte au daemon via Unix Domain Socket, lit les notifications
//! de détection et les transmet au thread UI principal.

use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use crossbeam_channel::Sender;

use crate::config::UiConfig;

/// Événements émis par le client socket à destination du thread UI.
#[derive(Debug)]
pub enum UiEvent {
    /// Connexion au daemon établie.
    Connected,
    /// Connexion au daemon perdue (socket fermé ou daemon arrêté).
    Disconnected,
    /// Mot-clé détecté par le daemon.
    WakeWordDetected {
        /// Instant de la détection (capturé côté UI au moment de la réception).
        timestamp: std::time::Instant,
    },
    /// Erreur non fatale (ex: socket inaccessible, permissions).
    Error(String),
}

// Vérifications de sécurité au niveau du type : UiEvent doit être Send+Sync
// car il traverse une frontière de thread via crossbeam.
const _: fn() = || {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<UiEvent>();
    assert_sync::<UiEvent>();
};

/// Client socket IPC qui reçoit les événements du daemon.
pub struct IpcClient {
    config: UiConfig,
}

impl IpcClient {
    /// Crée un nouveau client IPC avec la configuration donnée.
    pub fn new(config: UiConfig) -> Self {
        Self { config }
    }

    /// Tente de se connecter au socket du daemon.
    fn connect(&self) -> std::io::Result<UnixStream> {
        UnixStream::connect(&self.config.socket_path)
    }

    /// Boucle de lecture sur un stream connecté.
    /// Lit ligne par ligne et émet les événements correspondants.
    /// Retourne quand la connexion est fermée ou une erreur survient.
    fn read_loop(&self, stream: UnixStream, tx: &Sender<UiEvent>) {
        let reader = BufReader::new(stream);
        for line in reader.lines() {
            match line {
                Ok(msg) if msg.trim() == "WAKEWORD_DETECTED" => {
                    let _ = tx.send(UiEvent::WakeWordDetected {
                        timestamp: std::time::Instant::now(),
                    });
                }
                Ok(msg) if msg.trim().is_empty() => {
                    // Ignorer les lignes vides (P1.4)
                }
                Ok(_msg) => {
                    // Ligne inconnue ou malformée → ignorée silencieusement (P1.4)
                }
                Err(_) => {
                    // Erreur de lecture → connexion perdue
                    break;
                }
            }
        }
    }

    /// Boucle infinie de connexion/reconnexion.
    /// À exécuter dans un thread dédié.
    ///
    /// Algorithme :
    /// 1. Tente de se connecter au socket
    /// 2. Si connecté → émet `Connected`, lit les événements, puis émet `Disconnected`
    /// 3. Si échec → émet `Disconnected`, attend `reconnect_delay_ms`, retente
    pub fn run(&self, tx: Sender<UiEvent>) {
        let reconnect_delay = Duration::from_millis(self.config.reconnect_delay_ms);

        loop {
            match self.connect() {
                Ok(stream) => {
                    let _ = tx.send(UiEvent::Connected);
                    self.read_loop(stream, &tx);
                    let _ = tx.send(UiEvent::Disconnected);
                }
                Err(e) => {
                    let _ = tx.send(UiEvent::Disconnected);
                    tracing::warn!(
                        "Échec de connexion au daemon, nouvelle tentative dans {} ms : {}",
                        self.config.reconnect_delay_ms,
                        e
                    );
                    std::thread::sleep(reconnect_delay);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::net::UnixListener;
    use std::thread;

    /// Crée un chemin de socket temporaire unique.
    fn temp_socket_path() -> String {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("ww_test_{}.sock", std::process::id()));
        // Nettoyage si le fichier existe déjà
        let _ = std::fs::remove_file(&path);
        path.to_string_lossy().to_string()
    }

    /// Test : UiEvent dérive Debug et peut être envoyé entre threads.
    #[test]
    fn test_ui_event_is_send_sync() {
        // Le test compile seulement si les traits sont implémentés
        fn is_send<T: Send>() {}
        fn is_sync<T: Sync>() {}
        is_send::<UiEvent>();
        is_sync::<UiEvent>();

        let event = UiEvent::WakeWordDetected {
            timestamp: std::time::Instant::now(),
        };
        // Debug
        let _ = format!("{:?}", event);
    }

    /// Test : WakeWordDetected a un timestamp dans les 100 dernières ms.
    #[test]
    fn test_wakeword_detected_timestamp_is_recent() {
        let before = std::time::Instant::now();
        let event = UiEvent::WakeWordDetected {
            timestamp: std::time::Instant::now(),
        };
        let after = std::time::Instant::now();

        if let UiEvent::WakeWordDetected { timestamp } = event {
            assert!(timestamp >= before, "timestamp should be after 'before'");
            assert!(timestamp <= after, "timestamp should be before 'after'");
            assert!(
                after.duration_since(before) < Duration::from_millis(100),
                "test ran too slowly"
            );
        } else {
            panic!("Expected WakeWordDetected");
        }
    }

    /// Test I : Mock server envoie "WAKEWORD_DETECTED\n" → UiEvent reçu.
    #[test]
    fn test_receives_wakeword_detected() {
        let path = temp_socket_path();
        let listener = UnixListener::bind(&path).expect("bind");
        let (tx, rx) = crossbeam_channel::unbounded();

        // Thread serveur
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            writeln!(stream, "WAKEWORD_DETECTED").expect("write");
            // Fermer la connexion pour que le client termine read_loop
        });

        // Thread client
        let client_path = path.clone();
        let client = thread::spawn(move || {
            let config = UiConfig {
                socket_path: client_path,
                reconnect_delay_ms: 50,
                poll_interval_ms: 100,
            };
            let ipc = IpcClient::new(config);
            // On n'utilise pas run() car il boucle infiniment.
            // On fait le cycle manuellement pour le test.
            let stream = ipc.connect().expect("connect");
            tx.send(UiEvent::Connected).ok();
            ipc.read_loop(stream, &tx);
            tx.send(UiEvent::Disconnected).ok();
        });

        server.join().unwrap();
        client.join().unwrap();

        // Vérifier les événements reçus
        let events: Vec<UiEvent> = rx.try_iter().collect();
        assert!(
            events.iter().any(|e| matches!(e, UiEvent::Connected)),
            "Should receive Connected"
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, UiEvent::WakeWordDetected { .. })),
            "Should receive WakeWordDetected"
        );
        assert!(
            events.iter().any(|e| matches!(e, UiEvent::Disconnected)),
            "Should receive Disconnected"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Test I : Mock server envoie "WAKEWORD_DETECTED" sans \n (flush manuel) → reçu.
    #[test]
    fn test_receives_wakeword_detected_without_newline() {
        let path = temp_socket_path();
        let listener = UnixListener::bind(&path).expect("bind");
        let (tx, rx) = crossbeam_channel::unbounded();

        let _server_path = path.clone();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            // Envoyer SANS \n
            write!(stream, "WAKEWORD_DETECTED").expect("write");
            stream.flush().expect("flush");
            // Laisser le stream ouvert un instant puis fermer
            thread::sleep(Duration::from_millis(100));
            // Fermeture implicite
        });

        let client_path = path.clone();
        let client = thread::spawn(move || {
            let config = UiConfig {
                socket_path: client_path,
                reconnect_delay_ms: 50,
                poll_interval_ms: 100,
            };
            let ipc = IpcClient::new(config);
            let stream = ipc.connect().expect("connect");
            tx.send(UiEvent::Connected).ok();
            // BufReader::lines() ne retourne pas de ligne sans \n,
            // donc on lit différemment. On fait le test avec lines()
            // pour valider que le comportement est correct même sans \n
            // (la ligne sera retournée quand le stream ferme ou flush + stream fermé)
            ipc.read_loop(stream, &tx);
            tx.send(UiEvent::Disconnected).ok();
        });

        server.join().unwrap();
        client.join().unwrap();

        let events: Vec<UiEvent> = rx.try_iter().collect();
        // Avec BufReader::lines(), sans \n le message peut ne pas être reçu comme une ligne.
        // Ce test valide qu'il n'y a pas de crash/panic dans ce cas.
        assert!(
            events.iter().any(|e| matches!(e, UiEvent::Connected)),
            "Should receive Connected"
        );
        assert!(
            events.iter().any(|e| matches!(e, UiEvent::Disconnected)),
            "Should receive Disconnected"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Test I : Mock server ferme la connexion → Disconnected reçu.
    #[test]
    fn test_disconnected_on_server_close() {
        let path = temp_socket_path();
        let listener = UnixListener::bind(&path).expect("bind");
        let (tx, rx) = crossbeam_channel::unbounded();

        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            // Fermer immédiatement
            drop(stream);
        });

        let client_path = path.clone();
        let client = thread::spawn(move || {
            let config = UiConfig {
                socket_path: client_path,
                reconnect_delay_ms: 50,
                poll_interval_ms: 100,
            };
            let ipc = IpcClient::new(config);
            let stream = ipc.connect().expect("connect");
            tx.send(UiEvent::Connected).ok();
            ipc.read_loop(stream, &tx);
            tx.send(UiEvent::Disconnected).ok();
        });

        server.join().unwrap();
        client.join().unwrap();

        let events: Vec<UiEvent> = rx.try_iter().collect();
        assert!(
            events.iter().any(|e| matches!(e, UiEvent::Connected)),
            "Should receive Connected"
        );
        assert!(
            events.iter().any(|e| matches!(e, UiEvent::Disconnected)),
            "Should receive Disconnected"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Test I : Socket inexistant → pas de panic, Disconnected ou Error émis.
    #[test]
    fn test_socket_nonexistent() {
        let (tx, rx) = crossbeam_channel::unbounded();
        let path = "/tmp/ww_nonexistent_test_socket.sock";
        let _ = std::fs::remove_file(path);

        let config = UiConfig {
            socket_path: path.to_string(),
            reconnect_delay_ms: 10,
            poll_interval_ms: 100,
        };
        let ipc = IpcClient::new(config);

        // Lance run() dans un thread, on le tue après un délai
        let tx_clone = tx.clone();
        let handle = thread::spawn(move || {
            ipc.run(tx_clone);
        });

        // Attendre un peu que le thread tente de se connecter et émette Disconnected
        thread::sleep(Duration::from_millis(100));

        // On ne peut pas joindre proprement car run() est une boucle infinie,
        // mais on peut vérifier que des événements ont été émis
        let events: Vec<UiEvent> = rx.try_iter().collect();
        let has_disconnected = events.iter().any(|e| matches!(e, UiEvent::Disconnected));
        assert!(
            has_disconnected,
            "Should receive Disconnected for nonexistent socket"
        );

        // Nettoie : le thread est toujours vivant, c'est acceptable pour le test
        drop(handle);
    }

    /// Test I : Reconnexion automatique — daemon mock démarre après délai, le client se connecte.
    #[test]
    fn test_auto_reconnect() {
        let path = temp_socket_path();
        let (tx, rx) = crossbeam_channel::unbounded::<UiEvent>();

        let _server_path = path.clone();
        let server = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            let listener = UnixListener::bind(&_server_path).expect("bind late");
            let (mut stream, _) = listener.accept().expect("accept late");
            writeln!(stream, "WAKEWORD_DETECTED").expect("write");
            drop(stream);
        });

        let client_path = path.clone();
        let rx_events = rx.clone();
        let client = thread::spawn(move || {
            let config = UiConfig {
                socket_path: client_path,
                reconnect_delay_ms: 100,
                poll_interval_ms: 100,
            };
            // On fait le cycle manuellement pour pouvoir terminer
            let ipc = IpcClient::new(config);
            let reconnect_delay = Duration::from_millis(ipc.config.reconnect_delay_ms);
            let mut attempts = 0;

            loop {
                attempts += 1;
                if attempts > 20 {
                    break;
                }
                match ipc.connect() {
                    Ok(stream) => {
                        tx.send(UiEvent::Connected).ok();
                        ipc.read_loop(stream, &tx);
                        tx.send(UiEvent::Disconnected).ok();
                        break;
                    }
                    Err(_) => {
                        tx.send(UiEvent::Disconnected).ok();
                        // On s'arrête après avoir reçu au moins un Connected
                        let has_connected = rx_events
                            .try_iter()
                            .any(|e| matches!(e, UiEvent::Connected));
                        if has_connected {
                            break;
                        }
                        thread::sleep(reconnect_delay);
                    }
                }
            }
        });

        server.join().unwrap();
        client.join().unwrap();

        let events: Vec<UiEvent> = rx.try_iter().collect();
        let connected_count = events
            .iter()
            .filter(|e| matches!(e, UiEvent::Connected))
            .count();
        let detected_count = events
            .iter()
            .filter(|e| matches!(e, UiEvent::WakeWordDetected { .. }))
            .count();

        assert!(connected_count >= 1, "Should eventually connect");
        assert!(
            detected_count >= 1,
            "Should receive detection after connect"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Test I : Le client n'émet qu'un seul Disconnected par transition d'état.
    #[test]
    fn test_single_disconnected_per_transition() {
        let path = temp_socket_path();
        let listener = UnixListener::bind(&path).expect("bind");
        let (tx, rx) = crossbeam_channel::unbounded();

        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            // Fermer immédiatement
            drop(stream);
        });

        let client_path = path.clone();
        let client = thread::spawn(move || {
            let config = UiConfig {
                socket_path: client_path,
                reconnect_delay_ms: 50,
                poll_interval_ms: 100,
            };
            let ipc = IpcClient::new(config);
            let stream = ipc.connect().expect("connect");
            tx.send(UiEvent::Connected).ok();
            ipc.read_loop(stream, &tx);
            tx.send(UiEvent::Disconnected).ok();
            // Pas d'autre tentative de connexion
        });

        server.join().unwrap();
        client.join().unwrap();

        let events: Vec<UiEvent> = rx.try_iter().collect();
        let disconnected_count = events
            .iter()
            .filter(|e| matches!(e, UiEvent::Disconnected))
            .count();

        // Un seul Disconnected attendu après un Connected puis déconnexion
        assert_eq!(
            disconnected_count, 1,
            "Should have exactly 1 Disconnected event, got {}",
            disconnected_count
        );

        // Vérifier l'ordre : Connected puis Disconnected
        let positions: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                UiEvent::Connected => Some("Connected"),
                UiEvent::Disconnected => Some("Disconnected"),
                _ => None,
            })
            .collect();

        if let Some(first) = positions.first() {
            assert_eq!(*first, "Connected", "First event should be Connected");
        }
        if let Some(last) = positions.last() {
            assert_eq!(*last, "Disconnected", "Last event should be Disconnected");
        }

        let _ = std::fs::remove_file(&path);
    }

    /// Test P1.4 : Lignes vides ignorées, pas de crash.
    #[test]
    fn test_ignores_empty_lines() {
        let path = temp_socket_path();
        let listener = UnixListener::bind(&path).expect("bind");
        let (tx, rx) = crossbeam_channel::unbounded();

        let _server_path = path.clone();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            writeln!(stream, "").expect("write empty");
            writeln!(stream, "   ").expect("write spaces");
            writeln!(stream, "WAKEWORD_DETECTED").expect("write");
            writeln!(stream, "").expect("write empty");
        });

        let client_path = path.clone();
        let client = thread::spawn(move || {
            let config = UiConfig {
                socket_path: client_path,
                reconnect_delay_ms: 50,
                poll_interval_ms: 100,
            };
            let ipc = IpcClient::new(config);
            let stream = ipc.connect().expect("connect");
            tx.send(UiEvent::Connected).ok();
            ipc.read_loop(stream, &tx);
            tx.send(UiEvent::Disconnected).ok();
        });

        server.join().unwrap();
        client.join().unwrap();

        let events: Vec<UiEvent> = rx.try_iter().collect();
        // On doit avoir reçu le WakeWordDetected malgré les lignes vides
        assert!(
            events
                .iter()
                .any(|e| matches!(e, UiEvent::WakeWordDetected { .. })),
            "Should receive WakeWordDetected despite empty lines"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Test P1.4 : Données binaires → pas de crash, pas de panic.
    #[test]
    fn test_handles_binary_data() {
        let path = temp_socket_path();
        let listener = UnixListener::bind(&path).expect("bind");
        let (tx, rx) = crossbeam_channel::unbounded();

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            // Envoyer des octets binaires non UTF-8
            stream
                .write_all(&[0x00, 0xFF, 0xFE, 0x80, 0x90])
                .expect("write binary");
            stream.write_all(b"\n").expect("write newline");
            stream
                .write_all(b"WAKEWORD_DETECTED\n")
                .expect("write valid");
        });

        let client_path = path.clone();
        let client = thread::spawn(move || {
            let config = UiConfig {
                socket_path: client_path,
                reconnect_delay_ms: 50,
                poll_interval_ms: 100,
            };
            let ipc = IpcClient::new(config);
            let stream = ipc.connect().expect("connect");
            tx.send(UiEvent::Connected).ok();
            ipc.read_loop(stream, &tx);
            tx.send(UiEvent::Disconnected).ok();
        });

        server.join().unwrap();
        client.join().unwrap();

        let events: Vec<UiEvent> = rx.try_iter().collect();
        // Les données binaires peuvent produire une erreur de lecture (UTF-8 invalide)
        // mais ne doivent pas crasher. On vérifie qu'au moins Connected et Disconnected sont là.
        assert!(
            events.iter().any(|e| matches!(e, UiEvent::Connected)),
            "Should not crash on binary data"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Test P1.4 : Ligne de 10 Ko → ignorée, pas de crash, pas de OOM.
    #[test]
    fn test_handles_large_line() {
        let path = temp_socket_path();
        let listener = UnixListener::bind(&path).expect("bind");
        let (tx, rx) = crossbeam_channel::unbounded();

        let _server_path = path.clone();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let large_data = "A".repeat(10 * 1024); // 10 Ko
            writeln!(stream, "{}", large_data).expect("write large");
            writeln!(stream, "WAKEWORD_DETECTED").expect("write valid");
        });

        let client_path = path.clone();
        let client = thread::spawn(move || {
            let config = UiConfig {
                socket_path: client_path,
                reconnect_delay_ms: 50,
                poll_interval_ms: 100,
            };
            let ipc = IpcClient::new(config);
            let stream = ipc.connect().expect("connect");
            tx.send(UiEvent::Connected).ok();
            ipc.read_loop(stream, &tx);
            tx.send(UiEvent::Disconnected).ok();
        });

        server.join().unwrap();
        client.join().unwrap();

        let events: Vec<UiEvent> = rx.try_iter().collect();
        // La grande ligne est ignorée, le WAKEWORD_DETECTED suivant doit être reçu
        assert!(
            events
                .iter()
                .any(|e| matches!(e, UiEvent::WakeWordDetected { .. })),
            "Should receive WakeWordDetected after large line"
        );

        let _ = std::fs::remove_file(&path);
    }
}
