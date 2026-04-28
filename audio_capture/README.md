# audio_capture

Capture microphone PCM Float32 mono 16 kHz via CoreAudio (macOS).

## Prérequis

- macOS 12+ (Apple Silicon ou Intel)
- Rust 1.78+
- Microphone physique accessible (pour les tests réels)

## Lancer les tests

```bash
# Tests unitaires + intégration réels (microphone requis)
cargo test -p audio_capture

# Tests mock uniquement (pas de microphone)
cargo test -p audio_capture --features mock_audio
```

## Utiliser depuis un autre crate du workspace

Dans `Cargo.toml` du crate consommateur :

```toml
[dependencies]
audio_capture = { path = "../audio_capture" }
```

Exemple minimal :

```rust
use audio_capture::{AudioCapture, AudioCaptureConfig};
use std::sync::mpsc;

fn main() {
    let (tx, rx) = mpsc::channel();
    let mut cap = AudioCapture::new(AudioCaptureConfig::default()).unwrap();
    cap.start(tx).unwrap();

    // Lire les batches depuis le thread principal
    for batch in rx.iter().take(10) {
        println!("{} samples reçus", batch.len());
    }

    cap.stop().unwrap();
}
```

## Architecture

```
AudioCapture (façade publique)
├── AudioUnitCapture     — HAL CoreAudio (AudioDeviceCreateIOProcID)
├── AudioRingBuffer      — ArrayQueue<f32> lock-free (crossbeam)
└── AudioConsumer        — thread de polling → Sender<Vec<f32>>
```

## Feature flags

| Flag         | Effet                                                  |
|--------------|--------------------------------------------------------|
| `mock_audio` | Active `audio_capture::mock` (signal sinusoïdal synthétique pour tests sans microphone) |
| `standalone` | Active l'example `standalone_capture` (capture 3 s)   |
