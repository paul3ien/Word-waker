//! Suite de tests de régression et d'intégration du crate `trigger`.
//!
//! - Chaque test utilise un path de socket unique pour éviter les conflits en parallèle.
//! - Chaque test nettoie son socket en fin d'exécution.

use std::io::Read;
use std::os::unix::net::UnixListener;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use trigger::{IpcNotifier, TriggerConfig, TriggerEngine, TriggerError, TriggerModule};

// ─── Utilitaires ──────────────────────────────────────────────────────────────

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn unique_socket(label: &str) -> String {
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("/tmp/wakeword_regr_{}_{}.sock", label, id)
}

fn cleanup(path: &str) {
    let _ = std::fs::remove_file(path);
}

fn read_message(listener: &UnixListener) -> Vec<u8> {
    let (mut stream, _) = listener.accept().expect("accept failed");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).expect("read failed");
    buf
}

fn default_test_config(socket_path: &str) -> TriggerConfig {
    TriggerConfig {
        socket_path: socket_path.to_string(),
        cooldown_ms: 200,
        ..TriggerConfig::default()
    }
}

// ─── P7.2 — Tests unitaires de régression ─────────────────────────────────────

#[test]
fn regression_default_config_is_valid() {
    assert!(TriggerConfig::default().validate().is_ok());
}

#[test]
fn regression_all_trigger_error_variants_have_nonempty_display() {
    let variants: Vec<Box<dyn std::fmt::Display>> = vec![
        Box::new(TriggerError::ChannelClosed),
        Box::new(TriggerError::IpcSendFailed("err".to_string())),
        Box::new(TriggerError::SocketBindFailed("err".to_string())),
        Box::new(TriggerError::InvalidConfig("err".to_string())),
    ];
    for v in &variants {
        assert!(!v.to_string().is_empty());
    }
}

#[test]
fn regression_3_positive_votes_on_5_triggers() {
    let mut engine = TriggerEngine::new(&TriggerConfig::default());
    assert!(!engine.push(0.9));
    assert!(!engine.push(0.5));
    assert!(!engine.push(0.9));
    assert!(!engine.push(0.5));
    assert!(engine.push(0.9));
}

#[test]
fn regression_2_positive_votes_on_5_no_trigger() {
    let mut engine = TriggerEngine::new(&TriggerConfig::default());
    for score in [0.9f32, 0.5, 0.9, 0.5, 0.5] {
        assert!(!engine.push(score));
    }
}

#[test]
fn regression_cooldown_blocks_immediate_second_trigger() {
    let mut engine = TriggerEngine::new(&TriggerConfig::default());
    // Première détection
    assert!(!engine.push(0.9));
    assert!(!engine.push(0.9));
    assert!(engine.push(0.9));
    // Immédiatement : cooldown bloque
    assert!(!engine.push(0.9));
    assert!(!engine.push(0.9));
    assert!(!engine.push(0.9));
}

#[test]
fn regression_history_cleared_after_detection() {
    let mut engine = TriggerEngine::new(&TriggerConfig::default());
    assert!(!engine.push(0.9));
    assert!(!engine.push(0.9));
    assert!(engine.push(0.9));
    assert_eq!(engine.history_len(), 0);
}

#[test]
fn regression_reset_clears_state() {
    let mut engine = TriggerEngine::new(&TriggerConfig::default());
    engine.push(0.9);
    engine.push(0.9);
    engine.reset();
    assert_eq!(engine.pending_votes(), 0);
    assert_eq!(engine.history_len(), 0);
}

// ─── P7.3 — Tests d'intégration de régression ─────────────────────────────────

#[test]
fn regression_notify_without_client_returns_ok() {
    let path = unique_socket("no_client");
    cleanup(&path);
    let notifier = IpcNotifier::new(path.clone());
    assert!(notifier.notify().is_ok());
    cleanup(&path);
}

#[test]
fn regression_notify_with_client_sends_correct_message() {
    let path = unique_socket("with_client");
    cleanup(&path);
    let listener = UnixListener::bind(&path).expect("bind failed");
    let notifier = IpcNotifier::new(path.clone());
    notifier.notify().expect("notify failed");
    assert_eq!(read_message(&listener), b"WAKEWORD_DETECTED\n");
    cleanup(&path);
}

#[test]
fn regression_thread_channel_close_terminates_cleanly() {
    let path = unique_socket("chan_close");
    cleanup(&path);
    let mut module = TriggerModule::new(default_test_config(&path)).expect("new failed");
    let (tx, rx) = crossbeam_channel::unbounded::<f32>();
    module.start(rx).expect("start failed");
    drop(tx);
    module.stop().expect("stop failed");
    cleanup(&path);
}

#[test]
fn regression_drop_without_stop_no_zombie() {
    let path = unique_socket("drop_zombie");
    cleanup(&path);
    let mut module = TriggerModule::new(default_test_config(&path)).expect("new failed");
    let (tx, rx) = crossbeam_channel::unbounded::<f32>();
    module.start(rx).expect("start failed");
    drop(tx);
    drop(module);
    cleanup(&path);
}

#[test]
fn regression_two_detections_separated_by_cooldown() {
    let path = unique_socket("two_detections");
    cleanup(&path);

    let listener = UnixListener::bind(&path).expect("bind failed");
    let cfg = TriggerConfig {
        cooldown_ms: 100,
        ..default_test_config(&path)
    };
    let mut module = TriggerModule::new(cfg).expect("new failed");
    let (tx, rx) = crossbeam_channel::unbounded::<f32>();
    module.start(rx).expect("start failed");

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
    module.stop().expect("stop failed");
    cleanup(&path);
}

