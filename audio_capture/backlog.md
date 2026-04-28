# Backlog — Module `audio_capture`

> Découpage industriel des tâches élémentaires pour la conception, l'implémentation et la validation du module de capture audio.
> Chaque tâche est atomique, testable, et les tests s'incrémentent avec les fonctionnalités.
> Le module doit être **compilable, testable et exécutable de manière isolée** à tout moment.

---

## Légende

| Symbole | Signification |
|---|---|
| `[SETUP]` | Infrastructure, environnement, configuration |
| `[IMPL]` | Implémentation d'une fonctionnalité |
| `[TEST-U]` | Test unitaire (logique pure, sans hardware) |
| `[TEST-I]` | Test d'intégration (avec le vrai module, potentiellement avec hardware) |
| `[TEST-P]` | Test de performance / profiling |
| `[VALID]` | Validation manuelle ou instrumentée |
| `[ ]` | Non commencé |
| `[x]` | Terminé |

---

## PARTIE 0 — Installation & Configuration de l'environnement

> **Objectif :** Avoir un environnement de développement opérationnel, le workspace Cargo structuré, et un premier `cargo check` qui passe.

---

### P0.1 — Vérification de la toolchain

- [x] `[SETUP]` Vérifier que Rust stable ≥ 1.78 est installé (`rustup show`)
- [x] `[SETUP]` Vérifier que la target `aarch64-apple-darwin` est active (machine Apple Silicon native)
- [x] `[SETUP]` Vérifier que Xcode Command Line Tools est installé (`xcode-select --install` si absent) — requis pour les headers CoreAudio
- [x] `[SETUP]` Vérifier la présence des frameworks : `CoreAudio.framework`, `AudioToolbox.framework`, `Foundation.framework` dans `/Library/Developer/CommandLineTools/SDKs/`
- [x] `[TEST-U]` **Test de smoke :** `rustc --version && cargo --version` — les deux doivent répondre sans erreur

### P0.2 — Création de la structure du projet Cargo

- [x] `[SETUP]` Initialiser le workspace Cargo à la racine du projet (`Cargo.toml` workspace)
- [x] `[SETUP]` Créer le crate `audio_capture` comme library crate (`cargo new --lib audio_capture`)
- [x] `[SETUP]` Ajouter `audio_capture` dans le `[workspace]` du `Cargo.toml` racine
- [x] `[SETUP]` Créer le fichier `build.rs` vide à la racine du crate `audio_capture`
- [x] `[SETUP]` Créer l'arborescence initiale : `src/`, `src/audio_capture/`, `tests/`
- [x] `[TEST-U]` **Test de smoke :** `cargo check -p audio_capture` — doit compiler sans erreur ni warning

### P0.3 — Configuration des dépendances initiales

- [x] `[SETUP]` Ajouter `crossbeam = "0.8"` dans `[dependencies]` du crate
- [x] `[SETUP]` Ajouter `libc = "0.2"` dans `[dependencies]`
- [x] `[SETUP]` Ajouter `anyhow = "1.0"` dans `[dependencies]`
- [x] `[SETUP]` Ajouter `tracing = "0.1"` et `tracing-subscriber = "0.3"` dans `[dependencies]`
- [x] `[SETUP]` Configurer le `build.rs` : écrire les `println!("cargo:rustc-link-lib=framework=...")` pour `CoreAudio`, `AudioToolbox`, `Foundation`
- [x] `[TEST-U]` **Test :** `cargo build -p audio_capture` — doit compiler et linker les frameworks Apple sans erreur

### P0.4 — Feature flags & mode standalone

- [x] `[SETUP]` Déclarer le feature flag `mock_audio` dans `[features]` du `Cargo.toml`
- [x] `[SETUP]` Déclarer le feature flag `standalone` pour activer un `main` de test intégré (via `[[bin]]` ou `examples/`)
- [x] `[TEST-U]` **Test :** `cargo build -p audio_capture --features mock_audio` — doit compiler
- [x] `[TEST-U]` **Test :** `cargo build -p audio_capture --features standalone` — doit compiler

