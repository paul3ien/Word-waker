# Stack Technique — Module `trigger`

> Ce fichier documente les technologies, dépendances et contraintes propres au module de déclenchement et IPC.
> Le module reçoit des scores `f32` depuis `inference_ml`, applique un vote glissant anti-faux-positifs,
> et notifie les clients via Unix Domain Socket lorsqu'un wake-word est détecté.
> Il doit être utilisable de manière **autonome** (scores simulés en entrée) et **intégré** dans le pipeline complet.

---

## Architecture de l'étage

```
inference_ml  →  Sender<f32>  →  TriggerEngine (vote glissant)
                                        ↓ wake-word détecté
                               ipc_notify()  →  Unix Domain Socket
                                        ↓
                               /tmp/wakeword_daemon.sock  →  App cliente
```

> **Pas de timer actif, pas de polling.** Le thread trigger bloque sur `rx.recv()`.
> Le cooldown est vérifié sur `Instant::elapsed()` à chaque score reçu — zéro thread supplémentaire.

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
| Rust | 1.78+ | 🔴 CRITIQUE | Langage principal — `std::os::unix::net`, `VecDeque`, `Instant` |
| Cargo | (bundled) | 🔴 CRITIQUE | Build system |
| Target `aarch64-apple-darwin` | macOS 14+ | 🔴 CRITIQUE | Cible exclusive — Unix Domain Socket POSIX natif |
| Edition Rust 2021 | — | 🟠 OBLIGATOIRE | Resolver v2 |
| `rustfmt` | (stable) | 🟡 IMPORTANT | Formatage |
| `clippy` | (stable) | 🟡 IMPORTANT | Linting |

---

## 2. Mécanisme de vote glissant (anti-faux-positifs)

> C'est le cœur algorithmique du module. Tous les paramètres doivent être configurables.

| Composant | Valeur nominale | Niveau | Justification |
|---|---|---|---|
| `VecDeque<f32>` (historique des scores) | — | 🔴 CRITIQUE | Structure FIFO — permet le glissement de la fenêtre sans allocation |
| `window_size` | 5 inférences | 🔴 CRITIQUE | ≈ 500 ms de fenêtre d'observation à 100 ms/inférence |
| `score_threshold` | 0.80 | 🔴 CRITIQUE | Seuil individuel — un score doit dépasser 0.80 pour compter comme vote positif |
| `vote_threshold` | 3 sur 5 | 🔴 CRITIQUE | Majorité nécessaire — évite les faux positifs sur un pic isolé |
| `cooldown_ms` | 2 000 ms | 🔴 CRITIQUE | Délai minimal entre deux détections — évite les déclenchements répétés sur le même mot |
| `last_trigger: Instant` | — | 🔴 CRITIQUE | Référence temporelle pour le cooldown |
| `history.clear()` après détection | — | 🟠 OBLIGATOIRE | Réinitialise la fenêtre après un déclenchement pour éviter les cascades |
| Vérification cooldown avant le vote | — | 🟠 OBLIGATOIRE | Ordre d'évaluation : cooldown d'abord, votes ensuite |

---

## 3. Communication Inter-Processus (IPC)

### 3.1 Mécanisme retenu : Unix Domain Socket

| Technologie | Niveau | Justification |
|---|---|---|
| `std::os::unix::net::UnixStream` | 🔴 CRITIQUE | Connexion cliente vers le socket daemon — natif Rust std, zéro dépendance |
| `std::os::unix::net::UnixListener` | 🔴 CRITIQUE | Écoute entrante côté daemon (optionnel selon architecture) |
| `const SOCKET_PATH: &str = "/tmp/wakeword_daemon.sock"` | 🔴 CRITIQUE | Chemin convenu entre le daemon et ses clients |
| Message `"WAKEWORD_DETECTED\n"` | 🔴 CRITIQUE | Message ASCII simple — facile à parser côté client quel que soit le langage |
| `std::io::Write::write_all` | 🔴 CRITIQUE | Envoi atomique du message |
| Ignorance silencieuse si aucun client | 🟠 OBLIGATOIRE | Si `UnixStream::connect` échoue → log warning, pas d'erreur fatale |
| Chemin configurable | 🟠 OBLIGATOIRE | `SOCKET_PATH` dans la config, pas hardcodé |

### 3.2 Mécanismes alternatifs (non retenus pour ce projet)

| Mécanisme | Niveau | Raison du non-choix |
|---|---|---|
| Named Pipe (FIFO) | 🟢 OPTIONNEL | Lecture bloquante côté client — moins flexible |
| Signal POSIX (`kill(pid, SIGUSR1)`) | 🟢 OPTIONNEL | Ultra-léger mais binaire — pas de payload possible |
| D-Bus (via `dbus-rs`) | 🟢 OPTIONNEL | Overkill pour ce cas d'usage |

---

## 4. Intégration dans le daemon macOS

| Technologie | Niveau | Justification |
|---|---|---|
| `daemonize` crate | 🟠 OBLIGATOIRE | Daemonisation POSIX : `fork`, `setsid`, redirection I/O — utilisé dans le `main` du daemon (hors scope strict du module) |
| LaunchAgent `.plist` | 🟡 IMPORTANT | Démarrage automatique du daemon au login utilisateur |
| `codesign` (Developer ID) | 🟡 IMPORTANT | Signature du binaire pour distribution hors App Store |
| `tracing` + `os_log` | 🟡 IMPORTANT | Intégration avec `Console.app` macOS pour le monitoring |

---

## 5. Dépendances Cargo (module `trigger`)