// ─── P7.4 — Tests de performance ──────────────────────────────────────────────

/// Latence d'un appel `TriggerEngine::push` doit être < 1 µs.
/// On mesure sur 10 000 appels pour stabiliser.
#[test]
fn perf_push_latency_under_1us() {
    let mut engine = TriggerEngine::new(&TriggerConfig {
        cooldown_ms: 1, // cooldown très court pour éviter de bloquer les mesures
        ..TriggerConfig::default()
    });

    let iterations = 10_000usize;
    let start = Instant::now();
    for i in 0..iterations {
        engine.push(if i % 7 == 0 { 0.9 } else { 0.3 });
    }
    let elapsed = start.elapsed();
    let avg_ns = elapsed.as_nanos() / iterations as u128;

    assert!(
        avg_ns < 1_000,
        "Latence moyenne push trop élevée : {} ns (objectif < 1000 ns)",
        avg_ns
    );
}

/// Latence d'un appel `IpcNotifier::notify` avec client doit être < 5 ms.
#[test]
fn perf_notify_latency_under_5ms() {
    let path = unique_socket("perf_notify");
    cleanup(&path);

    let listener = UnixListener::bind(&path).expect("bind failed");
    let notifier = IpcNotifier::new(path.clone());

    // Mesure sur 10 appels (round-trip socket local)
    let iterations = 10usize;
    let mut total = Duration::ZERO;

    for _ in 0..iterations {
        // Spawner un accepteur en arrière-plan pour chaque connexion
        let listener_ref = &listener;
        let t = Instant::now();
        notifier.notify().expect("notify failed");
        total += t.elapsed();

        // Drainer le listener pour éviter ECONNREFUSED au prochain tour
        let (mut s, _) = listener_ref.accept().expect("accept failed");
        let mut buf = [0u8; 32];
        let _ = s.read(&mut buf);
    }

    let avg_ms = total.as_millis() / iterations as u128;
    assert!(
        avg_ms < 5,
        "Latence moyenne notify trop élevée : {} ms (objectif < 5 ms)",
        avg_ms
    );

    cleanup(&path);
}

// ─── P6.2 — Validation des métriques de déclenchement ─────────────────────────

/// P6.2 — Faux positifs : 8 h équivalent de scores uniformément distribués
/// entre 0.0 et 0.7 → aucun déclenchement (tous < threshold 0.80).
///
/// Simulation : 100 fps × 8 h × 3600 s = 2 880 000 scores.
#[test]
fn metric_false_positive_rate_zero_on_noise_scores() {
    let cfg = TriggerConfig {
        score_threshold: 0.80,
        vote_threshold: 3,
        window_size: 5,
        cooldown_ms: 2000,
        socket_path: "/tmp/unused_fp.sock".to_string(),
    };
    let mut engine = TriggerEngine::new(&cfg);

    // 8h × 3600 s × 100 inférences/s = 2 880 000 scores
    let n = 8 * 3600 * 100usize;
    let mut triggers = 0usize;

    for i in 0..n {
        // Distribution uniforme dans [0.0, 0.7] via LCG déterministe
        let raw = ((i as u64)
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407)
            >> 33) as f32
            / u32::MAX as f32;
        let score = raw * 0.7; // [0.0, 0.7)
        if engine.push(score) {
            triggers += 1;
        }
    }

    assert_eq!(
        triggers, 0,
        "P6.2 faux positifs : {} déclenchements sur {} scores bruit (attendu 0)",
        triggers, n
    );
}

/// P6.2 — Faux négatifs : 500 séquences idéales (3 scores > 0.80 sur 5)
/// → 100 % déclenchent (taux de faux négatifs = 0 %).
#[test]
fn metric_false_negative_rate_zero_on_ideal_sequences() {
    let cfg = TriggerConfig {
        score_threshold: 0.80,
        vote_threshold: 3,
        window_size: 5,
        cooldown_ms: 1, // minimal pour permettre 500 détections successives
        socket_path: "/tmp/unused_fn.sock".to_string(),
    };
    let mut engine = TriggerEngine::new(&cfg);

    let sequences = 500usize;
    let mut detected = 0usize;

    for _ in 0..sequences {
        engine.reset();
        let _ = engine.push(0.9);
        let _ = engine.push(0.5);
        let _ = engine.push(0.9);
        let _ = engine.push(0.5);
        if engine.push(0.9) {
            detected += 1;
        }
    }

    assert_eq!(
        detected, sequences,
        "P6.2 faux négatifs : {}/{} séquences détectées (attendu 100 %)",
        detected, sequences
    );
}

/// P6.2 — Latence IPC round-trip complète : `notify()` → réception socket < 5 ms.
#[test]
fn metric_ipc_round_trip_latency_under_5ms() {
    use std::io::Read;

    let path = unique_socket("metric_latency");
    cleanup(&path);

    let listener = UnixListener::bind(&path).expect("bind failed");
    let notifier = IpcNotifier::new(path.clone());

    let iterations = 20usize;
    let mut max_latency = Duration::ZERO;

    for _ in 0..iterations {
        let t0 = Instant::now();
        notifier.notify().expect("notify failed");
        let (mut stream, _) = listener.accept().expect("accept failed");
        let mut buf = [0u8; 32];
        let _ = stream.read(&mut buf);
        let elapsed = t0.elapsed();
        if elapsed > max_latency {
            max_latency = elapsed;
        }
        std::thread::sleep(Duration::from_millis(5));
    }

    assert!(
        max_latency < Duration::from_millis(5),
        "P6.2 latence IPC max = {:.2} ms (objectif < 5 ms)",
        max_latency.as_secs_f64() * 1000.0
    );

    cleanup(&path);
}