---

## PARTIE 1 — Gestion des erreurs et types de base

> **Objectif :** Définir le contrat d'erreur du module avant toute implémentation. Toute erreur possible doit être typée.

---

### P1.1 — Définition du type d'erreur du module

- [x] `[IMPL]` Créer `src/audio_capture/error.rs`
- [x] `[IMPL]` Définir l'enum `AudioCaptureError` avec les variantes : `DeviceNotFound`, `FormatUnsupported`, `UnitCreationFailed(i32)`, `UnitStartFailed(i32)`, `UnitStopFailed(i32)`, `PropertySetFailed(i32)`, `RingBufferFull`
- [x] `[IMPL]` Implémenter `std::fmt::Display` pour `AudioCaptureError`
- [x] `[IMPL]` Implémenter `std::error::Error` pour `AudioCaptureError`
- [x] `[TEST-U]` **Test unitaire :** Vérifier que chaque variante produit un message `Display` non vide
- [x] `[TEST-U]` **Test unitaire :** Vérifier que `AudioCaptureError` est `Send + Sync`

### P1.2 — Définition de la configuration

- [x] `[IMPL]` Créer `src/audio_capture/config.rs`
- [x] `[IMPL]` Définir la struct `AudioCaptureConfig` avec les champs : `sample_rate: f64`, `buffer_size_frames: u32`, `ring_capacity: usize`
- [x] `[IMPL]` Implémenter `Default` pour `AudioCaptureConfig` avec les valeurs nominales (16000 Hz, 256 frames, 32000 samples)
- [x] `[IMPL]` Implémenter une méthode `validate(&self) -> Result<(), AudioCaptureError>` qui vérifie les invariants (sample_rate > 0, buffer_size_frames puissance de 2, etc.)
- [x] `[TEST-U]` **Test unitaire :** `AudioCaptureConfig::default()` — vérifier que `validate()` retourne `Ok`
- [x] `[TEST-U]` **Test unitaire :** Config avec `sample_rate = 0` — vérifier que `validate()` retourne `Err`
- [x] `[TEST-U]` **Test unitaire :** Config avec `buffer_size_frames` non puissance de 2 — vérifier le rejet

---

## PARTIE 2 — Ring Buffer

> **Objectif :** Implémenter et valider le ring buffer lock-free qui sert de pont entre le callback RT et le consommateur. C'est la pièce la plus critique du module.

---

### P2.1 — Wrapper `AudioRingBuffer`

- [x] `[IMPL]` Créer `src/audio_capture/ring_buffer.rs`
- [x] `[IMPL]` Définir la struct `AudioRingBuffer` contenant un `Arc<ArrayQueue<f32>>`
- [x] `[IMPL]` Implémenter `AudioRingBuffer::new(capacity: usize) -> Self`
- [x] `[IMPL]` Implémenter `producer_handle(&self) -> Arc<ArrayQueue<f32>>` — retourne un clone de l'`Arc` pour le callback RT
- [x] `[IMPL]` Implémenter `consumer_handle(&self) -> Arc<ArrayQueue<f32>>` — retourne un clone de l'`Arc` pour le consommateur
- [x] `[TEST-U]` **Test unitaire :** Vérifier que `new(32_000)` crée un buffer avec la bonne capacité
- [x] `[TEST-U]` **Test unitaire :** `producer_handle` et `consumer_handle` pointent vers la même `ArrayQueue` (vérification via `Arc::ptr_eq`)

### P2.2 — Sémantique de production et consommation

