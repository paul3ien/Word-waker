# Backlog — Crate `daemon`

> Découpage industriel des tâches élémentaires pour la conception, l'implémentation et la validation du binaire daemon.
> Chaque tâche est atomique, testable, et les tests s'incrémentent avec les fonctionnalités.
> Le daemon doit être **compilable, démarrable et arrêtable proprement** à tout moment du développement.

---

## Légende

| Symbole | Signification |
|---|---|
| `[SETUP]` | Infrastructure, environnement, configuration |
| `[IMPL]` | Implémentation d'une fonctionnalité |
| `[TEST-I]` | Test d'intégration (pipeline réel ou mocks) |
| `[VALID]` | Validation manuelle ou instrumentée |
| `[ ]` | Non commencé |
| `[x]` | Terminé |

---

## PARTIE 0 — Installation & Configuration de l'environnement

> **Objectif :** Avoir un crate `daemon` qui compile avec `cargo check` sans modifier les crates existants.

---

### P0.1 — Création de la structure du crate

- [x] `[SETUP]` Créer le crate `daemon` comme binary crate dans le workspace (`cargo new --bin daemon`)
- [x] `[SETUP]` Ajouter `daemon` dans le `[workspace]` du `Cargo.toml` racine
- [x] `[SETUP]` Créer l'arborescence : `src/`, `src/config.rs`, `src/pipeline.rs`
- [x] `[TEST-I]` **Test de smoke :** `cargo check -p daemon` — passe sans erreur

### P0.2 — Configuration des dépendances

- [x] `[SETUP]` Ajouter dans `[dependencies]` :
  - `audio_capture = { path = "../audio_capture" }`
  - `pipeline_dsp = { path = "../pipeline" }`
  - `inference_ml = { path = "../inference_ml" }`
  - `trigger = { path = "../trigger" }`
  - `crossbeam-channel = "0.5"`
  - `anyhow = "1.0"`
  - `tracing = "0.1"`
  - `tracing-subscriber = "0.3"`
- [x] `[SETUP]` Ajouter dans `[features]` : `mock_pipeline` — remplace `audio_capture` par des samples synthétiques (pour CI sans microphone)
- [x] `[TEST-I]` **Test :** `cargo build -p daemon` — compile sans erreur

### P0.3 — Configuration runtime

- [x] `[SETUP]` Créer `src/config.rs` avec la struct `DaemonConfig` :
  - `model_path: String` — chemin vers le modèle CoreML (`.mlmodelc`)
  - `socket_path: String` — chemin du socket IPC (défaut `/tmp/wakeword_daemon.sock`)
  - `score_threshold: f32` — seuil de détection (défaut `0.80`)
  - `cooldown_ms: u64` — délai entre deux détections (défaut `2000`)
- [x] `[IMPL]` Implémenter `DaemonConfig::from_env()` — lit `WAKEWORD_MODEL_PATH`, `WAKEWORD_SOCKET_PATH`, `WAKEWORD_THRESHOLD`, `WAKEWORD_COOLDOWN_MS` depuis les variables d'environnement avec valeurs par défaut
- [x] `[TEST-I]` **Test :** `DaemonConfig::from_env()` avec variables non définies → valeurs par défaut valides

---

## PARTIE 1 — Câblage du pipeline

> **Objectif :** Assembler les 4 crates en pipeline via des channels crossbeam. Aucun code nouveau dans les crates existants.

---

### P1.1 — Channels inter-étages

- [x] `[IMPL]` Créer les 3 channels dans `main` :
  - `(tx_pcm, rx_pcm)` — `std::sync::mpsc` (exigé par AudioCapture) + pont crossbeam pour DspRunner
  - `(tx_mfcc, rx_mfcc): (Sender<[[f32;13];98]>, Receiver<[[f32;13];98]>)` — pipeline_dsp → inference_ml
  - `(tx_score, rx_score): (Sender<f32>, Receiver<f32>)` — inference_ml → trigger

### P1.2 — Initialisation des modules

- [x] `[IMPL]` Initialiser `AudioCapture::new(AudioCaptureConfig::default())`
- [x] `[IMPL]` Initialiser `DspRunner::start(DspConfig::default(), rx_pcm, tx_mfcc)`
- [x] `[IMPL]` Initialiser `InferenceEngine::new(InferenceConfig { model_path: config.model_path, ..Default::default() })`
- [x] `[IMPL]` Initialiser `TriggerModule::new(TriggerConfig { socket_path, score_threshold, cooldown_ms, ..Default::default() })`
- [ ] `[TEST-I]` **Test de smoke :** Avec `mock_pipeline`, tous les modules s'initialisent sans erreur

### P1.3 — Démarrage en séquence

- [x] `[IMPL]` Démarrer dans l'ordre (inverse de la propagation de signal) :
  1. `trigger.start(rx_score)`
  2. `engine.start(rx_mfcc, tx_score)`
  3. `dsp_runner` (déjà démarré dans `DspRunner::start`)
  4. `capture.start(tx_pcm)`
