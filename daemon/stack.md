# Stack Technique — Crate `daemon`

> Ce fichier documente les technologies, dépendances et contraintes propres au binaire daemon Word Waker.
> Le daemon orchestre les 4 crates bibliothèques (`audio_capture`, `pipeline_dsp`, `inference_ml`, `trigger`)
> via des channels `crossbeam-channel`, de la capture microphone jusqu'à la notification IPC.
> Il constitue le seul point d'entrée exécutable du workspace.

---

## Architecture du daemon

```
Microphone (CoreAudio HAL)
        │  PCM Vec<f32> — 16 kHz Float32 mono
        ▼
AudioCapture::start(tx_pcm)
        │  Sender<Vec<f32>>  ──────────────────────────────────────────┐
        ▼                                                               │
DspRunner::start(rx_pcm, tx_mfcc)                                      │
        │  Sender<[[f32;13];98]>  ─────────────────────────────────┐   │
        ▼                                                           │   │
InferenceEngine::start(rx_mfcc, tx_score)                          │   │
        │  Sender<f32>  ──────────────────────────────────────┐    │   │
        ▼                                                      │    │   │
TriggerModule::start(rx_score)                                 │    │   │
        │  "WAKEWORD_DETECTED\n"                               │    │   │
        ▼                                                      │    │   │
Unix Domain Socket (/tmp/wakeword_daemon.sock)                 │    │   │
                                                               │    │   │
Channels crossbeam-channel (bounded(8)) ──────────────────────┘────┘───┘
```

> **Zéro thread actif en idle.** Chaque étage bloque sur `recv()`.
> L'arrêt se propage par fermeture des `Sender` : `capture.stop()` → `tx_pcm` fermé → DSP quitte → `tx_mfcc` fermé → etc.

---

## Légende des niveaux d'importance

| Niveau | Signification |
|--------|---------------|
| 🔴 CRITIQUE | Bloquant — le daemon ne peut pas fonctionner sans |
| 🟠 OBLIGATOIRE | Requis pour respecter les contraintes de performance et de sécurité |
| 🟡 IMPORTANT | Fortement recommandé, contournement possible à court terme uniquement |
| 🟢 OPTIONNEL | Amélioration ou outillage, non bloquant |

---

## 1. Langage & Toolchain

| Technologie | Version min | Niveau | Justification |
|---|---|---|---|
| Rust | 1.78+ | 🔴 CRITIQUE | Langage principal |
| Cargo | (bundled) | 🔴 CRITIQUE | Build system |
| Target `aarch64-apple-darwin` | macOS 14+ | 🔴 CRITIQUE | Cible exclusive — CoreAudio, CoreML, Unix Domain Socket POSIX |
| Edition Rust 2021 | — | 🟠 OBLIGATOIRE | Resolver v2, compatible workspace |
| `rustfmt` | (stable) | 🟡 IMPORTANT | Formatage |
| `clippy` | (stable) | 🟡 IMPORTANT | Linting |

---

## 2. Dépendances Cargo

### 2.1 Crates workspace internes

| Crate | Chemin | Niveau | Rôle dans le daemon |
|---|---|---|---|
| `audio_capture` | `{ path = "../audio_capture" }` | 🔴 CRITIQUE | Fournit `AudioCapture` + `AudioCaptureConfig` — capture PCM |
| `pipeline_dsp` | `{ path = "../pipeline" }` | 🔴 CRITIQUE | Fournit `DspRunner` + `DspConfig` — DSP MFCC |
| `inference_ml` | `{ path = "../inference_ml" }` | 🔴 CRITIQUE | Fournit `InferenceEngine` + `InferenceConfig` — CoreML |
| `trigger` | `{ path = "../trigger" }` | 🔴 CRITIQUE | Fournit `TriggerModule` + `TriggerConfig` — vote + IPC |

### 2.2 Crates externes

