# pipeline_dsp

Crate de traitement du signal DSP pour la reconnaissance vocale.

Prend des samples PCM Float32 16 kHz en entrée et produit des matrices MFCC `[[f32;13];98]` prêtes pour l'inférence ML.

## Chaîne de traitement

```
samples PCM (f32, 16 kHz)
  → PreEmphasis (α=0.97)
  → Framing (400 samples / 160 hop)
  → Fenêtre de Hann
  → FFT 512 points (vDSP_fft_zrip)
  → Banc de filtres Mel (40 filtres, 20–8000 Hz)
  → log
  → DCT-II (vDSP, zéro-padding vers 48)
  → 13 coefficients MFCC par trame
  → Accumulateur 98 trames → matrice [[f32;13];98]
```

## Prérequis

- **macOS** avec **Accelerate.framework** (fourni par Xcode Command Line Tools)
- **Rust 1.78+** (`rustup update stable`)
- **Python 3.10+** avec `numpy`, `scipy` (optionnel — pour les scripts de référence)

```bash
pip install numpy scipy
```

## Lancer les tests

```bash
# Depuis la racine du workspace
cargo test -p pipeline_dsp

# Avec la feature mock_input
cargo test -p pipeline_dsp --features mock_input

# Tests avec doc-tests inclus
cargo test -p pipeline_dsp --doc
```

## Générer la fixture de référence

La fixture `fixtures/reference_mfcc.json` est générée directement depuis Rust
(le pipeline vDSP est la source de vérité) :

```bash
cargo run --example generate_fixture --features standalone -p pipeline_dsp
```

La fixture contient 98 trames × 13 coefficients MFCC calculés sur un signal sinus
440 Hz de 2 secondes.

## Exemple standalone

```bash
cargo run --example standalone_dsp --features standalone -p pipeline_dsp
```

Génère 3 secondes de sinus 440 Hz, les traite via `DspPipeline`, et affiche la
première matrice MFCC.

## Benchmarks

```bash
cargo bench -p pipeline_dsp
```

Résultats cibles sur Apple Silicon :
- `bench_frame_processor` : < 0.5 ms par trame
- `bench_pipeline_1s` : < 5 ms par seconde d'audio

## Documentation

```bash
cargo doc --no-deps -p pipeline_dsp --open
```

## Intégration avec `audio_capture`

`DspRunner` expose une API basée sur des `crossbeam_channel` :

```rust
use crossbeam_channel::unbounded;
use pipeline_dsp::pipeline_dsp::{config::DspConfig, runner::DspRunner};

let (audio_tx, audio_rx) = unbounded::<Vec<f32>>();
let (mfcc_tx, mfcc_rx) = unbounded::<[[f32; 13]; 98]>();

// Démarrer le thread DSP
let runner = DspRunner::start(DspConfig::default(), audio_rx, mfcc_tx).unwrap();

// audio_capture envoie des batches via audio_tx
// mfcc_rx reçoit les matrices [[f32;13];98]

runner.stop();
```

`audio_capture` peut envoyer des batches de n'importe quelle taille —
`DspPipeline` accumule les samples et émet des matrices complètes dès que
98 trames sont disponibles.

## Structure du crate

```
src/
  lib.rs                        # Entrée publique
  pipeline_dsp/
    mod.rs                      # Re-exports
    error.rs                    # DspError
    config.rs                   # DspConfig (paramètres DSP)
    ffi.rs                      # Bindings Accelerate (vDSP, BLAS, DCT)
    preemphasis.rs              # Filtre IIR du premier ordre
    framing.rs                  # Découpage en trames avec overlap
    windowing.rs                # Fenêtre de Hann
    fft.rs                      # FFT réelle → magnitudes
    mel_filterbank.rs           # Banc de filtres Mel triangulaires
    mfcc.rs                     # log + DCT-II → MFCC[13]
    processor.rs                # FrameProcessor + MfccAccumulator
    pipeline.rs                 # DspPipeline (façade haut niveau)
    runner.rs                   # DspRunner (thread + channels)
examples/
  standalone_dsp.rs             # Démo complète (--features standalone)
  generate_fixture.rs           # Génère fixtures/reference_mfcc.json
fixtures/
  reference_mfcc.json           # 98×13 MFCC de référence (Rust)
benches/
  dsp_bench.rs                  # Benchmarks Criterion
```
