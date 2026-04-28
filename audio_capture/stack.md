# Stack Technique — Module `audio_capture`

> Ce fichier documente les technologies, dépendances et contraintes propres au module de capture audio.
> Le module doit être utilisable de manière **autonome** (tests isolés) et **intégré** dans le pipeline complet.

---

## Légende des niveaux d'importance

| Niveau | Signification |
|--------|---------------|
| 🔴 CRITIQUE | Bloquant — le module ne peut pas fonctionner sans |
| 🟠 OBLIGATOIRE | Requis pour respecter les contraintes de performance et de sécurité |
| 🟡 IMPORTANT | Fortement recommandé, contournement possible à court terme uniquement |
| 🟢 OPTIONNEL | Amélioration ou outillage, non bloquant |

---

## 1. Langage & Toolchain

| Technologie | Version min | Niveau | Justification |
|---|---|---|---|
| Rust | 1.78+ | 🔴 CRITIQUE | Langage principal — gestion mémoire sans GC, FFI propre |
| Cargo | (bundled) | 🔴 CRITIQUE | Build system, gestion des dépendances |
| Target `aarch64-apple-darwin` | macOS 14+ | 🔴 CRITIQUE | Seule cible supportée — Apple Silicon obligatoire |
| Edition Rust 2021 | — | 🟠 OBLIGATOIRE | Syntaxe moderne, resolver v2 |
| `rustfmt` | (stable) | 🟡 IMPORTANT | Formatage uniforme du code |
| `clippy` | (stable) | 🟡 IMPORTANT | Linting — détecter les anti-patterns unsafe |

---

## 2. API système Apple (FFI — sans wrapper Obj-C)

| Technologie | Niveau | Justification |
|---|---|---|
| `CoreAudio.framework` — HAL C API | 🔴 CRITIQUE | Seule API permettant un callback audio temps-réel sur thread RT Apple |
| `AudioUnit` / `AudioComponent` | 🔴 CRITIQUE | Composant HAL utilisé pour la capture microphone bas-niveau |
| `AudioToolbox.framework` | 🔴 CRITIQUE | Complémentaire à CoreAudio, types `AudioStreamBasicDescription` |
| `kAudioFormatLinearPCM` | 🔴 CRITIQUE | Format de sortie imposé : Float32, mono, 16 kHz, non-entrelacé |
| `AudioObjectGetPropertyData` | 🟠 OBLIGATOIRE | Enumération et sélection du device d'entrée par défaut |
| `Foundation.framework` | 🟠 OBLIGATOIRE | Requis au link même sans Obj-C (types de base Apple) |

---

## 3. Thread & Concurrence

| Technologie | Niveau | Justification |
|---|---|---|
| Thread RT Apple (callback CoreAudio) | 🔴 CRITIQUE | Le callback s'exécute sur un thread RT managé par macOS — non contrôlable par Rust |
| `crossbeam::ArrayQueue` | 🔴 CRITIQUE | Ring buffer lock-free wait-free — seule structure sûre dans un contexte RT |
| `std::sync::Arc` | 🔴 CRITIQUE | Partage du ring buffer entre thread RT (producteur) et thread DSP (consommateur) |
| `std::thread` (thread DSP consommateur) | 🟠 OBLIGATOIRE | Thread Rust dédié qui lit le ring buffer toutes les 10 ms |
| Interdiction absolue de `Mutex` dans le callback | 🔴 CRITIQUE | Toute acquisition de lock dans le thread RT provoque un audio glitch ou watchdog kill |
| Interdiction d'allocation heap dans le callback | 🔴 CRITIQUE | `Box`, `Vec`, `String` etc. sont prohibés dans le callback audio |

---

## 4. Dépendances Cargo (module `audio_capture`)

| Crate | Version | Niveau | Rôle |
|---|---|---|---|
| `crossbeam` | 0.8+ | 🔴 CRITIQUE | `ArrayQueue` — ring buffer lock-free entre callback RT et consommateur |
| `libc` | 0.2+ | 🔴 CRITIQUE | Types C natifs : `c_void`, `OSStatus`, `UInt32`, pointeurs FFI |
| `anyhow` | 1.0+ | 🟠 OBLIGATOIRE | Propagation d'erreurs ergonomique pour l'init de l'AudioUnit |
| `tracing` | 0.1+ | 🟡 IMPORTANT | Logs structurés (latence, drop de samples, événements RT) |
| `tracing-subscriber` | 0.3+ | 🟡 IMPORTANT | Backends de logs pour les tests et le daemon |
| `cc` (build-dep) | 1.0+ | 🟡 IMPORTANT | Compilation future du bridge Obj-C++ (pas requis à ce stade) |

---

## 5. Paramètres audio — Contraintes figées

| Paramètre | Valeur | Niveau | Justification |
|---|---|---|---|
| Fréquence d'échantillonnage | 16 000 Hz | 🔴 CRITIQUE | Optimale pour la reconnaissance vocale ; imposée par le modèle ML aval |
| Format d'encodage | Float32 | 🔴 CRITIQUE | Format natif CoreAudio — pas de conversion nécessaire |
| Canaux | Mono (1) | 🔴 CRITIQUE | Le pipeline DSP attend un signal mono |
| Entrelacement | Non-entrelacé | 🔴 CRITIQUE | `kAudioFormatFlagIsNonInterleaved` requis |
| Taille de buffer HAL | 256 frames | 🟠 OBLIGATOIRE | ≈ 16 ms de latence d'acquisition — bon compromis latence/CPU |
| Capacité du ring buffer | 32 000 samples | 🟠 OBLIGATOIRE | 2 secondes d'audio — absorbe les pics de latence du consommateur |

