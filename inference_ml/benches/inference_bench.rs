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

/// Chemin vers le vrai modèle entraîné (copié depuis model_ww_v2/exports/).
fn real_model_path() -> String {
    format!(
        "{}/fixtures/real_model/WakeWord.mlmodelc",
        env!("CARGO_MANIFEST_DIR")
    )
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

// ─── P9.3 ─────────────────────────────────────────────────────────────────────

/// P9.3 — Latence ANE avec le **vrai modèle entraîné** (cible P50 ≤ 1 ms).
///
/// Utilise le vrai modèle si `fixtures/real_model/WakeWord.mlmodelc` est présent,
/// sinon retombe sur le mock (mention dans la sortie standard).
///
/// Collecte 200 échantillons de latence bruts et affiche P50 / P95 / P99
/// avant de lancer la mesure Criterion (qui fournit ses propres statistiques).
fn bench_ane_latency(c: &mut Criterion) {
    use std::time::Instant;

    let rmp = real_model_path();
    let (model, label) = if std::path::Path::new(&rmp).exists() {
        let cfg = InferenceConfig { model_path: rmp.clone(), ..Default::default() };
        let m = CoreMLModel::load(&cfg).expect("chargement vrai modèle échoué");
        (m, "real_model/ANE")
    } else {
        eprintln!(
            "\n[bench_ane_latency] Vrai modèle absent : {rmp}\n\
             Copier d'abord :\n\
             cp -r model_ww_v2/exports/WakeWord.mlmodelc \\\n\
             inference_ml/fixtures/real_model/\n\
             → Utilisation du mock (latence non significative)\n"
        );
        (CoreMLModel::load(&mock_config()).expect("mock load"), "mock_model")
    };

    let mfcc = [[0.0f32; 13]; 98];

    // ── Préchauffage (évite le JIT CoreML au premier appel) ──────────────────
    for _ in 0..10 {
        model.infer(&mfcc).ok();
    }

    // ── Collecte manuelle de 200 échantillons pour le rapport P50/P95/P99 ────
    const N: usize = 200;
    let mut durations_us: Vec<f64> = Vec::with_capacity(N);
    for _ in 0..N {
        let t0 = Instant::now();
        model.infer(&mfcc).ok();
        durations_us.push(t0.elapsed().as_secs_f64() * 1_000_000.0); // µs
    }
    durations_us.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let p50  = durations_us[N / 2];
    let p95  = durations_us[(N as f64 * 0.95) as usize];
    let p99  = durations_us[(N as f64 * 0.99) as usize];
    let mean = durations_us.iter().sum::<f64>() / N as f64;

    let target_us = 1_000.0_f64; // 1 ms = 1000 µs
    let status = if p50 <= target_us { "✓ GO" } else { "✗ NO-GO" };

    eprintln!(
        "\n[{label}] Latence inférence sur {N} appels :\n\
         +-----------------------------------------------+\n\
         |  P50  = {:8.1} us  (cible <= 1 000 us)  {status} |\n\
         |  P95  = {:8.1} us                               |\n\
         |  P99  = {:8.1} us                               |\n\
         |  Mean = {:8.1} us                               |\n\
         +-----------------------------------------------+\n",
        p50, p95, p99, mean
    );

    // ── Mesure Criterion (statistics officielles) ─────────────────────────────
    let mut group = c.benchmark_group("ane_latency");
    group.bench_function(label, |b| {
        b.iter(|| {
            model.infer(criterion::black_box(&mfcc)).ok();
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_infer_all_units,
    bench_infer_cpu_only,
    bench_infer_throughput,
    bench_ane_latency,
);
criterion_main!(benches);