- [x] `[IMPL]` Implémenter `push_sample(queue: &ArrayQueue<f32>, sample: f32)` — utilise `force_push` (écrase si plein, RT-safe)
- [x] `[IMPL]` Implémenter `drain_available(queue: &ArrayQueue<f32>) -> Vec<f32>` — draine tous les samples disponibles sans bloquer
- [x] `[IMPL]` Implémenter un compteur atomique `dropped_samples: AtomicUsize` pour tracer les overflows
- [x] `[TEST-U]` **Test unitaire :** Produire N samples, consommer N samples — vérifier l'ordre FIFO
- [x] `[TEST-U]` **Test unitaire :** Remplir le buffer au-delà de la capacité — vérifier que `dropped_samples` s'incrémente correctement
- [x] `[TEST-U]` **Test unitaire :** Consommer un buffer vide — vérifier que `drain_available` retourne un `Vec` vide sans bloquer

### P2.3 — Thread-safety du ring buffer

- [x] `[TEST-I]` **Test d'intégration :** Lancer un thread producteur (1 million de push à 16 kHz simulé) et un thread consommateur en parallèle — vérifier zéro data race (avec `cargo test` sous ThreadSanitizer si disponible)
- [x] `[TEST-U]` **Test unitaire :** Vérifier que `AudioRingBuffer` implémente `Send` et `Sync`
- [ ] `[TEST-P]` **Benchmark :** Mesurer le throughput du ring buffer (push + pop) — doit supporter > 100 000 opérations/s sans dégradation

---

## PARTIE 3 — Sélection et configuration du device audio

> **Objectif :** Interroger CoreAudio pour identifier le device d'entrée par défaut et vérifier qu'il supporte le format requis.

---

### P3.1 — Bindings FFI CoreAudio (types & constantes)

- [x] `[IMPL]` Créer `src/audio_capture/ffi.rs`
- [x] `[IMPL]` Déclarer les types C nécessaires via `libc` : `OSStatus`, `AudioObjectID`, `AudioObjectPropertyAddress`, `AudioStreamBasicDescription`, `AudioBufferList`, `AudioBuffer`
- [x] `[IMPL]` Déclarer les constantes CoreAudio utilisées : `kAudioObjectPropertyScopeInput`, `kAudioHardwarePropertyDefaultInputDevice`, `kAudioObjectSystemObject`, `kAudioFormatLinearPCM`, `kAudioFormatFlagIsFloat`, `kAudioFormatFlagIsNonInterleaved`, `noErr`
- [x] `[IMPL]` Déclarer les fonctions FFI `extern "C"` : `AudioObjectGetPropertyData`, `AudioObjectSetPropertyData`, `AudioObjectGetPropertyDataSize`
- [x] `[TEST-U]` **Test unitaire :** Vérifier que `noErr == 0` (constante de sanité)
- [x] `[TEST-U]` **Test unitaire :** Vérifier que les tailles des structs C correspondent aux tailles attendues (`std::mem::size_of`)

### P3.2 — Sélection du device d'entrée par défaut

- [x] `[IMPL]` Créer `src/audio_capture/device.rs`
- [x] `[IMPL]` Implémenter `get_default_input_device() -> Result<AudioObjectID, AudioCaptureError>` — appelle `AudioObjectGetPropertyData` avec `kAudioHardwarePropertyDefaultInputDevice`
- [x] `[IMPL]` Implémenter `device_name(device_id: AudioObjectID) -> String` — récupère le nom du device pour les logs
- [x] `[TEST-I]` **Test d'intégration :** Appeler `get_default_input_device()` sur la machine de dev — doit retourner un ID non nul (nécessite microphone système)
- [x] `[TEST-U]` **Test unitaire (mock)** *(feature `mock_audio`)* : Simuler un retour `kAudioObjectUnknown` — vérifier que `DeviceNotFound` est retourné

### P3.3 — Vérification du format supporté

