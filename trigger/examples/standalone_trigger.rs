//! Exemple standalone du module trigger.
//!
//! Génère des séquences de scores synthétiques, instancie `TriggerModule`,
//! et écoute les notifications sur un socket local.
//!
//! Utilisation :
//! ```bash
//! cargo run --example standalone_trigger --features standalone
//! ```

#[cfg(feature = "standalone")]
fn main() {
    use std::io::Read;
    use std::os::unix::net::UnixListener;
    use std::thread;
    use std::time::Duration;
    use trigger::{TriggerConfig, TriggerModule};

    // Initialisation des logs
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let socket_path = "/tmp/wakeword_standalone.sock";
    let _ = std::fs::remove_file(socket_path);

    let listener = UnixListener::bind(socket_path).expect("bind failed");
    listener
        .set_nonblocking(true)
        .expect("set_nonblocking failed");

    let config = TriggerConfig {
        socket_path: socket_path.to_string(),
        cooldown_ms: 3000,
        ..TriggerConfig::default()
    };

    let mut module = TriggerModule::new(config).expect("module creation failed");
    let (tx, rx) = crossbeam_channel::unbounded::<f32>();
    module.start(rx).expect("start failed");

    println!("=== Standalone trigger — démo ===");
    println!("Socket : {}", socket_path);
    println!("Vague 1 : scores positifs (devrait déclencher)");

    // Vague 1 : scores positifs → doit déclencher
    for score in [0.9f32, 0.5, 0.9, 0.5, 0.9] {
        println!("  → envoi score {:.2}", score);
        tx.send(score).unwrap();
        thread::sleep(Duration::from_millis(50));
    }
    thread::sleep(Duration::from_millis(200));

    // Vérifier le socket
    match listener.accept() {
        Ok((mut stream, _)) => {
            let mut buf = Vec::new();
            stream.read_to_end(&mut buf).unwrap();
            println!("  ✓ Message reçu : {:?}", String::from_utf8_lossy(&buf));
        }
        Err(_) => println!("  (aucun message reçu sur le socket)"),
    }

    println!("Vague 2 : scores négatifs (ne devrait pas déclencher)");
    for _ in 0..10 {
        tx.send(0.3).unwrap();
        thread::sleep(Duration::from_millis(30));
    }
    thread::sleep(Duration::from_millis(200));

    match listener.accept() {
        Ok(_) => println!("  (inattendu) Message reçu"),
        Err(_) => println!("  ✓ Aucun déclenchement (correct)"),
    }

    println!("Vague 3 : nouvelle vague positive après 3 s de cooldown");
    thread::sleep(Duration::from_millis(3100));

    for score in [0.85f32, 0.4, 0.92, 0.4, 0.88] {
        println!("  → envoi score {:.2}", score);
        tx.send(score).unwrap();
        thread::sleep(Duration::from_millis(50));
    }
    thread::sleep(Duration::from_millis(200));

    match listener.accept() {
        Ok((mut stream, _)) => {
            let mut buf = Vec::new();
            stream.read_to_end(&mut buf).unwrap();
            println!("  ✓ Message reçu : {:?}", String::from_utf8_lossy(&buf));
        }
        Err(_) => println!("  (aucun message reçu sur le socket)"),
    }

    drop(tx);
    module.stop().expect("stop failed");
    let _ = std::fs::remove_file(socket_path);
    println!("=== Fin de la démo ===");
}

#[cfg(not(feature = "standalone"))]
fn main() {
    eprintln!(
        "Cet exemple nécessite la feature `standalone`.\n\
        Lancez : cargo run --example standalone_trigger --features standalone"
    );
}