- [x] `[IMPL]` Logger chaque démarrage réussi avec `tracing::info!`

### P1.4 — Arrêt propre

- [x] `[IMPL]` Implémenter `fn shutdown(...)` qui arrête dans l'ordre inverse :
  1. `capture.stop()` — ferme `tx_pcm`, signal de fermeture vers DSP
  2. Drop de `tx_pcm` — le `DspRunner` se termine quand `rx_pcm` est épuisé
  3. `engine.stop()`
  4. `trigger.stop()`
- [x] `[IMPL]` Chaque `stop()` est appelé même si le précédent a retourné une erreur (log + continue)
- [ ] `[TEST-I]` **Test :** Démarrage → immédiat arrêt → zéro thread zombie, zéro panique

---

## PARTIE 2 — Signal handling & boucle principale

> **Objectif :** Le daemon tourne jusqu'à `Ctrl+C` (SIGINT) ou SIGTERM et s'arrête proprement.

---

### P2.1 — Capture de SIGINT / SIGTERM

- [x] `[IMPL]` Ajouter dépendance `ctrlc = "3"` dans `[dependencies]`
- [x] `[IMPL]` Installer un handler `ctrlc::set_handler` qui lève un `AtomicBool` `shutdown_requested`
- [x] `[IMPL]` La boucle principale (`loop { if shutdown.load() { break; } thread::sleep(100ms); }`) se termine dès que le flag est levé

### P2.2 — Logging au démarrage

- [x] `[IMPL]` Initialiser `tracing_subscriber::fmt().with_max_level(Level::INFO).init()` au début de `main`
- [x] `[IMPL]` Logger au démarrage :
  ```
  [INFO] Word Waker daemon démarré
  [INFO] Modèle : <model_path>
  [INFO] Socket : <socket_path>
  [INFO] Seuil  : <score_threshold>
  [INFO] Cooldown : <cooldown_ms> ms
  ```
- [x] `[IMPL]` Logger à l'arrêt : `[INFO] Arrêt propre — au revoir.`

### P2.3 — Codes de sortie

- [x] `[IMPL]` `main` retourne `anyhow::Result<()>` — toute erreur fatale s'affiche avec contexte complet et exit code 1
- [x] `[IMPL]` Arrêt normal (SIGINT/SIGTERM) → exit code 0
- [ ] `[VALID]` Lancer `cargo run -p daemon -- && echo "exit:$?"` → exit code 0 après Ctrl+C

---

## PARTIE 3 — Validation end-to-end

> **Objectif :** Vérifier le comportement en conditions réelles et avec mocks.

---

### P3.1 — Validation avec microphone réel

- [ ] `[VALID]` **Démarrage :** `cargo run --release -p daemon` — démarre sans erreur, logs INFO visibles
- [ ] `[VALID]` **Détection :** Prononcer le mot-clé → message `WAKEWORD_DETECTED\n` visible via `nc -U /tmp/wakeword_daemon.sock`
- [ ] `[VALID]` **Cooldown :** Deux prononciations rapides → une seule détection (cooldown respecté)
- [ ] `[VALID]` **Arrêt :** Ctrl+C → logs d'arrêt propre, exit code 0, socket supprimé

### P3.2 — Validation sans microphone (CI / mock_pipeline)

- [ ] `[TEST-I]` **Feature `mock_pipeline` :** Injecter 3 matrices MFCC factices → vérifier réception d'un score sur `rx_score`
- [ ] `[VALID]` `cargo test -p daemon` — tous les tests passent sans microphone

### P3.3 — Robustesse

- [ ] `[VALID]` **Modèle absent :** `WAKEWORD_MODEL_PATH=/nonexistent cargo run -p daemon` → erreur claire, exit code 1
- [ ] `[VALID]` **Socket déjà présent :** Relancer le daemon sans Ctrl+C préalable → socket recréé, pas d'erreur `AddrInUse`
- [ ] `[VALID]` **Aucun client IPC :** Daemon en marche, aucun `nc` connecté → détection silencieuse, pas de crash

---

## PARTIE 4 — Performance & outillage

> **Objectif :** Vérifier que le daemon ne consomme pas de ressources inutiles en idle.

---

### P4.1 — CPU idle

- [ ] `[VALID]` Daemon démarré, aucun son — `ps -o %cpu= -p <pid>` sur 5 échantillons → CPU < 1 % (threads bloqués sur `recv()`)

### P4.2 — Latence bout-en-bout

- [ ] `[VALID]` Latence audio → socket : mesurer avec `Instruments → Time Profiler` — objectif < 150 ms (pipeline complet : 100 ms/inférence + ~17 µs CoreML + <5 ms IPC)

### P4.3 — Build release

- [ ] `[VALID]` `cargo build --release -p daemon` — compile sans warning, binaire généré dans `target/release/daemon`
- [ ] `[VALID]` Taille du binaire `target/release/daemon` < 5 Mo (hors modèle CoreML)
