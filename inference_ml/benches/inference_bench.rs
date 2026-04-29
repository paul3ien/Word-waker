use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use inference_ml::{CoreMLModel, InferenceConfig};

fn mock_config() -> InferenceConfig {
    InferenceConfig {
        model_path: format!(
            "{}/fixtures/mock_model/WakeWordMock.mlmodelc",
            env!("CARGO_MANIFEST_DIR")
        ),
        ..Default::default()
    }
}

/// P8.2 — Latence d'une inférence (MLComputeUnitsAll — ANE prioritaire).
/// Cible : médiane < 5 ms sur 100 appels.
fn bench_infer_all_units(c: &mut Criterion) {
    let model = CoreMLModel::load(&mock_config()).expect("load échoué");
    let mfcc = [[0.0f32; 13]; 98];

    c.bench_function("infer/all_units", |b| {
        b.iter(|| {
            model.infer(&mfcc).expect("infer échoué");
        });
    });
}

/// P8.2 — Throughput : nombre d'inférences par seconde.
/// Cible : >= 20 inférences/s (1 toutes les 50 ms max).
fn bench_infer_throughput(c: &mut Criterion) {
    let model = CoreMLModel::load(&mock_config()).expect("load échoué");
    let mfcc = [[0.0f32; 13]; 98];

    let mut group = c.benchmark_group("throughput");
    // Mesure le débit sur différentes tailles de batch séquentiel.
    for n in [1usize, 10, 20, 50] {
        group.bench_with_input(BenchmarkId::new("sequential", n), &n, |b, &n| {
            b.iter(|| {
                for _ in 0..n {
                    model.infer(&mfcc).expect("infer échoué");
                }
            });
        });
    }
    group.finish();
}

/// P8.2 — Latence CPU-only (`MLComputeUnitsCPUOnly`) pour mesurer le gain ANE.
/// Charge le modèle sans le Neural Engine, puis mesure la médiane des inférences.
fn bench_infer_cpu_only(c: &mut Criterion) {
    let model = CoreMLModel::load_cpu_only(&mock_config()).expect("load_cpu_only échoué");
    let mfcc = [[0.0f32; 13]; 98];

    c.bench_function("infer/cpu_only", |b| {
        b.iter(|| {
            model.infer(&mfcc).expect("infer échoué");
        });
    });
}

criterion_group!(benches, bench_infer_all_units, bench_infer_cpu_only, bench_infer_throughput);
criterion_main!(benches);