| Crate | Version | Niveau | Rôle |
|---|---|---|---|
| `crossbeam-channel` | 0.5+ | 🔴 CRITIQUE | Channels inter-étages (`bounded(8)`) |
| `anyhow` | 1.0+ | 🔴 CRITIQUE | Propagation d'erreurs dans `main` avec contexte |
| `ctrlc` | 3.0+ | 🟠 OBLIGATOIRE | Handler SIGINT/SIGTERM pour arrêt propre |
| `tracing` | 0.1+ | 🟡 IMPORTANT | Logs structurés : démarrage, détections, arrêt |
| `tracing-subscriber` | 0.3+ | 🟡 IMPORTANT | Backend de logs console |

---

## 3. Interfaces des crates consommées

### 3.1 `audio_capture`

| Symbole | Signature | Notes |
|---|---|---|
| `AudioCapture::new` | `(AudioCaptureConfig) → Result<Self, AudioCaptureError>` | Détecte le device par défaut |
| `AudioCapture::start` | `(&mut self, Sender<Vec<f32>>) → Result<(), AudioCaptureError>` | Démarre la capture et le thread consommateur |
| `AudioCapture::stop` | `(&mut self) → Result<(), AudioCaptureError>` | Arrêt propre — ferme `tx_pcm` implicitement |
| `AudioCaptureConfig::default` | `() → Self` | 16 kHz, Float32 mono, buffer 4096 frames |

### 3.2 `pipeline_dsp`

| Symbole | Signature | Notes |
|---|---|---|
| `DspRunner::start` | `(DspConfig, Receiver<Vec<f32>>, Sender<[[f32;13];98]>) → Result<Self, DspError>` | Spawne le thread DSP immédiatement |
| `DspRunner::stop` | `(&mut self)` | Signal d'arrêt + join du thread |
| `DspConfig::default` | `() → Self` | 16 kHz, frame 400, hop 160, 40 filtres Mel, 13 MFCC, 98 trames |

### 3.3 `inference_ml`

| Symbole | Signature | Notes |
|---|---|---|
| `InferenceEngine::new` | `(InferenceConfig) → Result<Self, InferenceError>` | Charge le modèle CoreML |
| `InferenceEngine::start` | `(&mut self, Receiver<[[f32;13];98]>, Sender<f32>) → Result<(), InferenceError>` | Démarre le thread d'inférence |
| `InferenceEngine::stop` | `(&mut self) → Result<(), InferenceError>` | Join du thread |
| `InferenceConfig::default` | `() → Self` | `model_path` vide — à renseigner obligatoirement |

### 3.4 `trigger`

| Symbole | Signature | Notes |
|---|---|---|
| `TriggerModule::new` | `(TriggerConfig) → Result<Self, TriggerError>` | Valide la config, crée le runner |
| `TriggerModule::start` | `(&mut self, Receiver<f32>) → Result<(), TriggerError>` | Démarre le thread trigger |
| `TriggerModule::stop` | `(&mut self) → Result<(), TriggerError>` | Join du thread |
| `TriggerConfig::default` | `() → Self` | threshold=0.80, votes=3/5, cooldown=2000ms, socket=/tmp/wakeword_daemon.sock |

---

## 4. Topologie des channels

| Channel | Type | Capacité | Producteur | Consommateur |
|---|---|---|---|---|
| `(tx_pcm, rx_pcm)` | `crossbeam_channel::bounded<Vec<f32>>(8)` | 8 batches | `AudioCapture` | `DspRunner` |
| `(tx_mfcc, rx_mfcc)` | `crossbeam_channel::bounded<[[f32;13];98]>(8)` | 8 matrices | `DspRunner` | `InferenceEngine` |
| `(tx_score, rx_score)` | `crossbeam_channel::bounded<f32>(8)` | 8 scores | `InferenceEngine` | `TriggerModule` |

> Capacité 8 : absorbe les micro-variations de latence sans bloquer le thread producteur. Une capacité plus grande augmenterait la latence bout-en-bout.

---

## 5. Ordre de démarrage et d'arrêt

### Démarrage (du consommateur vers le producteur)

```
1. TriggerModule::start(rx_score)       ← prêt à consommer des scores
2. InferenceEngine::start(rx_mfcc, tx_score)  ← prêt à consommer des matrices
3. DspRunner::start(rx_pcm, tx_mfcc)    ← prêt à consommer du PCM (spawne thread)
4. AudioCapture::start(tx_pcm)          ← commence à capturer du son
```

