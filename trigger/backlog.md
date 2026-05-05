# Backlog — Module `trigger`

> Découpage industriel des tâches élémentaires pour la conception, l'implémentation et la validation du module de déclenchement et IPC.
> Chaque tâche est atomique, testable, et les tests s'incrémentent avec les fonctionnalités.
> Le module doit être **compilable, testable et exécutable de manière isolée** (sans `inference_ml` ni daemon) à tout moment.

---

## Légende

| Symbole | Signification |
|---|---|
| `[SETUP]` | Infrastructure, environnement, configuration |
| `[IMPL]` | Implémentation d'une fonctionnalité |
| `[TEST-U]` | Test unitaire (logique pure, scores injectés manuellement) |
| `[TEST-I]` | Test d'intégration (socket réel, thread réel) |
| `[TEST-P]` | Test de performance / latence |
| `[VALID]` | Validation manuelle ou instrumentée |
| `[ ]` | Non commencé |
| `[x]` | Terminé |

---

## PARTIE 0 — Installation & Configuration de l'environnement

> **Objectif :** Avoir un crate `trigger` qui compile et passe `cargo check`, sans aucune dépendance externe.

---

### P0.1 — Création de la structure du crate

- [x] `[SETUP]` Créer le crate `trigger` comme library crate dans le workspace (`cargo new --lib trigger`)
- [x] `[SETUP]` Ajouter `trigger` dans le `[workspace]` du `Cargo.toml` racine
- [x] `[SETUP]` Créer l'arborescence : `src/trigger/`, `tests/`
- [x] `[TEST-U]` **Test de smoke :** `cargo check -p trigger` — passe sans erreur

### P0.2 — Configuration des dépendances

- [x] `[SETUP]` Ajouter `crossbeam-channel = "0.5"` dans `[dependencies]`
- [x] `[SETUP]` Ajouter `anyhow = "1.0"` dans `[dependencies]`
- [x] `[SETUP]` Ajouter `tracing = "0.1"` et `tracing-subscriber = "0.3"` dans `[dependencies]`
- [x] `[TEST-U]` **Test :** `cargo build -p trigger` — compile sans erreur (aucun `build.rs` requis pour ce module)

### P0.3 — Feature flags

- [x] `[SETUP]` Déclarer feature `mock_scores` — permet d'injecter des séquences de scores synthétiques sans `inference_ml`
- [x] `[SETUP]` Déclarer feature `standalone` — active un `example` exécutable en autonomie
- [x] `[TEST-U]` **Test :** `cargo build -p trigger --features mock_scores,standalone` — compile

---

## PARTIE 1 — Gestion des erreurs et configuration

> **Objectif :** Définir les contrats d'erreur et de configuration avant toute implémentation.

---

### P1.1 — Type d'erreur du module

- [x] `[IMPL]` Créer `src/trigger/error.rs`
- [x] `[IMPL]` Définir l'enum `TriggerError` avec les variantes :
  - `ChannelClosed` — le `Receiver<f32>` a été fermé
  - `IpcSendFailed(String)` — échec d'écriture sur le socket
  - `SocketBindFailed(String)` — échec de création du socket (mode serveur)
  - `InvalidConfig(String)` — paramètres incohérents
- [x] `[IMPL]` Implémenter `std::fmt::Display` pour `TriggerError`
- [x] `[IMPL]` Implémenter `std::error::Error` pour `TriggerError`
- [x] `[TEST-U]` **Test unitaire :** Chaque variante produit un message `Display` non vide et distinct
- [x] `[TEST-U]` **Test unitaire :** `TriggerError` est `Send + Sync`

### P1.2 — Configuration

