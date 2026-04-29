# Word Waker

> Daemon macOS de détection de mot-clé (wake-word) en temps réel.
> Capture microphone → DSP/MFCC → inférence CoreML in-process → trigger.

## Architecture

```
Microphone (CoreAudio HAL)
        │  PCM f32, 16 kHz
        ▼
┌─────────────────┐
│  audio_capture  │  capture PCM Float32 mono 16 kHz
└────────┬────────┘
         │  Vec<f32>
         ▼
┌─────────────────┐
│    pipeline     │  pre-emphasis → framing → Hann → FFT → Mel → log → DCT
│   (pipeline_dsp)│  produit [[f32;13];98] (98 trames × 13 coeffs MFCC)
└────────┬────────┘
         │  [[f32;13];98]
         ▼
┌─────────────────┐
│  inference_ml   │  CoreML in-process via bridge Obj-C++
│                 │  coreml_load / coreml_infer / coreml_free
│                 │  MLComputeUnitsAll → CPU + GPU + ANE
└────────┬────────┘
         │  f32  (score wake-word ∈ [0.0, 1.0])
         ▼
      trigger
```

## Crates

| Crate | Description | Statut |
|---|---|---|
| `audio_capture` | Capture microphone 16 kHz via CoreAudio HAL | ✅ Terminé |
| `pipeline` | DSP : pre-emphasis, framing, FFT vDSP, banc Mel 40 filtres, DCT-II → MFCC `[[f32;13];98]` | ✅ Terminé |
| `inference_ml` | Inférence CoreML in-process via bridge Objective-C++, thread dédié crossbeam | ✅ Terminé |
| `integration_test` | Test d'intégration workspace : simule le flux `pipeline_dsp → inference_ml` | ✅ Terminé |

## Prérequis

- **macOS 14+** / Apple Silicon (`aarch64-apple-darwin`)
- **Rust 1.78+** — `rustup update stable`
- **Xcode Command Line Tools** — fournit `clang`, CoreAudio, CoreML, Accelerate
- **Microphone physique** — pour les tests réels d'`audio_capture`
- **Python 3.10+** + `coremltools` — uniquement pour regénérer le modèle mock

```bash
pip install coremltools numpy
```

## Lancer les tests

```bash
# Tous les crates (sans microphone, avec mocks)
cargo test -p audio_capture --features mock_audio
cargo test -p pipeline_dsp
cargo test -p inference_ml --features mock_model
cargo test -p integration_test

# Ou tous en une commande (nécessite microphone pour audio_capture)
cargo test
```

## Benchmarks (inference_ml)

```bash
cargo bench -p inference_ml
# Rapport HTML : target/criterion/report/index.html
```

Résultats de référence (Apple M-series) :

| Benchmark | Latence médiane |
|---|---|
| `infer/all_units` (ANE) | ~17 µs |
| `infer/cpu_only` | ~17 µs |

## Modèle mock

Le modèle mock (`WakeWordMock.mlmodelc`) est versionné dans `inference_ml/fixtures/`.
Pour le regénérer :

```bash
cd inference_ml
python scripts/generate_mock_model.py
# → fixtures/mock_model/WakeWordMock.mlmodelc
```

## Structure du workspace

```
Word-waker/
├── Cargo.toml           — workspace (members: audio_capture, pipeline, inference_ml, integration_test)
├── audio_capture/       — capture CoreAudio
│   ├── src/
│   ├── tests/
│   └── README.md
├── pipeline/            — DSP / MFCC (Accelerate.framework)
│   ├── src/
│   ├── tests/
│   └── README.md
├── inference_ml/        — inférence CoreML + bridge Objective-C++
│   ├── src/bridge/      — coreml_bridge.mm
│   ├── benches/         — criterion benchmarks
│   ├── fixtures/        — WakeWordMock.mlmodelc
│   ├── tests/
│   └── README.md
├── integration_test/    — test d'intégration workspace
│   └── tests/
└── docs/
```

## Licence

<!-- À compléter -->