---

## 6. Structure du module (organisation des fichiers)

| Fichier / Répertoire | Niveau | Rôle |
|---|---|---|
| `src/audio_capture/mod.rs` | 🔴 CRITIQUE | Point d'entrée public du module, re-exports |
| `src/audio_capture/device.rs` | 🔴 CRITIQUE | Sélection et configuration du device CoreAudio |
| `src/audio_capture/unit.rs` | 🔴 CRITIQUE | Setup, start/stop de l'AudioUnit, callback RT |
| `src/audio_capture/ring_buffer.rs` | 🔴 CRITIQUE | Wrapper `AudioRingBuffer` autour de `crossbeam::ArrayQueue` |
| `src/audio_capture/consumer.rs` | 🟠 OBLIGATOIRE | Thread consommateur — lit le ring buffer et émet vers le pipeline |
| `src/audio_capture/error.rs` | 🟠 OBLIGATOIRE | Types d'erreurs spécifiques au module (`AudioCaptureError`) |
| `tests/audio_capture_integration.rs` | 🟡 IMPORTANT | Tests d'intégration isolés du reste du pipeline |
| `build.rs` | 🟡 IMPORTANT | Linkage des frameworks Apple (`CoreAudio`, `AudioToolbox`, `Foundation`) |

---

## 7. Interface publique du module (contrat)

| Symbole | Niveau | Description |
|---|---|---|
| `AudioCapture::new(config)` | 🔴 CRITIQUE | Constructeur — configure le device et l'AudioUnit |
| `AudioCapture::start()` | 🔴 CRITIQUE | Démarre le callback RT |
| `AudioCapture::stop()` | 🔴 CRITIQUE | Arrête proprement l'AudioUnit |
| `AudioCapture::consumer_handle()` | 🔴 CRITIQUE | Retourne un `Arc<ArrayQueue<f32>>` pour le pipeline aval |
| `AudioCaptureConfig` (struct) | 🟠 OBLIGATOIRE | Paramètres configurables : sample rate, buffer size, capacité ring |
| `AudioCaptureError` (enum) | 🟠 OBLIGATOIRE | Erreurs typées : `DeviceNotFound`, `FormatUnsupported`, `UnitStartFailed` |
| `AudioCapture::drop()` | 🟠 OBLIGATOIRE | `Drop` impl — stop automatique si non arrêté manuellement |

---

## 8. Contraintes de qualité & métriques du module

| Métrique | Objectif | Niveau |
|---|---|---|
| Zéro allocation dans le callback RT | Obligatoire | 🔴 CRITIQUE |
| Zéro `Mutex` / `RwLock` dans le callback RT | Obligatoire | 🔴 CRITIQUE |
| Aucune fuite mémoire (valider via AddressSanitizer) | Zéro fuite | 🔴 CRITIQUE |
| Latence d'acquisition | < 20 ms | 🟠 OBLIGATOIRE |
| Drop rate de samples en régime permanent | < 0,1 % | 🟠 OBLIGATOIRE |
| Consommation CPU du module isolé | < 0,2 % | 🟠 OBLIGATOIRE |
| Couverture de tests unitaires (hors callback RT) | > 80 % | 🟡 IMPORTANT |
| Compilable et testable sans le reste du workspace | Obligatoire | 🔴 CRITIQUE |

---

## 9. Outillage de test & profiling

| Outil | Niveau | Usage |
|---|---|---|
| `cargo test` | 🔴 CRITIQUE | Tests unitaires et d'intégration du module |
| `cargo test --features mock_audio` | 🟠 OBLIGATOIRE | Tests sans microphone physique (audio simulé) |
| `AddressSanitizer` (`-Z sanitizer=address`) | 🟠 OBLIGATOIRE | Détection de fuites mémoire dans les zones `unsafe` |
| `Instruments.app` → CPU Profiler | 🟡 IMPORTANT | Vérification consommation CPU en conditions réelles |
| `cargo-criterion` | 🟡 IMPORTANT | Benchmarks du ring buffer et du consommateur |
| `cargo-flamegraph` | 🟢 OPTIONNEL | Flamegraphs via dtrace pour identifier les hotspots |

---

## 10. Intégration dans le pipeline complet

| Contrainte | Niveau | Description |
|---|---|---|
| Interface via `Arc<ArrayQueue<f32>>` uniquement | 🔴 CRITIQUE | Le module n'expose jamais de canal interne — uniquement le handle du ring buffer |
| Pas de dépendance vers les modules `dsp`, `inference`, `trigger` | 🔴 CRITIQUE | Découplage strict — `audio_capture` ne connaît pas ses consommateurs |
| Feature flag `standalone` pour exécution autonome | 🟠 OBLIGATOIRE | Permet de tester le module seul avec un consommateur minimal (`main` de test) |
| Compatibilité avec `crossbeam-channel` pour le pipeline | 🟠 OBLIGATOIRE | Le thread consommateur peut être wrappé derrière un `Sender<Vec<f32>>` |