- [x] `[IMPL]` Créer `src/trigger/config.rs`
- [x] `[IMPL]` Définir la struct `TriggerConfig` avec les champs :
  - `score_threshold: f32` — seuil par score individuel
  - `vote_threshold: usize` — nombre de votes positifs requis dans la fenêtre
  - `window_size: usize` — taille de la fenêtre glissante (en nombre d'inférences)
  - `cooldown_ms: u64` — délai minimal entre deux détections
  - `socket_path: String` — chemin du socket IPC
- [x] `[IMPL]` Implémenter `Default` : `score_threshold=0.80`, `vote_threshold=3`, `window_size=5`, `cooldown_ms=2000`, `socket_path="/tmp/wakeword_daemon.sock".to_string()`
- [x] `[IMPL]` Implémenter `validate(&self) -> Result<(), TriggerError>` :
  - `score_threshold` dans `(0.0, 1.0]`
  - `vote_threshold` ≤ `window_size`
  - `window_size` > 0
  - `cooldown_ms` > 0
  - `socket_path` non vide
- [x] `[TEST-U]` **Test unitaire :** `TriggerConfig::default().validate()` → `Ok`
- [x] `[TEST-U]` **Test unitaire :** `vote_threshold = 6`, `window_size = 5` → `validate()` → `Err(InvalidConfig)`
- [x] `[TEST-U]` **Test unitaire :** `score_threshold = 0.0` → `Err(InvalidConfig)`
- [x] `[TEST-U]` **Test unitaire :** `score_threshold = 1.5` → `Err(InvalidConfig)`
- [x] `[TEST-U]` **Test unitaire :** `window_size = 0` → `Err(InvalidConfig)`
- [x] `[TEST-U]` **Test unitaire :** `socket_path = ""` → `Err(InvalidConfig)`

---

## PARTIE 2 — Moteur de vote glissant (`TriggerEngine`)

> **Objectif :** Implémenter et valider de manière exhaustive l'algorithme anti-faux-positifs.
> C'est le cœur du module — toutes les propriétés doivent être couvertes par des tests unitaires.

---

### P2.1 — Structure et constructeur

- [ ] `[IMPL]` Créer `src/trigger/engine.rs`
- [ ] `[IMPL]` Définir la struct `TriggerEngine` :
  ```
  history: VecDeque<f32>
  window_size: usize
  score_threshold: f32
  vote_threshold: usize
  cooldown_ms: u64
  last_trigger: Instant
  ```
- [ ] `[IMPL]` Implémenter `TriggerEngine::new(config: &TriggerConfig) -> Self` — initialise avec `VecDeque::with_capacity(window_size)` et `last_trigger = Instant::now() - Duration::from_millis(cooldown_ms + 1)` (cooldown déjà écoulé au démarrage)

### P2.2 — Méthode principale `push`

- [ ] `[IMPL]` Implémenter `TriggerEngine::push(&mut self, score: f32) -> bool` :
  - [ ] `[IMPL]` Ajouter `score` à `history`
  - [ ] `[IMPL]` Si `history.len() > window_size` : supprimer le plus ancien (`pop_front`)
  - [ ] `[IMPL]` Vérifier le cooldown : si `last_trigger.elapsed().as_millis() < cooldown_ms as u128` → retourner `false` immédiatement
  - [ ] `[IMPL]` Compter les votes positifs : `history.iter().filter(|&&s| s > score_threshold).count()`
  - [ ] `[IMPL]` Si `votes >= vote_threshold` : poser `last_trigger = Instant::now()`, appeler `history.clear()`, retourner `true`
  - [ ] `[IMPL]` Sinon : retourner `false`

### P2.3 — Méthodes utilitaires

- [ ] `[IMPL]` Implémenter `TriggerEngine::reset(&mut self)` — vide `history`, remet `last_trigger` à `Instant::now() - Duration::from_millis(cooldown_ms + 1)`
- [ ] `[IMPL]` Implémenter `TriggerEngine::pending_votes(&self) -> usize` — compte les votes positifs courants dans la fenêtre
- [ ] `[IMPL]` Implémenter `TriggerEngine::history_len(&self) -> usize` — retourne `history.len()`
- [ ] `[IMPL]` Implémenter `TriggerEngine::cooldown_remaining_ms(&self) -> u64` — retourne le temps restant avant la fin du cooldown (0 si expiré)

### P2.4 — Tests du moteur de vote

#### Cas nominaux

- [ ] `[TEST-U]` **Détection nominale :** Pousser 3 scores > 0.80 en 5 appels → `push` retourne `true` au 3e vote positif
- [ ] `[TEST-U]` **Pas de détection :** Pousser 2 scores > 0.80 et 3 < 0.80 → jamais `true`
- [ ] `[TEST-U]` **Seuil exact :** Score = 0.80 exactement → ne compte **pas** comme vote positif (filtre strict `>`)
- [ ] `[TEST-U]` **Seuil dépassé :** Score = 0.801 → compte comme vote positif

#### Fenêtre glissante

- [ ] `[TEST-U]` **Glissement :** Remplir 5 scores positifs, ajouter un 6e score → vérifier que `history_len()` reste ≤ 5
- [ ] `[TEST-U]` **Éviction :** Pousser [0.9, 0.9, 0.9, 0.1, 0.1] (3 positifs → détection), puis immédiatement [0.9, 0.9, 0.9] → cooldown empêche le second déclenchement
- [ ] `[TEST-U]` **Fenêtre vide :** Moteur neuf → `push(0.9)` × 1 → pas de détection (1 vote < 3 requis)

#### Cooldown

- [ ] `[TEST-U]` **Cooldown actif :** Détection → immédiatement pousser 3 scores > 0.80 → `false` (cooldown non expiré)
- [ ] `[TEST-U]` **Cooldown expiré :** Après initialisation avec cooldown déjà écoulé → première détection possible immédiatement
- [ ] `[TEST-U]` **`cooldown_remaining_ms` :** Juste après détection → valeur ≈ `cooldown_ms` (±50 ms)
- [ ] `[TEST-U]` **`cooldown_remaining_ms` :** Bien avant toute détection → 0

#### Comportement post-détection

- [ ] `[TEST-U]` **Clear après détection :** Après `true`, `history_len()` == 0
- [ ] `[TEST-U]` **Reset :** Après `reset()`, `pending_votes()` == 0 et `history_len()` == 0
- [ ] `[TEST-U]` **Détection après reset :** `reset()` puis 3 scores > 0.80 → détection possible

#### Configurations limites

- [ ] `[TEST-U]` **window_size = 1, vote_threshold = 1 :** Un seul score > 0.80 → détection immédiate
- [ ] `[TEST-U]` **vote_threshold = window_size :** Tous les scores de la fenêtre doivent être positifs
- [ ] `[TEST-U]` **score_threshold = 0.99 :** Seuls les scores très proches de 1.0 comptent

---

## PARTIE 3 — Notifier IPC (`IpcNotifier`)

> **Objectif :** Implémenter l'envoi de notification sur le Unix Domain Socket de manière robuste et silencieuse en l'absence de client.

---

### P3.1 — Structure et constructeur

- [ ] `[IMPL]` Créer `src/trigger/ipc.rs`
- [ ] `[IMPL]` Définir la struct `IpcNotifier { socket_path: String }`
- [ ] `[IMPL]` Implémenter `IpcNotifier::new(socket_path: String) -> Self`

### P3.2 — Envoi de notification

- [ ] `[IMPL]` Implémenter `IpcNotifier::notify(&self) -> Result<(), TriggerError>` :
  - [ ] `[IMPL]` Tenter `UnixStream::connect(&self.socket_path)`
  - [ ] `[IMPL]` Si succès : écrire `b"WAKEWORD_DETECTED\n"` via `write_all`
  - [ ] `[IMPL]` Si échec de connexion (aucun client) : logger un `tracing::debug!` et retourner `Ok(())` — **pas d'erreur fatale**
  - [ ] `[IMPL]` Si échec d'écriture après connexion réussie : retourner `Err(IpcSendFailed(...))`
- [ ] `[IMPL]` Implémenter `IpcNotifier::notify_with_payload(&self, payload: &[u8]) -> Result<(), TriggerError>` — variante permettant un message customisé (pour les tests et extensions futures)

### P3.3 — Tests du notifier

- [ ] `[TEST-I]` **Test d'intégration — aucun client :** Appeler `notify()` sans aucun listener sur le socket → retourne `Ok(())` sans panique ni log d'erreur fatale
- [ ] `[TEST-I]` **Test d'intégration — client présent :** Ouvrir un `UnixListener` dans le test, appeler `notify()`, vérifier que le listener reçoit `"WAKEWORD_DETECTED\n"`
- [ ] `[TEST-I]` **Test d'intégration — message complet :** Vérifier que le message reçu est exactement `b"WAKEWORD_DETECTED\n"` (longueur, contenu, pas de troncature)
- [ ] `[TEST-I]` **Test d'intégration — notifications multiples :** Appeler `notify()` 5 fois successives → 5 messages reçus côté listener
- [ ] `[TEST-U]` **Test unitaire :** `IpcNotifier::new("/tmp/test.sock")` crée l'instance sans erreur (pas de connexion dans `new`)

---

## PARTIE 4 — Thread trigger (`TriggerRunner`)

> **Objectif :** Implémenter le thread qui orchestre le moteur de vote et le notifier IPC.

---

### P4.1 — Structure du runner

- [ ] `[IMPL]` Créer `src/trigger/runner.rs`
- [ ] `[IMPL]` Définir la struct `TriggerRunner` :
  ```
  engine: TriggerEngine,
  notifier: IpcNotifier,
  running: Arc<AtomicBool>,
  thread_handle: Option<JoinHandle<()>>,
  ```
- [ ] `[IMPL]` Implémenter `TriggerRunner::new(config: &TriggerConfig) -> Self`

### P4.2 — Boucle principale

- [ ] `[IMPL]` Implémenter `TriggerRunner::start(rx: Receiver<f32>) -> Result<(), TriggerError>` :
  - [ ] `[IMPL]` Spawner un thread qui boucle sur `rx.recv()` (bloquant — zéro polling, zéro CPU en idle)
  - [ ] `[IMPL]` Pour chaque score reçu : appeler `engine.push(score)`
  - [ ] `[IMPL]` Si `push` retourne `true` : appeler `notifier.notify()`, logger la détection avec `tracing::info!("WAKE-WORD DETECTED — score window: {:?}", ...)`
  - [ ] `[IMPL]` Si `rx.recv()` retourne `Err` (channel fermé) : sortir de la boucle proprement
  - [ ] `[IMPL]` Logger chaque score reçu au niveau `tracing::trace!`
- [ ] `[IMPL]` Implémenter `TriggerRunner::stop()` : poser `running` à false, `join` le thread
- [ ] `[IMPL]` Implémenter `Drop for TriggerRunner` : appelle `stop()` silencieusement

### P4.3 — Tests du runner

- [ ] `[TEST-I]` **Test d'intégration :** Envoyer une séquence de 5 scores dont 3 > 0.80 → vérifier qu'un message est reçu sur un `UnixListener` de test
- [ ] `[TEST-I]` **Test d'intégration :** Envoyer 10 scores tous < 0.80 → vérifier qu'aucun message n'est émis sur le socket
- [ ] `[TEST-I]` **Test d'intégration :** Fermer le `Sender` → le thread se termine proprement, `stop()` ne panic pas
- [ ] `[TEST-I]` **Test d'intégration :** Drop sans `stop()` → zéro thread zombie
- [ ] `[TEST-I]` **Test d'intégration — cooldown :** Déclencher une détection, puis immédiatement envoyer 5 scores positifs → un seul message reçu (cooldown actif)
- [ ] `[TEST-I]` **Test d'intégration — double détection :** Déclencher, attendre `cooldown_ms + 100` ms (via `std::thread::sleep` dans le test), envoyer 3 scores positifs → second message reçu

---

## PARTIE 5 — Façade publique `TriggerModule`

> **Objectif :** Exposer une API unifiée et stable, seule surface visible depuis le daemon.

---

### P5.1 — Struct `TriggerModule`

- [ ] `[IMPL]` Créer ou compléter `src/trigger/mod.rs`
- [ ] `[IMPL]` Définir `pub struct TriggerModule { runner: TriggerRunner, config: TriggerConfig }`
- [ ] `[IMPL]` Implémenter `TriggerModule::new(config: TriggerConfig) -> Result<Self, TriggerError>` — valide la config, crée `TriggerRunner`
- [ ] `[IMPL]` Implémenter `TriggerModule::start(rx: Receiver<f32>) -> Result<(), TriggerError>`
- [ ] `[IMPL]` Implémenter `TriggerModule::stop() -> Result<(), TriggerError>`
- [ ] `[IMPL]` Implémenter `Drop for TriggerModule` — appelle `stop()` silencieusement

### P5.2 — Tests de la façade

- [ ] `[TEST-I]` **Test d'intégration :** Cycle complet `new → start → séquence de scores → stop` — détection correcte, socket notifié
- [ ] `[TEST-I]` **Test d'intégration :** Deux cycles `start/stop` consécutifs — idempotence, zéro panique
- [ ] `[TEST-I]` **Test d'intégration :** Drop sans stop → propre

### P5.3 — Mode standalone

- [ ] `[IMPL]` Créer `examples/standalone_trigger.rs` (feature `standalone`)
- [ ] `[IMPL]` L'exemple génère une séquence de scores synthétiques (vague de scores positifs, puis négatifs, puis de nouveau positifs après 3 s), instancie `TriggerModule`, log les détections et les messages socket reçus sur un listener local
- [ ] `[TEST-I]` **Test :** `cargo run --example standalone_trigger --features standalone` — s'exécute sans erreur
- [ ] `[VALID]` **Validation manuelle :** Dans un autre terminal, `nc -U /tmp/wakeword_daemon.sock` (ou le path de test) — vérifier la réception de `"WAKEWORD_DETECTED\n"` au bon moment

---

## PARTIE 6 — Pipeline complet end-to-end (intégration workspace)

> **Objectif :** Valider la chaîne complète `inference_ml → trigger → socket` au niveau du workspace.

---

### P6.1 — Test d'intégration workspace

- [ ] `[TEST-I]` **Test workspace :** Dans `integration_test`, brancher `inference_ml` (mode `mock_model`) → `trigger` → `UnixListener` de test — envoyer des matrices MFCC qui déclenchent un score > 0.80, vérifier la réception du message socket de bout en bout
- [ ] `[TEST-I]` **Test workspace :** Vérifier la latence bout-en-bout : timestamp à l'envoi de la matrice MFCC → timestamp à la réception du message socket — doit être < 150 ms
- [ ] `[TEST-I]` **Test workspace :** Vérifier qu'aucune fuite mémoire n'est introduite par l'intégration (AddressSanitizer sur la suite d'intégration)