- [x] `[IMPL]` Implémenter `check_format_support(device_id: AudioObjectID, config: &AudioCaptureConfig) -> Result<(), AudioCaptureError>` — vérifie que le device supporte Float32 mono 16 kHz
- [x] `[TEST-I]` **Test d'intégration :** Appeler `check_format_support` avec la config nominale — doit retourner `Ok` sur toute machine Apple Silicon
- [x] `[TEST-U]` **Test unitaire (mock)** : Simuler un device qui ne supporte pas le format — vérifier que `FormatUnsupported` est retourné

---

## PARTIE 4 — AudioUnit : Setup, Callback et Cycle de vie

> **Objectif :** Configurer l'AudioUnit AUHAL pour la capture, enregistrer le callback RT, et gérer le cycle de vie start/stop proprement.

---

### P4.1 — Bindings FFI AudioUnit

- [ ] `[IMPL]` Dans `ffi.rs`, ajouter les types : `AudioUnit`, `AudioComponent`, `AudioComponentDescription`, `AudioUnitRenderActionFlags`, `AudioTimeStamp`
- [ ] `[IMPL]` Déclarer les fonctions FFI : `AudioComponentFindNext`, `AudioComponentInstanceNew`, `AudioComponentInstanceDispose`, `AudioUnitInitialize`, `AudioUnitUninitialize`, `AudioOutputUnitStart`, `AudioOutputUnitStop`, `AudioUnitSetProperty`, `AudioUnitGetProperty`, `AudioUnitAddRenderNotify`
- [ ] `[IMPL]` Déclarer les constantes : `kAudioUnitType_Output`, `kAudioUnitSubType_HALOutput`, `kAudioUnitManufacturer_Apple`, `kAudioOutputUnitProperty_EnableIO`, `kAudioOutputUnitProperty_CurrentDevice`, `kAudioUnitProperty_StreamFormat`, `kAudioUnitScope_Input`, `kAudioUnitScope_Output`
- [ ] `[TEST-U]` **Test unitaire :** Vérifier la taille de `AudioComponentDescription` attendue par l'ABI Apple

### P4.2 — Construction et configuration de l'AudioUnit

- [ ] `[IMPL]` Créer `src/audio_capture/unit.rs`
- [ ] `[IMPL]` Implémenter `AudioUnitCapture::new(device_id: AudioObjectID, config: &AudioCaptureConfig) -> Result<Self, AudioCaptureError>`
  - [ ] `[IMPL]` Trouver le composant AUHAL via `AudioComponentFindNext`
  - [ ] `[IMPL]` Instancier le composant via `AudioComponentInstanceNew`
  - [ ] `[IMPL]` Activer l'IO d'entrée et désactiver l'IO de sortie (`kAudioOutputUnitProperty_EnableIO`)
  - [ ] `[IMPL]` Sélectionner le device (`kAudioOutputUnitProperty_CurrentDevice`)
  - [ ] `[IMPL]` Configurer le format de stream (`kAudioUnitProperty_StreamFormat`) : Float32, mono, 16 kHz, non-entrelacé
  - [ ] `[IMPL]` Appeler `AudioUnitInitialize`
- [ ] `[TEST-I]` **Test d'intégration :** Appeler `AudioUnitCapture::new` avec la config nominale — doit retourner `Ok` sans erreur (nécessite microphone)
- [ ] `[TEST-U]` **Test unitaire (mock)** : Simuler un échec de `AudioComponentFindNext` (retour null) — vérifier `UnitCreationFailed`

### P4.3 — Enregistrement du callback RT

- [ ] `[IMPL]` Définir la signature du callback : `unsafe extern "C" fn audio_render_callback(in_ref_con, action_flags, time_stamp, bus_number, num_frames, io_data) -> OSStatus`
- [ ] `[IMPL]` Dans le callback : extraire les samples Float32 depuis `AudioBufferList`, appeler `force_push` sur le `ArrayQueue` — **zéro allocation, zéro lock**
- [ ] `[IMPL]` Passer le pointeur vers l'`Arc<ArrayQueue<f32>>` via `in_ref_con` (mécanisme `Box::into_raw` / `Box::from_raw` sécurisé)
- [ ] `[IMPL]` Enregistrer le callback via `AudioUnitSetProperty` avec `kAudioUnitProperty_SetRenderCallback`
- [ ] `[TEST-I]` **Test d'intégration :** Démarrer la capture 100 ms, vérifier que des samples ont été poussés dans le ring buffer (count > 0)
- [ ] `[VALID]` **Validation manuelle :** Inspecter les valeurs PCM dans la console — vérifier qu'elles sont dans `[-1.0, 1.0]`

