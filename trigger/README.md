# trigger

Module de déclenchement wake-word avec vote glissant anti-faux-positifs et notification via Unix Domain Socket.

## Architecture

```
inference_ml  →  Sender<f32>  →  TriggerEngine (vote glissant)
                                        ↓ wake-word détecté
                               IpcNotifier  →  Unix Domain Socket
                                        ↓
                /tmp/wakeword_daemon.sock  →  Application cliente
```

## Fonctionnement du vote glissant

Le moteur `TriggerEngine` maintient une fenêtre FIFO des derniers scores reçus :

| Paramètre | Valeur nominale | Description |
|---|---|---|
| `score_threshold` | `0.80` | Un score doit être **strictement supérieur** à cette valeur pour compter comme vote positif |
| `vote_threshold` | `3` | Nombre de votes positifs requis dans la fenêtre pour déclencher |
| `window_size` | `5` | Taille de la fenêtre glissante (en nombre d'inférences) |
| `cooldown_ms` | `2000` | Délai minimal entre deux détections (ms) |

**Ordre d'évaluation de `push(score)` :**
1. Ajout du score dans la fenêtre (éviction du plus ancien si pleine)
2. Vérification du cooldown — retourne `false` immédiatement si actif
3. Comptage des votes positifs (filtre strict `>`)
4. Si `votes >= vote_threshold` : réinitialise la fenêtre, met à jour le cooldown, retourne `true`

## Message socket

Quand un wake-word est détecté, le daemon envoie :

```
WAKEWORD_DETECTED\n
```

(18 octets ASCII, terminé par `\n`)

## Connexion côté client

```bash
# Écouter les notifications en temps réel
nc -U /tmp/wakeword_daemon.sock

# Ou avec socat
socat - UNIX-CONNECT:/tmp/wakeword_daemon.sock
```

## Utilisation depuis le daemon

```rust
use trigger::{TriggerConfig, TriggerModule};

let config = TriggerConfig {
    socket_path: "/tmp/wakeword_daemon.sock".to_string(),
    score_threshold: 0.80,
    vote_threshold: 3,
    window_size: 5,
    cooldown_ms: 2000,
};

let mut module = TriggerModule::new(config)?;
let (tx, rx) = crossbeam_channel::unbounded();

// Démarrer le thread trigger
module.start(rx)?;

// Envoyer des scores depuis inference_ml
tx.send(score)?;

// Arrêt propre
module.stop()?;
```

## Exécuter l'exemple standalone

```bash
cargo run --example standalone_trigger --features standalone -p trigger
```

## Tests

```bash
# Suite complète
cargo test -p trigger

# Avec scores mockés
cargo test -p trigger --features mock_scores

# Build release
cargo build -p trigger --release
```

## Contraintes de performance

| Métrique | Objectif |
|---|---|
| Latence `push` (moteur seul) | < 1 µs |
| Latence `notify` (IPC) | < 5 ms |
| Latence bout-en-bout (audio → socket) | < 150 ms |
| CPU en idle (thread bloqué sur `recv()`) | ≈ 0 % |

## Structure du module

```
src/trigger/
├── mod.rs       — TriggerModule (façade publique)
├── config.rs    — TriggerConfig
├── engine.rs    — TriggerEngine (vote glissant)
├── ipc.rs       — IpcNotifier (Unix Domain Socket)
├── error.rs     — TriggerError
└── runner.rs    — TriggerRunner (thread)
tests/
└── trigger_integration.rs  — suite de régression & performance
examples/
└── standalone_trigger.rs   — démo autonome
```