### P6.2 — Validation des métriques cibles

- [ ] `[VALID]` **Faux positifs :** Envoyer 8 heures de scores simulés correspondant à du bruit de fond (scores uniformément distribués entre 0.0 et 0.7) → vérifier que le nombre de déclenchements est 0 (aucun bruit ne dépasse 0.80)
- [ ] `[VALID]` **Faux négatifs :** Envoyer 500 séquences simulant une bonne prononciation (3 scores > 0.80 sur 5) → vérifier que 100 % déclenchent (taux de faux négatifs = 0 % sur signal synthétique idéal)
- [ ] `[VALID]` **Latence IPC :** Mesurer le temps entre `notifier.notify()` et la réception côté `UnixListener` — doit être < 5 ms

---

## PARTIE 7 — Suite de tests complète & régression

> **Objectif :** Consolider tous les tests en une suite reproductible, garantir la non-régression.

---

### P7.1 — Organisation des tests

- [ ] `[SETUP]` Créer `tests/trigger_integration.rs` — tests d'intégration du crate complet
- [ ] `[SETUP]` Utiliser des paths de socket uniques par test (`/tmp/wakeword_test_{uuid}.sock`) pour éviter les conflits entre tests parallèles
- [ ] `[SETUP]` Nettoyer les fichiers socket dans `teardown` de chaque test (`std::fs::remove_file`)