### P4.4 — Cycle de vie start / stop

- [ ] `[IMPL]` Implémenter `AudioUnitCapture::start() -> Result<(), AudioCaptureError>` — appelle `AudioOutputUnitStart`
- [ ] `[IMPL]` Implémenter `AudioUnitCapture::stop() -> Result<(), AudioCaptureError>` — appelle `AudioOutputUnitStop` puis `AudioUnitUninitialize`
- [ ] `[IMPL]` Implémenter `Drop for AudioUnitCapture` — appelle `stop()` puis `AudioComponentInstanceDispose` pour éviter toute fuite
- [ ] `[TEST-I]` **Test d'intégration :** Start → attendre 200 ms → Stop — vérifier `Ok` aux deux étapes et que le ring buffer est rempli
- [ ] `[TEST-I]` **Test d'intégration :** Double `stop()` — vérifier l'idempotence (pas de panic, pas d'erreur doublée)
- [ ] `[TEST-I]` **Test d'intégration :** Drop sans stop explicite — vérifier que `Drop` arrête proprement (pas de process suspendu)

---

## PARTIE 5 — Thread consommateur

> **Objectif :** Implémenter le thread Rust qui lit le ring buffer périodiquement et expose les samples vers le reste du pipeline.

---

### P5.1 — Structure du consommateur

- [ ] `[IMPL]` Créer `src/audio_capture/consumer.rs`
- [ ] `[IMPL]` Définir la struct `AudioConsumer` avec : `queue: Arc<ArrayQueue<f32>>`, `poll_interval_ms: u64`, `running: Arc<AtomicBool>`, `thread_handle: Option<JoinHandle<()>>`
- [ ] `[IMPL]` Implémenter `AudioConsumer::new(queue: Arc<ArrayQueue<f32>>, poll_interval_ms: u64) -> Self`

### P5.2 — Boucle de consommation

- [ ] `[IMPL]` Implémenter `AudioConsumer::start(sender: Sender<Vec<f32>>) -> Result<(), AudioCaptureError>` — spawn d'un thread qui drain le ring buffer toutes les `poll_interval_ms` ms et envoie les batches via le `Sender`
- [ ] `[IMPL]` Utiliser `std::thread::sleep(Duration::from_millis(poll_interval_ms))` dans la boucle
- [ ] `[IMPL]` Vérifier `running.load(Ordering::Relaxed)` à chaque itération pour sortir proprement
- [ ] `[IMPL]` Implémenter `AudioConsumer::stop()` — pose `running` à `false` et `join` le thread

### P5.3 — Tests du consommateur

- [ ] `[TEST-U]` **Test unitaire :** Pré-remplir le ring buffer avec des samples connus, lancer le consommateur, vérifier que le `Receiver` reçoit les mêmes valeurs
- [ ] `[TEST-U]` **Test unitaire :** Lancer le consommateur sur un ring buffer vide — vérifier qu'il ne bloque pas et n'envoie pas de batch vide
- [ ] `[TEST-I]` **Test d'intégration :** Consommateur branché sur le vrai module de capture (capture 500 ms) — vérifier que tous les batches reçus sont non vides et dans `[-1.0, 1.0]`
- [ ] `[TEST-I]` **Test d'intégration :** Stop du consommateur pendant la capture — vérifier que le thread se termine proprement sans panic

---

## PARTIE 6 — Façade publique du module

> **Objectif :** Exposer une API unifiée, simple et stable. C'est la seule surface visible depuis le reste du workspace.

---

### P6.1 — Struct `AudioCapture` (façade)

- [ ] `[IMPL]` Créer `src/audio_capture/mod.rs` (ou `lib.rs` si crate autonome)
- [ ] `[IMPL]` Définir la struct `AudioCapture` qui agrège `AudioUnitCapture` et `AudioConsumer`
- [ ] `[IMPL]` Implémenter `AudioCapture::new(config: AudioCaptureConfig) -> Result<Self, AudioCaptureError>` — appelle `get_default_input_device`, `AudioUnitCapture::new`, `AudioRingBuffer::new`, `AudioConsumer::new`
- [ ] `[IMPL]` Implémenter `AudioCapture::start(sender: Sender<Vec<f32>>) -> Result<(), AudioCaptureError>` — démarre l'AudioUnit et le consommateur
- [ ] `[IMPL]` Implémenter `AudioCapture::stop() -> Result<(), AudioCaptureError>` — arrête dans l'ordre : consommateur puis AudioUnit
- [ ] `[IMPL]` Implémenter `Drop for AudioCapture` — appelle `stop()` silencieusement
- [ ] `[TEST-I]` **Test d'intégration :** Cycle complet `new → start → 1s capture → stop` — vérifier que des samples valides ont été reçus
- [ ] `[TEST-I]` **Test d'intégration :** Deux cycles successifs `start/stop` — vérifier l'idempotence de la réinitialisation

### P6.2 — Mode standalone (exécution sans pipeline)

- [ ] `[IMPL]` Créer `examples/standalone_capture.rs` (activé par feature `standalone`)
- [ ] `[IMPL]` Le binaire d'exemple : initialise le module, capture 3 secondes, affiche les statistiques (nombre de samples reçus, drop rate), puis s'arrête proprement
- [ ] `[TEST-I]` **Test d'intégration :** `cargo run --example standalone_capture --features standalone` — doit s'exécuter sans erreur et afficher des statistiques cohérentes
- [ ] `[VALID]` **Validation manuelle :** Parler dans le micro pendant l'exemple, vérifier visuellement que les amplitudes varient

---

## PARTIE 7 — Tests de régression & suite de tests complète

> **Objectif :** Consolider tous les tests précédents, ajouter les tests de régression manquants, et s'assurer que la suite complète passe en CI.

---

### P7.1 — Organisation du dossier de tests

- [ ] `[SETUP]` Créer `tests/audio_capture_integration.rs` comme fichier de tests d'intégration du crate
- [ ] `[SETUP]` Séparer les tests nécessitant un microphone physique avec le tag `#[cfg(not(feature = "mock_audio"))]`
- [ ] `[SETUP]` Créer un module de mock audio dans `src/audio_capture/mock.rs` (activé par feature `mock_audio`) qui simule un ring buffer pré-rempli avec un signal sinusoïdal connu

### P7.2 — Tests unitaires de régression

- [ ] `[TEST-U]` **Régression :** Config default → validate → Ok
- [ ] `[TEST-U]` **Régression :** Toutes les variantes de `AudioCaptureError` ont un Display non vide
- [ ] `[TEST-U]` **Régression :** Ring buffer FIFO sur 1000 samples
- [ ] `[TEST-U]` **Régression :** Ring buffer overflow → `dropped_samples` > 0
- [ ] `[TEST-U]` **Régression :** Ring buffer thread-safety (producteur + consommateur simultanés)
- [ ] `[TEST-U]` **Régression :** Tailles des structs FFI (`AudioComponentDescription`, `AudioStreamBasicDescription`)

### P7.3 — Tests d'intégration de régression (mock)

- [ ] `[TEST-I]` **Régression (mock)** : Cycle complet `new → start → drain → stop` avec signal sinusoïdal synthétique — vérifier le contenu des batches reçus
- [ ] `[TEST-I]` **Régression (mock)** : Start sans stop → Drop — vérifier zéro resource leak (thread zombie interdit)
- [ ] `[TEST-I]` **Régression (mock)** : Deux instances simultanées — vérifier l'isolation des ring buffers

### P7.4 — Tests d'intégration réels (avec microphone)

- [ ] `[TEST-I]` **Réel :** Device par défaut détecté → ID non nul
- [ ] `[TEST-I]` **Réel :** Format Float32 mono 16 kHz supporté par le device par défaut
- [ ] `[TEST-I]` **Réel :** Capture 1 seconde → ≥ 15 000 samples reçus (tolérance de ±10% sur 16 000 attendus)
- [ ] `[TEST-I]` **Réel :** Tous les samples dans `[-1.0, 1.0]`
- [ ] `[TEST-I]` **Réel :** Drop rate < 0,1 % sur 5 secondes de capture

### P7.5 — Tests de performance

- [ ] `[TEST-P]` **Benchmark :** Throughput du ring buffer (push/pop 1M ops) via `cargo-criterion`
- [ ] `[TEST-P]` **Benchmark :** Latence du consommateur (temps entre push et réception) — doit être < 15 ms en médiane
- [ ] `[VALID]` **Validation Instruments :** CPU du processus pendant 30 s de capture en arrière-plan — doit rester < 0,2 %
- [ ] `[VALID]` **Validation AddressSanitizer :** `RUSTFLAGS="-Z sanitizer=address" cargo +nightly test` — zéro erreur mémoire

---

## PARTIE 8 — Documentation & Livraison du module

> **Objectif :** Le module est documenté, versionné, et prêt à être intégré dans le workspace complet.

---

### P8.1 — Documentation du module

- [ ] `[SETUP]` Ajouter des doc-comments `///` sur tous les types et fonctions publics (`AudioCapture`, `AudioCaptureConfig`, `AudioCaptureError`)
- [ ] `[SETUP]` Écrire un doc-example dans le commentaire de `AudioCapture::new` montrant le cycle minimal `new/start/stop`
- [ ] `[TEST-U]` **Test :** `cargo doc --no-deps -p audio_capture` — doit s'exécuter sans erreur ni warning

### P8.2 — Validation finale du module

- [ ] `[VALID]` `cargo clippy -p audio_capture -- -D warnings` — zéro warning
- [ ] `[VALID]` `cargo fmt --check -p audio_capture` — code formaté
- [ ] `[VALID]` `cargo test -p audio_capture` — suite complète verte
- [ ] `[VALID]` `cargo test -p audio_capture --features mock_audio` — suite mock verte (sans microphone)
- [ ] `[VALID]` `cargo build -p audio_capture --release` — binaire release compile proprement

### P8.3 — Intégration dans le workspace

- [ ] `[SETUP]` Vérifier que `audio_capture` est bien déclaré dans le `[workspace]` racine
- [ ] `[SETUP]` Documenter dans `audio_capture/README.md` : prérequis, comment lancer les tests, comment utiliser l'API depuis un autre crate du workspace
- [ ] `[TEST-I]` **Test d'intégration finale :** Depuis un crate `pipeline` factice (crate vide dans le workspace), importer `audio_capture` et appeler `AudioCapture::new(Default::default())` — doit compiler sans erreur

---

## Récapitulatif par partie

| Partie | Thème | Dépend de |
|---|---|---|
| P0 | Installation & Setup | — |
| P1 | Erreurs & Config | P0 |
| P2 | Ring Buffer | P1 |
| P3 | Sélection Device | P1, P2 |
| P4 | AudioUnit & Callback | P2, P3 |
| P5 | Thread consommateur | P2, P4 |
| P6 | Façade publique | P3, P4, P5 |
| P7 | Suite de tests complète | P1→P6 |
| P8 | Documentation & Livraison | P7 |
