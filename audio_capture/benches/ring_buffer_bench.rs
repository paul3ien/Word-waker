use audio_capture::audio_capture::ring_buffer::{drain_available, push_sample, AudioRingBuffer};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Benchmark 1 : Throughput push + pop — 1 million d'opérations
// Critère : > 100 000 ops/s (soit < 10 µs/op)
// ---------------------------------------------------------------------------

fn bench_throughput(c: &mut Criterion) {
    let ring = AudioRingBuffer::new(65_536);
    let producer = ring.producer_handle();
    let consumer = ring.consumer_handle();
    let dropped = Arc::new(AtomicUsize::new(0));

    c.bench_function("ring_buffer_throughput_1M", |b| {
        b.iter(|| {
            // Push 1000 samples puis drain — répété N fois par criterion
            for i in 0..1000u32 {
                push_sample(&producer, &dropped, black_box(i as f32 / 1000.0));
            }
            let batch = drain_available(&consumer);
            black_box(batch);
        })
    });
}

// ---------------------------------------------------------------------------
// Benchmark 2 : Latence consommateur — temps entre push et réception
// Critère : médiane < 15 ms
// ---------------------------------------------------------------------------

fn bench_latency(c: &mut Criterion) {
    use std::sync::mpsc;
    use std::thread;

    c.bench_function("consumer_latency_push_to_receive", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;

            for _ in 0..iters {
                let ring = AudioRingBuffer::new(1024);
                let producer = ring.producer_handle();
                let consumer = ring.consumer_handle();
                let dropped = Arc::new(AtomicUsize::new(0));

                let (tx, rx) = mpsc::sync_channel::<Vec<f32>>(1);

                // Thread consommateur : poll toutes les 1 ms
                let handle = thread::spawn(move || {
                    loop {
                        let batch = drain_available(&consumer);
                        if !batch.is_empty() {
                            let _ = tx.send(batch);
                            return;
                        }
                        thread::sleep(Duration::from_millis(1));
                    }
                });

                // Mesure : temps entre push et réception
                let t0 = Instant::now();
                push_sample(&producer, &dropped, black_box(0.5_f32));
                let _ = rx.recv();
                total += t0.elapsed();

                handle.join().unwrap();
            }

            total
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(5))
        .sample_size(50);
    targets = bench_throughput, bench_latency
}
criterion_main!(benches);