### P7.2 — Tests unitaires de régression

- [ ] `[TEST-U]` **Régression :** `TriggerConfig::default().validate()` → `Ok`
- [ ] `[TEST-U]` **Régression :** Toutes les variantes de `TriggerError` ont un `Display` non vide
- [ ] `[TEST-U]` **Régression :** 3 votes positifs sur 5 → détection (configuration nominale)
- [ ] `[TEST-U]` **Régression :** 2 votes positifs sur 5 → pas de détection
- [ ] `[TEST-U]` **Régression :** Cooldown bloque le second déclenchement immédiat
- [ ] `[TEST-U]` **Régression :** `history.clear()` après détection
- [ ] `[TEST-U]` **Régression :** `reset()` remet l'état à zéro

### P7.3 — Tests d'intégration de régression

- [ ] `[TEST-I]` **Régression :** `notify()` sans client → `Ok` (silence gracieux)
- [ ] `[TEST-I]` **Régression :** `notify()` avec client → message `"WAKEWORD_DETECTED\n"` reçu
- [ ] `[TEST-I]` **Régression :** Thread runner → fermeture channel → terminaison propre
- [ ] `[TEST-I]` **Régression :** Drop sans stop → zéro thread zombie
- [ ] `[TEST-I]` **Régression :** Deux détections séparées par cooldown → deux messages socket

