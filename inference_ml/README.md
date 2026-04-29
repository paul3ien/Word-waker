# inference_ml

Module d'inférence wake-word basé sur **Core ML** (Apple Neural Engine).

Fournit un thread d'inférence dédié qui reçoit des matrices MFCC via un canal
`crossbeam_channel` et émet des scores de présence du mot-clé.

---

## Prérequis

- macOS 14+ / aarch64 (Apple Silicon)
- Xcode Command Line Tools (fournit `clang`, CoreML.framework)
- Rust stable ≥ 1.78

---

## Générer le modèle mock

Le modèle mock est un réseau trivial généré par `coremltools` (score constant 0.5).
Il est nécessaire pour les tests d'intégration et les benchmarks.

```bash
# Depuis la racine du workspace
cd inference_ml
pip install coremltools numpy      # ou uv pip install …
python scripts/generate_mock_model.py
# → fixtures/mock_model/WakeWordMock.mlmodelc
```

Le modèle est déjà versionné dans `fixtures/` — cette commande n'est
nécessaire que si tu veux le régénérer.

---

## Lancer les tests

```bash
# Tests complets avec le modèle mock réel (fixtures/)
cargo test -p inference_ml

# Tests avec le stub en mémoire (sans fixtures/)
cargo test -p inference_ml --features mock_model

# Tests sous AddressSanitizer (nightly requis)
RUSTFLAGS="-Z sanitizer=address" \
  cargo +nightly test -p inference_ml --target aarch64-apple-darwin
```

---

## Benchmarks

```bash
# Latence ANE (~17 µs) et CPU-only (~17 µs sur modèle mock)
cargo bench -p inference_ml

# Rapport HTML dans target/criterion/
open target/criterion/report/index.html
```

---

## Profiling Instruments

```bash
# Compiler et signer pour Instruments
cargo build --example instruments_loop -p inference_ml
codesign -s - --entitlements debug.entitlements --force \
  target/debug/examples/instruments_loop

# Lancer (30 secondes de boucle)
./target/debug/examples/instruments_loop
```

Dans Xcode Instruments : **File → New → Attach to Process → `instruments_loop`**

Métriques validées :
- Heap persistent : **3.74 MiB** (< 5 Mo ✅)
- ANE : **124 527 batches** dispatché ✅
- CPU thread inférence : quasi nul (bloqué sur `recv()`) ✅

---

## Brancher sur `pipeline_dsp`

`InferenceEngine` consomme un `Receiver<[[f32; 13]; 98]>` et produit un
`Sender<f32>`. Le crate `pipeline` (ou `pipeline_dsp`) n'a qu'à créer les
deux canaux et les passer à `engine.start()` :

```rust
use crossbeam_channel::bounded;
use inference_ml::{InferenceConfig, InferenceEngine};

let config = InferenceConfig {
    model_path: "path/to/WakeWord.mlmodelc".into(),
    ..Default::default()
};
let mut engine = InferenceEngine::new(config)?;

let (tx_mfcc, rx_mfcc) = bounded::<[[f32; 13]; 98]>(8);
let (tx_score, rx_score) = bounded::<f32>(8);
engine.start(rx_mfcc, tx_score)?;

// pipeline_dsp envoie des matrices MFCC :
tx_mfcc.send(mfcc_matrix)?;

// Un autre thread lit les scores :
let score = rx_score.recv()?;  // ∈ [0.0, 1.0]

engine.stop()?;
```

---

## Structure du crate

```
inference_ml/
├── src/
│   ├── lib.rs          — exports publics + doc crate
│   ├── error.rs        — InferenceError (7 variantes)
│   ├── config.rs       — InferenceConfig + validate()
│   ├── ffi.rs          — déclarations FFI (coreml_load / infer / free)
│   ├── model.rs        — CoreMLModel (load / infer / Drop)
│   ├── runner.rs       — InferenceRunner (thread dédié)
│   ├── engine.rs       — InferenceEngine (façade publique)
│   └── bridge/
│       └── coreml_bridge.mm  — bridge Objective-C++
├── benches/
│   └── inference_bench.rs    — benchmarks criterion
├── examples/
│   ├── standalone_inference.rs
│   └── instruments_loop.rs
├── fixtures/
│   └── mock_model/WakeWordMock.mlmodelc
├── tests/
│   └── inference_ml_integration.rs  — 21 tests d'intégration
└── build.rs            — compilation du bridge via cc
```
