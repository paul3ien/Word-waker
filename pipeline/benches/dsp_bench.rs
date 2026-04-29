// Benchmarks du crate pipeline_dsp — P11.5

use std::f32::consts::PI;

use criterion::{criterion_group, criterion_main, Criterion};
use pipeline_dsp::pipeline_dsp::config::DspConfig;
use pipeline_dsp::pipeline_dsp::pipeline::DspPipeline;
use pipeline_dsp::pipeline_dsp::processor::FrameProcessor;

/// Benchmark P11.5-A : latence de `FrameProcessor::process_frame` sur 1 trame.
/// Objectif : < 0.5 ms
fn bench_frame_processor(c: &mut Criterion) {
    let cfg = DspConfig::default();
    let mut processor = FrameProcessor::new(&cfg).unwrap();
    let raw_frame: Vec<f32> = (0..cfg.frame_size)
        .map(|i| (2.0 * PI * 440.0 * i as f32 / cfg.sample_rate as f32).sin())
        .collect();

    c.bench_function("FrameProcessor::process_frame", |b| {
        b.iter(|| {
            let mut frame = raw_frame.clone();
            criterion::black_box(processor.process_frame(&mut frame))
        });
    });
}

/// Benchmark P11.5-B : throughput `DspPipeline::process_batch` pour 1 seconde d'audio.
/// Objectif : < 5 ms pour 1 s d'audio (frame_size=400, hop=160, n_fft=512)
fn bench_pipeline_1s(c: &mut Criterion) {
    let cfg = DspConfig::default();
    let n_samples = cfg.sample_rate as usize; // 1 seconde
    let samples: Vec<f32> = (0..n_samples)
        .map(|i| (2.0 * PI * 440.0 * i as f32 / cfg.sample_rate as f32).sin())
        .collect();

    c.bench_function("DspPipeline::process_batch (1s)", |b| {
        b.iter(|| {
            let mut pipeline = DspPipeline::new(cfg.clone()).unwrap();
            criterion::black_box(pipeline.process_batch(&samples))
        });
    });
}

criterion_group!(benches, bench_frame_processor, bench_pipeline_1s);
criterion_main!(benches);