### P7.4 — Tests de performance

- [ ] `[TEST-P]` **Latence `push` :** Mesurer le temps d'un appel `TriggerEngine::push` — doit être < 1 µs (pas de contention, pas d'I/O dans le moteur seul)
- [ ] `[TEST-P]` **Latence `notify` :** Mesurer le temps d'un appel `IpcNotifier::notify` avec client présent — doit être < 5 ms
- [ ] `[VALID]` **CPU idle :** Thread trigger bloqué sur `recv()` sans scores entrants pendant 60 s — `Instruments → Energy Log` confirme CPU ≈ 0 %

---

## PARTIE 8 — Documentation & Livraison du module

> **Objectif :** Module documenté, propre, prêt à être intégré dans le daemon final.

---

### P8.1 — Documentation

- [ ] `[SETUP]` Ajouter doc-comments `///` sur tous les types et fonctions publics : `TriggerModule`, `TriggerConfig`, `TriggerError`, `TriggerEngine`, `IpcNotifier`
- [ ] `[SETUP]` Écrire un doc-example dans `TriggerModule::new` montrant le cycle minimal `new/start/stop`
- [ ] `[SETUP]` Documenter dans le doc-comment de `TriggerEngine::push` la sémantique exacte du vote (seuil strict `>`, cooldown avant vote, clear après détection)
- [ ] `[TEST-U]` **Test :** `cargo doc --no-deps -p trigger` — sans erreur ni warning

