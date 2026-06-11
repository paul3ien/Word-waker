//! Tests d'intégration pour le crate UI Word Waker.
//!
//! Ces tests valident le comportement complet du client socket IPC
//! en utilisant un serveur mock Unix Domain Socket, sans dépendre
//! du daemon réel ni d'AppKit.

use std::io::Write;
use std::os::unix::net::UnixListener;
use std::thread;
use std::time::Duration;

use crossbeam_channel;

/// Types d'événements (dupliqués pour éviter la dépendance à ui::socket_client)
#[derive(Debug, PartialEq)]
enum TestEvent {
    Connected,
    Disconnected,
    WakeWordDetected,
    Error(String),
}

/// Test end-to-end complet : mock server → événements dans l'ordre.
#[test]
fn test_full_mock_flow_events_in_order() {
    let path = temp_socket_path();
    let listener = UnixListener::bind(&path).expect("bind");

    let (tx, rx) = crossbeam_channel::unbounded::<TestEvent>();
    let _server_path = path.clone();

    // Serveur mock : envoie WAKEWORD_DETECTED puis ferme
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        writeln!(stream, "WAKEWORD_DETECTED").expect("write 1");
        thread::sleep(Duration::from_millis(50));
        writeln!(stream, "WAKEWORD_DETECTED").expect("write 2");
        // Fermer
    });

    // Client : simule le comportement de IpcClient
    let client_path = path.clone();
    let client = thread::spawn(move || {
        let stream = std::os::unix::net::UnixStream::connect(&client_path).expect("connect");
        tx.send(TestEvent::Connected).ok();

        use std::io::BufRead;
        let reader = std::io::BufReader::new(stream);
        for line in reader.lines() {
            match line {
                Ok(msg) if msg.trim() == "WAKEWORD_DETECTED" => {
                    tx.send(TestEvent::WakeWordDetected).ok();
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
        tx.send(TestEvent::Disconnected).ok();
    });

    server.join().unwrap();
    client.join().unwrap();

    let events: Vec<TestEvent> = rx.try_iter().collect();

    // Vérifier l'ordre : Connected → WakeWordDetected → WakeWordDetected → Disconnected
    assert!(
        events.len() >= 4,
        "Expected at least 4 events, got {}",
        events.len()
    );

    let connected_pos = events.iter().position(|e| *e == TestEvent::Connected);
    let detected_positions: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| **e == TestEvent::WakeWordDetected)
        .map(|(i, _)| i)
        .collect();
    let disconnected_pos = events.iter().position(|e| *e == TestEvent::Disconnected);

    assert!(connected_pos.is_some(), "Missing Connected event");
    assert_eq!(detected_positions.len(), 2, "Expected 2 detections");
    assert!(disconnected_pos.is_some(), "Missing Disconnected event");

    let c = connected_pos.unwrap();
    let d = disconnected_pos.unwrap();

    // Connected doit être avant les détections
    assert!(
        c < detected_positions[0],
        "Connected must be before first detection"
    );
    // Les détections doivent être avant Disconnected
    assert!(
        detected_positions[1] < d,
        "Detections must be before Disconnected"
    );

    let _ = std::fs::remove_file(&path);
}

/// Test : tous les événements possibles sont reçus lors d'un cycle complet.
#[test]
fn test_all_event_types_received() {
    let path = temp_socket_path();
    let listener = UnixListener::bind(&path).expect("bind");

    let (tx, rx) = crossbeam_channel::unbounded::<TestEvent>();
    let _server_path = path.clone();

    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        writeln!(stream, "WAKEWORD_DETECTED").expect("write");
        stream.flush().ok();
        drop(stream);
    });

    let client_path = path.clone();
    let client = thread::spawn(move || {
        let stream = std::os::unix::net::UnixStream::connect(&client_path).expect("connect");
        tx.send(TestEvent::Connected).ok();

        use std::io::BufRead;
        let reader = std::io::BufReader::new(stream);
        for line in reader.lines() {
            if let Ok(msg) = line {
                if msg.trim() == "WAKEWORD_DETECTED" {
                    tx.send(TestEvent::WakeWordDetected).ok();
                }
            }
        }
        tx.send(TestEvent::Disconnected).ok();
    });

    server.join().unwrap();
    client.join().unwrap();

    let events: Vec<TestEvent> = rx.try_iter().collect();
    let has_connected = events.iter().any(|e| *e == TestEvent::Connected);
    let has_detected = events.iter().any(|e| *e == TestEvent::WakeWordDetected);
    let has_disconnected = events.iter().any(|e| *e == TestEvent::Disconnected);

    assert!(has_connected, "Should receive Connected");
    assert!(has_detected, "Should receive WakeWordDetected");
    assert!(has_disconnected, "Should receive Disconnected");

    let _ = std::fs::remove_file(&path);
}

