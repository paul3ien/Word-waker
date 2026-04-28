//! Tests d'intégration du crate `audio_capture`.
//!
//! - Tests sans microphone (purs, mock) : compilent et passent avec
//!   `cargo test -p audio_capture --features mock_audio`
//! - Tests nécessitant un microphone physique : gardés sous
//!   `#[cfg(not(feature = "mock_audio"))]`

// ---------------------------------------------------------------------------
// P7.2 — Tests unitaires de régression (aucun hardware requis)
// ---------------------------------------------------------------------------

/// Config default → validate → Ok.
#[test]
fn regression_config_default_valide() {
    use audio_capture::AudioCaptureConfig;
    assert!(AudioCaptureConfig::default().validate().is_ok());
}

/// Toutes les variantes de AudioCaptureError ont un Display non vide.
#[test]
fn regression_error_display_non_vide() {
    use audio_capture::AudioCaptureError;
    let variants = [
        AudioCaptureError::DeviceNotFound,
        AudioCaptureError::FormatUnsupported,
        AudioCaptureError::UnitCreationFailed(-1),
        AudioCaptureError::UnitStartFailed(-2),
        AudioCaptureError::UnitStopFailed(-3),
        AudioCaptureError::PropertySetFailed(-4),
        AudioCaptureError::RingBufferFull,
    ];
    for e in &variants {
        assert!(!e.to_string().is_empty(), "Display vide pour {e:?}");
    }
}

/// Ring buffer FIFO sur 1000 samples.
#[test]
fn regression_ring_buffer_fifo_1000() {
    use audio_capture::audio_capture::ring_buffer::{
        drain_available, push_sample, AudioRingBuffer,
    };
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;

    let rb = AudioRingBuffer::new(2048);
    let prod = rb.producer_handle();
    let cons = rb.consumer_handle();
    let dropped = Arc::new(AtomicUsize::new(0));

    let input: Vec<f32> = (0..1000).map(|i| i as f32 / 1000.0).collect();
    for &s in &input {
        push_sample(&prod, &dropped, s);
    }
    let output = drain_available(&cons);
    assert_eq!(output, input, "FIFO non respecté sur 1000 samples");
}

/// Ring buffer overflow → dropped_samples > 0.
#[test]
fn regression_ring_buffer_overflow_dropped() {
    use audio_capture::audio_capture::ring_buffer::{push_sample, AudioRingBuffer};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    let cap = 8;
    let rb = AudioRingBuffer::new(cap);
    let prod = rb.producer_handle();
    let dropped = Arc::new(AtomicUsize::new(0));

    for i in 0..(cap * 2) {
        push_sample(&prod, &dropped, i as f32);
    }
    assert!(
        dropped.load(Ordering::Relaxed) > 0,
        "Aucun sample perdu malgré overflow"
    );
}