> Raison : chaque étage doit être prêt à consommer avant que le précédent commence à produire. Évite le blocage des channels bornés.

### Arrêt (du producteur vers le consommateur)

```
1. AudioCapture::stop()    → tx_pcm fermé → DspRunner vide rx_pcm et quitte
2. DspRunner::stop()       → tx_mfcc fermé → InferenceEngine vide rx_mfcc et quitte
3. InferenceEngine::stop() → tx_score fermé → TriggerModule vide rx_score et quitte
4. TriggerModule::stop()   → join du thread trigger
```

> Chaque étage se termine naturellement quand son `Receiver` retourne `Err` (channel fermé). `stop()` ne fait que joindre le thread, pas le forcer.

---

## 6. Gestion des signaux

| Signal | Comportement | Niveau |
|---|---|---|
| `SIGINT` (Ctrl+C) | Lève `AtomicBool shutdown_requested` → boucle principale se termine → `shutdown()` appelé | 🔴 CRITIQUE |
| `SIGTERM` | Identique à SIGINT via `ctrlc` (gère les deux) | 🟠 OBLIGATOIRE |
| `SIGHUP` | Non géré — relance manuelle requise | 🟢 OPTIONNEL |

---

## 7. Configuration runtime

| Variable d'environnement | Valeur par défaut | Description |
|---|---|---|
| `WAKEWORD_MODEL_PATH` | *(obligatoire)* | Chemin absolu vers le `.mlmodelc` |
| `WAKEWORD_SOCKET_PATH` | `/tmp/wakeword_daemon.sock` | Chemin du socket IPC |
| `WAKEWORD_THRESHOLD` | `0.80` | Seuil de score individuel |
| `WAKEWORD_COOLDOWN_MS` | `2000` | Délai minimal entre deux détections (ms) |

> `WAKEWORD_MODEL_PATH` est la seule variable obligatoire. Toutes les autres ont des valeurs par défaut sensées.

---

## 8. Contraintes de qualité

| Métrique | Objectif | Niveau |
|---|---|---|
| Latence bout-en-bout (audio → socket) | < 150 ms | 🔴 CRITIQUE |
| CPU idle (aucun son) | < 1 % | 🟠 OBLIGATOIRE |
| Arrêt propre après SIGINT | < 500 ms | 🟠 OBLIGATOIRE |
| Taille binaire release | < 5 Mo | 🟡 IMPORTANT |
| Zéro thread zombie après arrêt | Obligatoire | 🔴 CRITIQUE |
| Zéro crash si aucun client IPC | Obligatoire | 🔴 CRITIQUE |
| Socket recréé si déjà présent | Obligatoire | 🔴 CRITIQUE |

---

## 9. Structure des fichiers

| Fichier | Niveau | Rôle |
|---|---|---|
| `Cargo.toml` | 🔴 CRITIQUE | Dépendances vers les 4 crates + externes |
| `src/main.rs` | 🔴 CRITIQUE | `main()` : init config, init modules, démarrage, boucle, arrêt |
| `src/config.rs` | 🟠 OBLIGATOIRE | `DaemonConfig` : variables d'environnement → config typée |
| `src/pipeline.rs` | 🟡 IMPORTANT | `fn start_pipeline(config) → PipelineHandles` + `fn shutdown(handles)` — isole le câblage du `main` |

---

## 10. Contraintes de compatibilité workspace

| Contrainte | Niveau | Description |
|---|---|---|
| Aucune modification des crates existants | 🔴 CRITIQUE | Seul `Cargo.toml` racine est modifié (ajout de `daemon`) |
| Compatibilité `resolver = "2"` | 🔴 CRITIQUE | Feature flags par crate bien isolés |
| `audio_capture` sans feature `mock_audio` en production | 🔴 CRITIQUE | Capture microphone réelle requise |
| Feature `mock_pipeline` pour CI/tests | 🟠 OBLIGATOIRE | Permet de tester sans microphone ni modèle CoreML réel |