### P8.2 — Validation finale

- [ ] `[VALID]` `cargo clippy -p trigger -- -D warnings` — zéro warning
- [ ] `[VALID]` `cargo fmt --check -p trigger` — code formaté
- [ ] `[VALID]` `cargo test -p trigger` — suite complète verte
- [ ] `[VALID]` `cargo test -p trigger --features mock_scores` — suite mock verte
- [ ] `[VALID]` `cargo build -p trigger --release` — compile proprement

### P8.3 — Intégration dans le workspace et le daemon

- [ ] `[SETUP]` Vérifier que `trigger` est bien dans le `[workspace]` racine
- [ ] `[SETUP]` Documenter dans `trigger/README.md` : sémantique du vote, paramètres conseillés, format du message socket, comment se connecter côté client (exemple `nc -U`)
- [ ] `[TEST-I]` **Test d'intégration finale workspace :** Chaîne complète `audio_capture (mock) → pipeline_dsp → inference_ml (mock) → trigger → UnixListener` — vérifier la réception d'un message socket sur un signal synthétique déclenchant

---

## Récapitulatif par partie

| Partie | Thème | Dépend de |
|---|---|---|
| P0 | Installation & Setup | — |
| P1 | Erreurs & Config | P0 |
| P2 | Moteur de vote glissant `TriggerEngine` | P1 |
| P3 | Notifier IPC `IpcNotifier` | P1 |
| P4 | Thread trigger `TriggerRunner` | P2, P3 |
| P5 | Façade publique `TriggerModule` | P4 |
| P6 | Intégration workspace end-to-end | P5 |
| P7 | Suite de tests complète & régression | P1→P5 |
| P8 | Documentation & Livraison | P7 |
