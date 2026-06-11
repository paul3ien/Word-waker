# Word Waker

> Daemon macOS de détection de mot-clé (wake-word) en temps réel.
> Capture microphone → DSP/MFCC → inférence CoreML in-process → trigger.

## Architecture

```
┌──────────────────────────────────────────────────────┐
│                      daemon                          │
│                                                      │
│  Microphone (CoreAudio HAL)                          │
│          │  PCM Vec<f32>, 16 kHz                     │
│          ▼                                           │
│  ┌─────────────────┐                                 │
│  │  audio_capture  │  capture PCM Float32 mono 16 kHz│
│  └────────┬────────┘                                 │
│           │  Sender<Vec<f32>>  (channel bounded 8)   │
│           ▼                                          │
│  ┌─────────────────┐                                 │
│  │    pipeline     │  pre-emphasis → framing → Hann  │
│  │   (pipeline_dsp)│  → FFT → Mel → log → DCT        │
│  │                 │  produit [[f32;13];98]           │
│  └────────┬────────┘                                 │
│           │  Sender<[[f32;13];98]>  (bounded 8)      │
│           ▼                                          │
│  ┌─────────────────┐                                 │
│  │  inference_ml   │  CoreML in-process (Obj-C++)     │
│  │                 │  MLComputeUnitsAll (CPU+GPU+ANE) │
│  └────────┬────────┘                                 │
│           │  Sender<f32>  score ∈ [0.0, 1.0]         │
│           ▼                                          │
│  ┌─────────────────┐                                 │
│  │    trigger      │  vote glissant 3/5, cooldown 2 s │
│  └────────┬────────┘                                 │
└───────────┼──────────────────────────────────────────┘
            │  "WAKEWORD_DETECTED\n"
            ▼
  Unix Domain Socket  →  Application cliente
  /tmp/wakeword_daemon.sock
```

## Crates

| Crate | Description | Statut |
|---|---|---|
| `audio_capture` | Capture microphone 16 kHz via CoreAudio HAL | ✅ Terminé |
| `pipeline` | DSP : pre-emphasis, framing, FFT vDSP, banc Mel 40 filtres, DCT-II → MFCC `[[f32;13];98]` | ✅ Terminé |
| `inference_ml` | Inférence CoreML in-process via bridge Objective-C++, thread dédié crossbeam | ✅ Terminé |
| `trigger` | Vote glissant anti-faux-positifs, cooldown, notification via Unix Domain Socket | ✅ Terminé |
| `integration_test` | Tests d'intégration workspace : `pipeline_dsp → inference_ml → trigger → socket` | ✅ Terminé |
| `daemon` | Binaire exécutable — câble les 4 crates en pipeline, gère SIGINT/SIGTERM, config via env | 🚧 En cours |
| `ui` | Application macOS native (NSStatusBar) — se connecte au daemon via socket IPC, affiche l'état en temps réel | 📋 Planifié |

## Prérequis

- **macOS 14+** / Apple Silicon (`aarch64-apple-darwin`)
- **Rust 1.78+** — `rustup update stable`
- **Xcode Command Line Tools** — fournit `clang`, CoreAudio, CoreML, Accelerate
- **Microphone physique** — pour les tests réels d'`audio_capture`
- **Python 3.10+** + `coremltools` — uniquement pour regénérer le modèle mock

```bash
pip install coremltools numpy
```

## Lancer le daemon

### En terminal (développement)

```bash
# Build release
cargo build --release -p daemon

# Lancer avec le modèle réel (chemin relatif depuis la racine du workspace)
WAKEWORD_MODEL_PATH=inference_ml/fixtures/real_model/WakeWord.mlmodelc \
  cargo run --release -p daemon

# Lancer avec le modèle mock (test sans modèle entraîné)
WAKEWORD_MODEL_PATH=inference_ml/fixtures/mock_model/WakeWordMock.mlmodelc \
  cargo run --release -p daemon

# Écouter les détections dans un autre terminal
nc -U /tmp/wakeword_daemon.sock

# Arrêt propre
# Ctrl+C  →  logs d'arrêt  →  exit 0
```

### En arrière-plan sans terminal (LaunchAgent macOS)

Copier le binaire et installer un LaunchAgent pour que le daemon démarre
automatiquement au login, **sans aucune fenêtre de terminal** :

```bash
# 1. Build release
cargo build --release -p daemon

# 2. Copier le binaire dans /usr/local/bin
sudo cp target/release/word-waker /usr/local/bin/word-waker

# 3. Installer le LaunchAgent (créer le fichier plist)
mkdir -p ~/Library/LaunchAgents
cat > ~/Library/LaunchAgents/com.wordwaker.daemon.plist << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>             <string>com.wordwaker.daemon</string>
  <key>ProgramArguments</key>  <array><string>/usr/local/bin/word-waker</string></array>
  <key>EnvironmentVariables</key>
  <dict>
    <key>WAKEWORD_MODEL_PATH</key>
    <string>/Users/apple/Documents/Code/Word-waker/inference_ml/fixtures/real_model/WakeWord.mlmodelc</string>
  </dict>
  <key>RunAtLoad</key>         <true/>
  <key>KeepAlive</key>         <true/>
  <key>StandardOutPath</key>   <string>/tmp/word-waker.log</string>
  <key>StandardErrorPath</key> <string>/tmp/word-waker.log</string>
</dict>
</plist>
EOF

# 4. Charger le daemon (démarre immédiatement + au prochain login)
launchctl load ~/Library/LaunchAgents/com.wordwaker.daemon.plist

# Vérifier qu'il tourne
launchctl list | grep wordwaker

# Voir les logs
tail -f /tmp/word-waker.log

# Arrêter le daemon
launchctl unload ~/Library/LaunchAgents/com.wordwaker.daemon.plist
```

Le daemon tourne silencieusement en arrière-plan, se relance automatiquement
en cas de crash (`KeepAlive`), et démarre au login sans interaction.

Variables d'environnement disponibles :

| Variable | Défaut | Description |
|---|---|---|
| `WAKEWORD_MODEL_PATH` | *(obligatoire)* | Chemin absolu vers le `.mlmodelc` |
| `WAKEWORD_SOCKET_PATH` | `/tmp/wakeword_daemon.sock` | Chemin du socket IPC |
| `WAKEWORD_THRESHOLD` | `0.80` | Seuil de score individuel |
| `WAKEWORD_COOLDOWN_MS` | `2000` | Délai minimal entre deux détections (ms) |

## Lancer les tests

```bash
# Tous les crates (sans microphone, avec mocks)
cargo test -p audio_capture --features mock_audio
cargo test -p pipeline_dsp
cargo test -p inference_ml --features mock_model
cargo test -p trigger
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
├── Cargo.toml           — workspace (members: audio_capture, pipeline, inference_ml, trigger, integration_test, daemon)
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
├── trigger/             — vote glissant + IPC Unix Domain Socket
│   ├── src/trigger/     — engine, ipc, runner, config, error
│   ├── tests/           — suite de régression & performance
│   ├── examples/        — standalone_trigger
│   └── README.md
├── integration_test/    — tests d'intégration workspace
│   └── tests/
├── daemon/              — binaire exécutable final
│   ├── src/             — main.rs, config.rs, pipeline.rs
│   ├── backlog.md
│   └── stack.md
└── docs/
```

## Licence

<!-- À compléter -->