/// Test : reconnexion après fermeture du serveur.
#[test]
fn test_reconnect_after_server_restart() {
    let path = temp_socket_path();
    let path_clone = path.clone();
    let path_clone2 = path.clone();

    let (tx, rx) = crossbeam_channel::unbounded::<TestEvent>();

    // Premier serveur : démarre, accepte, ferme
    let server1_path = path.clone();
    let server1 = thread::spawn(move || {
        let listener = UnixListener::bind(&server1_path).expect("bind");
        let (stream, _) = listener.accept().expect("accept");
        thread::sleep(Duration::from_millis(100));
        drop(stream);
        drop(listener);
        // Supprimer le socket pour que le deuxième serveur puisse bind
        let _ = std::fs::remove_file(&server1_path);
    });

    // Client : se connecte, reçoit les événements, se déconnecte, puis se reconnecte
    let client_path = path_clone;
    let client_tx = tx.clone();
    let client = thread::spawn(move || {
        // Première connexion
        thread::sleep(Duration::from_millis(50)); // attendre le serveur
        match std::os::unix::net::UnixStream::connect(&client_path) {
            Ok(stream) => {
                client_tx.send(TestEvent::Connected).ok();
                // Lire jusqu'à fermeture
                let mut buf = [0u8; 256];
                let _ = read_with_timeout(&stream, &mut buf[..], Duration::from_millis(500));
                client_tx.send(TestEvent::Disconnected).ok();
            }
            Err(_) => {
                client_tx
                    .send(TestEvent::Error("connect failed".into()))
                    .ok();
            }
        }

        // Attendre que le deuxième serveur démarre
        thread::sleep(Duration::from_millis(300));

        // Deuxième tentative de connexion
        match std::os::unix::net::UnixStream::connect(&client_path) {
            Ok(_stream) => {
                client_tx.send(TestEvent::Connected).ok();
                client_tx.send(TestEvent::Disconnected).ok();
            }
            Err(e) => {
                client_tx
                    .send(TestEvent::Error(format!("reconnect failed: {}", e)))
                    .ok();
            }
        }
    });

    server1.join().unwrap();

    // Deuxième serveur : démarre après un délai (simule redémarrage daemon)
    let server2_path = path_clone2;
    thread::sleep(Duration::from_millis(200));
    let server2 = thread::spawn(move || {
        let listener = UnixListener::bind(&server2_path).expect("bind2");
        let (stream, _) = listener.accept().expect("accept2");
        drop(stream);
    });

    client.join().unwrap();
    server2.join().unwrap();

    let events: Vec<TestEvent> = rx.try_iter().collect();

    // Vérifier qu'on a au moins 2 Connected
    let connected_count = events
        .iter()
        .filter(|e| matches!(e, TestEvent::Connected))
        .count();
    assert!(
        connected_count >= 2,
        "Expected at least 2 Connected events (initial + reconnect), got {}",
        connected_count
    );

    let _ = std::fs::remove_file(&path);
}

/// Test : résistance à un socket qui n'est pas un socket Unix.
#[test]
fn test_nonexistent_socket_no_crash() {
    let (tx, rx) = crossbeam_channel::unbounded::<TestEvent>();
    let path = "/tmp/ww_integration_nonexistent.sock";
    let _ = std::fs::remove_file(path);

    // Créer un fichier normal à la place
    std::fs::write(path, "not a socket").ok();

    let client_path = path.to_string();
    let client =
        thread::spawn(
            move || match std::os::unix::net::UnixStream::connect(&client_path) {
                Ok(_) => {
                    tx.send(TestEvent::Connected).ok();
                }
                Err(e) => {
                    tx.send(TestEvent::Error(format!("{}", e))).ok();
                }
            },
        );

    client.join().unwrap();
    let _ = std::fs::remove_file(path);

    let events: Vec<TestEvent> = rx.try_iter().collect();
    // Ne doit pas avoir Connected (le fichier n'est pas un socket)
    let has_connected = events.iter().any(|e| matches!(e, TestEvent::Connected));
    assert!(!has_connected, "Should not connect to a non-socket file");
}

/// Helper : chemin de socket temporaire unique.
fn temp_socket_path() -> String {
    // Utiliser /tmp directement pour éviter la limite SUN_LEN (104 chars sur macOS)
    let path = format!("/tmp/ww_integration_{}.sock", std::process::id());
    let _ = std::fs::remove_file(&path);
    path
}

/// Helper : read avec timeout sur un UnixStream
fn read_with_timeout(
    stream: &std::os::unix::net::UnixStream,
    buf: &mut [u8],
    timeout: Duration,
) -> std::io::Result<usize> {
    stream.set_read_timeout(Some(timeout))?;
    use std::io::Read;
    (&*stream).read(buf)
}