| Crate | Version | Niveau | Rôle |
|---|---|---|---|
| `crossbeam-channel` | 0.5+ | 🔴 CRITIQUE | Réception des scores depuis `inference_ml` |
| `anyhow` | 1.0+ | 🟠 OBLIGATOIRE | Propagation d'erreurs pour l'IPC et l'initialisation |
| `tracing` | 0.1+ | 🟡 IMPORTANT | Logs structurés : score reçu, vote, détection, cooldown |
| `tracing-subscriber` | 0.3+ | 🟡 IMPORTANT | Backend de logs pour les tests et le daemon |

> **Aucune dépendance externe système** : `std::os::unix::net` est dans la stdlib Rust — pas de `build.rs` requis pour ce module.

---

## 6. Structure du module (organisation des fichiers)

| Fichier | Niveau | Rôle |
|---|---|---|
| `src/trigger/mod.rs` | 🔴 CRITIQUE | Point d'entrée public, re-exports, struct `TriggerModule` |
| `src/trigger/error.rs` | 🔴 CRITIQUE | `TriggerError` : `ChannelClosed`, `IpcSendFailed(String)`, `InvalidConfig` |
| `src/trigger/config.rs` | 🔴 CRITIQUE | `TriggerConfig` : tous les paramètres configurables |
| `src/trigger/engine.rs` | 🔴 CRITIQUE | `TriggerEngine` : vote glissant, cooldown, logique de détection |
| `src/trigger/ipc.rs` | 🔴 CRITIQUE | `IpcNotifier` : envoi du message sur le socket |
| `src/trigger/runner.rs` | 🟠 OBLIGATOIRE | Thread trigger : reçoit scores, passe au moteur, notifie IPC |
| `tests/trigger_integration.rs` | 🟡 IMPORTANT | Tests d'intégration isolés |

---

## 7. Interface publique du module (contrat)

| Symbole | Niveau | Description |
|---|---|---|
| `TriggerModule::new(config: TriggerConfig) -> Result<Self, TriggerError>` | 🔴 CRITIQUE | Initialise le moteur de vote et le notifier IPC |
| `TriggerModule::start(rx: Receiver<f32>) -> Result<(), TriggerError>` | 🔴 CRITIQUE | Démarre le thread trigger |
| `TriggerModule::stop()` | 🔴 CRITIQUE | Arrête proprement le thread |
| `TriggerEngine::push(score: f32) -> bool` | 🔴 CRITIQUE | Ajoute un score, retourne `true` si wake-word détecté |
| `TriggerEngine::reset()` | 🟠 OBLIGATOIRE | Remet à zéro l'historique et le cooldown |
| `TriggerConfig` (struct) | 🔴 CRITIQUE | `score_threshold`, `vote_threshold`, `window_size`, `cooldown_ms`, `socket_path` |
| `TriggerError` (enum) | 🔴 CRITIQUE | Erreurs typées |
| `Drop for TriggerModule` | 🟠 OBLIGATOIRE | `stop()` automatique si non appelé manuellement |

---

## 8. Contraintes de qualité & métriques du module

| Métrique | Objectif | Niveau |
|---|---|---|
| Taux de faux positifs | < 1 %/heure sur corpus audio négatif 8 h | 🔴 CRITIQUE |
| Taux de faux négatifs | < 5 % sur 500 prononciations test | 🔴 CRITIQUE |
| Latence de détection (score reçu → socket envoyé) | < 5 ms | 🔴 CRITIQUE |
| Latence bout-en-bout (audio → socket) | < 150 ms | 🟠 OBLIGATOIRE |
| Consommation CPU du thread trigger | < 0,01 % | 🟠 OBLIGATOIRE |
| Zéro perte de score si aucun client IPC connecté | Silence gracieux | 🔴 CRITIQUE |
| Compilable et testable sans `inference_ml` ni daemon | Obligatoire | 🔴 CRITIQUE |

---

## 9. Outillage de test & profiling

| Outil | Niveau | Usage |
|---|---|---|
| `cargo test` | 🔴 CRITIQUE | Tests unitaires et d'intégration |
| `cargo test --features mock_scores` | 🟠 OBLIGATOIRE | Tests avec séquences de scores injectées directement (sans inference_ml) |
| Outil POSIX `nc -U /tmp/wakeword_daemon.sock` | 🟡 IMPORTANT | Client minimal pour validation manuelle du socket |
| `tracing-subscriber` en mode test | 🟡 IMPORTANT | Capturer les logs pendant les tests pour débogage |
| `Instruments.app → Energy Log` | 🟡 IMPORTANT | Vérifier que le thread trigger ne consomme pas d'énergie en idle |
| `cargo-flamegraph` | 🟢 OPTIONNEL | Vérifier qu'il n'y a pas de hotspot inattendu dans le vote |

---

## 10. Contraintes d'intégration dans le pipeline complet

| Contrainte | Niveau | Description |
|---|---|---|
| Interface entrée : `Receiver<f32>` | 🔴 CRITIQUE | Reçoit les scores depuis `inference_ml` |
| Interface sortie : Unix Domain Socket | 🔴 CRITIQUE | Notifie les clients via `/tmp/wakeword_daemon.sock` (chemin configurable) |
| Pas de dépendance vers `audio_capture`, `pipeline_dsp`, `inference_ml` | 🔴 CRITIQUE | Découplage strict — ne connaît que le `f32` en entrée |
| Un seul thread dédié, bloquant sur `recv()` | 🟠 OBLIGATOIRE | Zéro polling actif — consommation CPU nulle en idle |
| Compatibilité avec le daemon POSIX (semaine 5) | 🟡 IMPORTANT | Le module est instancié depuis le `main` du daemon, pas de contrainte interne supplémentaire |
| Le socket est recréé au démarrage si déjà présent | 🟠 OBLIGATOIRE | `std::fs::remove_file(SOCKET_PATH)` avant `bind()` pour éviter `AddrInUse` sur redémarrage |