/// Ring buffer thread-safety : producteur + consommateur simultanés.
#[test]
fn regression_ring_buffer_thread_safety() {
    use audio_capture::audio_capture::ring_buffer::{
        drain_available, push_sample, AudioRingBuffer,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;

    let rb = AudioRingBuffer::new(65_536);
    let prod = rb.producer_handle();
    let cons = rb.consumer_handle();
    let dropped = Arc::new(AtomicUsize::new(0));
    let dropped2 = Arc::clone(&dropped);

    let producer = thread::spawn(move || {
        for i in 0..10_000u32 {
            push_sample(&prod, &dropped2, i as f32);
        }
    });

    let consumer = thread::spawn(move || {
        let mut total = 0usize;
        // On draine en boucle jusqu'à ce que le producteur soit terminé + extra.
        for _ in 0..200 {
            total += drain_available(&cons).len();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        total
    });

    producer.join().expect("producteur paniqué");
    let received = consumer.join().expect("consommateur paniqué");
    let lost = dropped.load(Ordering::Relaxed);
    // Tous les samples ont été soit reçus, soit perdus.
    assert_eq!(
        received + lost,
        10_000,
        "received={received} lost={lost} — total incohérent"
    );
}

/// Tailles des structs FFI correctes.
#[test]
fn regression_taille_structs_ffi() {
    use audio_capture::audio_capture::ffi::{
        AudioComponentDescription, AudioStreamBasicDescription,
    };
    assert_eq!(std::mem::size_of::<AudioComponentDescription>(), 20);
    assert_eq!(std::mem::size_of::<AudioStreamBasicDescription>(), 40);
}

// ---------------------------------------------------------------------------
// P7.3 — Tests de régression mock (feature mock_audio, sans microphone)
// ---------------------------------------------------------------------------

/// Cycle complet mock new→start→drain→stop avec signal sinusoïdal.
#[cfg(feature = "mock_audio")]
#[test]
fn regression_mock_cycle_complet() {
    use audio_capture::audio_capture::{
        consumer::AudioConsumer,
        mock::{filled_mock_queue, generate_sine, MOCK_FREQ_HZ, MOCK_SAMPLE_RATE},
    };
    use std::sync::mpsc;
    use std::time::Duration;

    let n = 1600; // 100 ms à 16 kHz
    let queue = filled_mock_queue(n);
    let (tx, rx) = mpsc::channel::<Vec<f32>>();

    let mut consumer = AudioConsumer::new(queue, 10);
    consumer.start(tx).expect("consumer start failed");

    std::thread::sleep(Duration::from_millis(50));
    consumer.stop();

    let mut received: Vec<f32> = Vec::new();
    while let Ok(batch) = rx.try_recv() {
        received.extend(batch);
    }

    assert!(
        !received.is_empty(),
        "Aucun sample reçu depuis le mock queue"
    );

    // Vérifier que les valeurs sont dans [-1.0, 1.0].
    for &s in &received {
        assert!((-1.0..=1.0).contains(&s), "Sample mock hors plage : {s}");
    }

    // Vérifier la cohérence avec le signal sinusoïdal attendu.
    let expected = generate_sine(MOCK_FREQ_HZ, MOCK_SAMPLE_RATE, received.len());
    for (i, (&got, &exp)) in received.iter().zip(expected.iter()).enumerate() {
        assert!(
            (got - exp).abs() < 1e-5,
            "Divergence à l'index {i} : got={got}, exp={exp}"
        );
    }
}

/// Start sans stop → Drop : zéro resource leak (thread zombie interdit).
#[cfg(feature = "mock_audio")]
#[test]
fn regression_mock_drop_sans_stop() {
    use audio_capture::audio_capture::{consumer::AudioConsumer, mock::filled_mock_queue};
    use std::sync::mpsc;

    let queue = filled_mock_queue(1600);
    let (tx, _rx) = mpsc::channel::<Vec<f32>>();
    let mut consumer = AudioConsumer::new(queue, 10);
    consumer.start(tx).expect("start failed");
    std::thread::sleep(std::time::Duration::from_millis(20));
    drop(consumer); // Drop doit joindre le thread proprement.
}

/// Deux instances mock simultanées : isolation des ring buffers.
#[cfg(feature = "mock_audio")]
#[test]
fn regression_mock_deux_instances_isolation() {
    use audio_capture::audio_capture::{
        consumer::AudioConsumer,
        mock::{filled_mock_queue, generate_sine, MOCK_FREQ_HZ, MOCK_SAMPLE_RATE},
    };
    use std::sync::mpsc;
    use std::time::Duration;

    // Instance A : sinusoïde 440 Hz (valeur par défaut MOCK_FREQ_HZ).
    let queue_a = filled_mock_queue(320);
    // Instance B : sinusoïde différente (880 Hz).
    let queue_b = {
        use crossbeam::queue::ArrayQueue;
        use std::sync::Arc;
        let q = Arc::new(ArrayQueue::new(320));
        for s in generate_sine(880.0, MOCK_SAMPLE_RATE, 320) {
            let _ = q.push(s);
        }
        q
    };

    let (tx_a, rx_a) = mpsc::channel::<Vec<f32>>();
    let (tx_b, rx_b) = mpsc::channel::<Vec<f32>>();

    let mut ca = AudioConsumer::new(queue_a, 5);
    let mut cb = AudioConsumer::new(queue_b, 5);
    ca.start(tx_a).unwrap();
    cb.start(tx_b).unwrap();

    std::thread::sleep(Duration::from_millis(50));
    ca.stop();
    cb.stop();

    let samples_a: Vec<f32> = rx_a.try_iter().flatten().collect();
    let samples_b: Vec<f32> = rx_b.try_iter().flatten().collect();

    // Les deux buffers doivent être non vides et différents.
    assert!(!samples_a.is_empty(), "Instance A vide");
    assert!(!samples_b.is_empty(), "Instance B vide");

    // La somme des carrés des différences doit être non nulle (signaux distincts).
    let diff: f32 = samples_a
        .iter()
        .zip(samples_b.iter())
        .map(|(a, b)| (a - b).powi(2))
        .sum();
    assert!(diff > 0.0, "Les deux instances semblent identiques");
}

// ---------------------------------------------------------------------------
// P7.4 — Tests d'intégration réels (microphone physique requis)
// ---------------------------------------------------------------------------

/// Device par défaut détecté → ID non nul.
#[cfg(not(feature = "mock_audio"))]
#[test]
fn integration_device_par_defaut_non_nul() {
    use audio_capture::audio_capture::device::get_default_input_device;
    let id = get_default_input_device().expect("Pas de device d'entrée");
    assert_ne!(id, 0, "ID device nul");
}

/// Format Float32 mono 16 kHz supporté par le device par défaut.
#[cfg(not(feature = "mock_audio"))]
#[test]
fn integration_format_float32_supporte() {
    use audio_capture::audio_capture::{
        config::AudioCaptureConfig,
        device::{check_format_support, get_default_input_device},
    };
    let id = get_default_input_device().expect("Pas de device");
    check_format_support(id, &AudioCaptureConfig::default())
        .expect("Format Float32 mono 16 kHz non supporté");
}

/// Capture 1 seconde → ≥ 15 000 samples reçus (±10 % de 16 000).
#[cfg(not(feature = "mock_audio"))]
#[test]
fn integration_capture_1s_min_15000_samples() {
    use audio_capture::{AudioCapture, AudioCaptureConfig};
    use std::sync::mpsc;
    use std::time::Duration;

    let (tx, rx) = mpsc::channel::<Vec<f32>>();
    let mut cap = AudioCapture::new(AudioCaptureConfig::default()).expect("new failed");
    cap.start(tx).expect("start failed");
    std::thread::sleep(Duration::from_secs(1));
    cap.stop().expect("stop failed");

    let total: usize = rx.try_iter().map(|b| b.len()).sum();
    assert!(
        total >= 15_000,
        "Seulement {total} samples reçus en 1 s (attendu ≥ 15 000)"
    );
}

/// Tous les samples sont dans [-1.0, 1.0].
#[cfg(not(feature = "mock_audio"))]
#[test]
fn integration_samples_dans_plage() {
    use audio_capture::{AudioCapture, AudioCaptureConfig};
    use std::sync::mpsc;
    use std::time::Duration;

    let (tx, rx) = mpsc::channel::<Vec<f32>>();
    let mut cap = AudioCapture::new(AudioCaptureConfig::default()).expect("new failed");
    cap.start(tx).expect("start failed");
    std::thread::sleep(Duration::from_millis(500));
    cap.stop().expect("stop failed");

    for batch in rx.try_iter() {
        for s in batch {
            assert!((-1.0..=1.0).contains(&s), "Sample hors plage : {s}");
        }
    }
}

/// Drop rate < 0,1 % sur 5 secondes de capture.
#[cfg(not(feature = "mock_audio"))]
#[test]
fn integration_drop_rate_inferieur_0_1_pct() {
    use audio_capture::{AudioCapture, AudioCaptureConfig};
    use std::sync::mpsc;
    use std::time::Duration;

    let (tx, rx) = mpsc::channel::<Vec<f32>>();
    let mut cap = AudioCapture::new(AudioCaptureConfig::default()).expect("new failed");
    cap.start(tx).expect("start failed");
    std::thread::sleep(Duration::from_secs(5));
    cap.stop().expect("stop failed");

    let received: usize = rx.try_iter().map(|b| b.len()).sum();
    let dropped = cap.dropped_samples();
    let total = received + dropped;

    let rate = if total > 0 {
        dropped as f64 / total as f64 * 100.0
    } else {
        0.0
    };
    assert!(
        rate < 0.1,
        "Drop rate trop élevé : {rate:.4} % (dropped={dropped}, received={received})"
    );
}
