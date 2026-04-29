# Word Waker

> Application macOS de transcription vocale en temps réel.

## Vue d'ensemble

<!-- À compléter -->

## Architecture

```
Word-waker/
├── audio_capture/   — Capture microphone PCM Float32 via CoreAudio
├── pipeline/        — Pipeline de traitement audio (à venir)
└── docs/            — Documentation du projet
```

## Crates

| Crate | Description | Statut |
|---|---|---|
| `audio_capture` | Capture microphone 16 kHz via CoreAudio HAL | ✅ Terminé |
| `pipeline` | Pipeline traitement / transcription | 🚧 En cours |

## Prérequis

- macOS 12+ (Apple Silicon ou Intel)
- Rust 1.78+
- Microphone physique

## Lancer les tests

```bash
# Tous les crates
cargo test

# Avec mocks (sans microphone)
cargo test --features mock_audio
```

## Licence

<!-- À compléter -->
