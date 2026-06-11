# Stack Technique — Crate `ui`

> Ce fichier documente les technologies, dépendances et contraintes propres à l'application graphique (UI) de Word Waker.
> L'UI est une application macOS native qui se connecte au daemon via Unix Domain Socket,
> affiche l'état du daemon, les détections en temps réel, et permet de configurer le pipeline à chaud.
> Elle constitue le second point d'entrée exécutable du workspace (aux côtés de `daemon`).

---

## Architecture de l'UI

```
┌─────────────────────────────────────────────────────────────┐
│                       ui (Application)                       │
│                                                              │
│  ┌─────────────────┐     ┌──────────────────────────────┐   │
│  │   Menu Bar App  │────▶│      Panel flottant           │   │
│  │   (NSStatusBar) │     │  ┌────────────────────────┐  │   │
│  │   Icône + état  │     │  │ • Statut daemon         │  │   │
│  │                 │     │  │ • Compteur détections   │  │   │
│  │  ● Connexion    │     │  │ • Historique récent     │  │   │
│  │  ● Déconnecté   │     │  │ • Réglages (seuil,     │  │   │
│  │                 │     │  │   cooldown, modèle)     │  │   │
│  └────────┬────────┘     │  │ • Bouton Quitter        │  │   │
│           │              │  └────────────────────────┘  │   │
│           ▼              └──────────────┬───────────────┘   │
│  ┌─────────────────┐                    │                    │
│  │  IpcClient       │  Unix Domain      │                    │
│  │  (socket client) │◄══════════════════╣                    │
│  │                  │  /tmp/wakeword    │                    │
│  │  Reçoit les      │  _daemon.sock     │                    │
│  │  notifications   │                   │                    │
│  └─────────────────┘                   │                    │
└─────────────────────────────────────────┼────────────────────┘
                                          │
                   ┌──────────────────────┘
                   ▼
            ┌──────────────┐
            │    daemon    │
            │  (word-waker)│
            └──────────────┘
```

> **L'UI n'est qu'un client du daemon.** Elle lit le socket IPC pour recevoir
> les notifications `WAKEWORD_DETECTED\n` et les affiche. Elle ne contient
> aucun traitement audio, DSP ou inférence.

---

## Légende des niveaux d'importance

| Niveau | Signification |
|--------|---------------|
| 🔴 CRITIQUE | Bloquant — l'UI ne peut pas fonctionner sans |
| 🟠 OBLIGATOIRE | Requis pour respecter les contraintes de qualité et de sécurité |
| 🟡 IMPORTANT | Fortement recommandé, contournement possible à court terme uniquement |
| 🟢 OPTIONNEL | Amélioration ou outillage, non bloquant |

---

## 1. Langage & Toolchain

| Technologie | Version min | Niveau | Justification |
|---|---|---|---|
| Rust | 1.78+ | 🔴 CRITIQUE | Langage principal |
| Cargo | (bundled) | 🔴 CRITIQUE | Build system |
| Target `aarch64-apple-darwin` | macOS 14+ | 🔴 CRITIQUE | Cible exclusive — icrate, objc2, AppKit, Foundation, Unix Domain Socket |
| Edition Rust 2021 | — | 🟠 OBLIGATOIRE | Resolver v2 |

---

## 2. Choix du framework UI : `icrate` + `objc2` (AppKit natif)

### Pourquoi ne pas utiliser Tauri, egui, slint, ou Flutter ?

| Approche | Rejetée parce que... |
|---|---|
| **Tauri** | WebView + JS bridge = overhead mémoire (>50 Mo), latence, packaging complexe. Le projet est un daemon temps réel léger (<5 Mo). Tauri n'apporte rien ici. |
| **egui** | Excellente pour du debug, mais pas native macOS : pas de menu bar, pas de NSStatusBar, pas d'intégration LaunchAgent. Rendu OpenGL/Metal non nécessaire. |
| **Slint** | Framework intéressant mais encore jeune sur macOS, pas de NSStatusBar natif, licence GPLv3 problématique. |
| **Flutter** | Overkill monumental : Dart VM, moteur Skia, binaire >50 Mo pour 3 widgets. |
| **Swift/SwiftUI** | Excellent mais hors workspace Rust, nécessite un second build system (Xcode), interop complexe via C FFI. |

### Choix retenu : `icrate` (bindings Rust idiomatiques vers AppKit/Foundation)

| Critère | Évaluation |
|---|---|
| **Bindings natifs** | Accès direct à `NSApplication`, `NSStatusBar`, `NSMenu`, `NSWindow`, `NSTextField`, `NSButton` — 100% natif |
| **Zéro overhead** | Pas de VM, pas de WebView, pas de moteur de rendu — juste des appels FFI vers Objective-C |
| **NSStatusBar** | Support complet du menu bar macOS (icône, menu déroulant, state, tooltip) |
| **Taille binaire** | <3 Mo ajoutés (hors binaire du daemon) |
| **Licence** | MIT — compatible avec le projet |
| **Maintenance** | `objc2` maintenu activement, utilisé en production (ex: `warp`, `joshuto`) |

---

## 3. Dépendances Cargo

### 3.1 Crates workspace internes

| Crate | Chemin | Niveau | Rôle dans l'UI |
|---|---|---|---|
| *(aucun)* | — | — | L'UI n'importe **aucun** crate du workspace. Elle est totalement découplée du pipeline DSP/ML. |

### 3.2 Crates externes

| Crate | Version | Niveau | Rôle |
|---|---|---|---|
| `icrate` | 0.1.2+ | 🔴 CRITIQUE | Bindings AppKit (NSApplication, NSStatusBar, NSMenu, NSWindow, NSPanel, NSView) |
| `objc2` | 0.6+ | 🔴 CRITIQUE | Runtime Objective-C (msg_send!, rc::Retained, MainThreadMarker) |
| `objc2-foundation` | 0.3+ | 🔴 CRITIQUE | Types Foundation (NSArray, NSDictionary, NSData, NSRunLoop, NSTimer) |
| `crossbeam-channel` | 0.5+ | 🟠 OBLIGATOIRE | Communication thread UI ↔ thread socket client |
| `anyhow` | 1.0+ | 🟠 OBLIGATOIRE | Propagation d'erreurs |
| `tracing` | 0.1+ | 🟡 IMPORTANT | Logs structurés |
| `tracing-subscriber` | 0.3+ | 🟡 IMPORTANT | Backend console (ou fichier) |

### 3.3 Pas de dépendance à `audio_capture`, `pipeline_dsp`, `inference_ml`, `trigger`

> **Principe fondamental :** L'UI est un client IPC passif. Elle ne fait **pas** de DSP,
> **pas** de CoreML, **pas** de capture audio. Elle se connecte au socket du daemon
> et lit les messages `WAKEWORD_DETECTED\n`. Ce découplage total garantit :
> - Aucun risque de conflit CoreAudio entre l'UI et le daemon
> - L'UI peut tourner même si le daemon est arrêté (état "déconnecté")
> - Tests ultra-simples : on mock un server Unix socket

---

## 4. Architecture logicielle

```
src/
├── main.rs              — Point d'entrée : init, démarrage UI + client socket
├── config.rs            — UiConfig (socket path, refresh interval)
├── error.rs             — UiError (connexion, parse, NSStatusBar)
├── app.rs               — UiApp : NSApplication + NSStatusBar + menu
├── status_item.rs       — MenuBarIcon : icône, tooltip, menu constructeur
├── socket_client.rs     — IpcClient : client Unix Domain Socket non-bloquant
├── delegate.rs          — AppDelegate : callbacks cycle de vie NSApplication
└── panel.rs             — DetectionsPanel : fenêtre flottante avec historique
```

### Flux de données

```
┌──────────────┐  channel crossbeam  ┌──────────────┐
│  IpcClient   │ ──── Sender ──────▶ │  UiApp       │
│  (thread)    │     <Evenement>      │  (main thread)│
│              │                     │              │
│  Recv socket │                     │  Update UI   │
└──────┬───────┘                     └──────┬───────┘
       │ Unix Domain Socket                │ NSStatusBar
       │ (non-bloquant)                    │ NSMenu
       ▼                                   ▼
┌──────────────┐                    ┌──────────────┐
│   daemon     │                    │  Écran       │
└──────────────┘                    └──────────────┘
```

---

## 5. Comportement NSStatusBar

| État | Icône | Tooltip | Menu |
|---|---|---|---|
| Daemon connecté | 🟢 `waveform.circle.fill` (SF Symbol) ou texte "WW" | "Word Waker — en écoute" | Quitter, Préférences, À propos |
| Daemon déconnecté | 🔴 `xmark.circle.fill` (SF Symbol) ou texte "WW ⚠" | "Word Waker — daemon déconnecté" | Quitter, Reconnecter |
| Détection en cours | 🟡 flash bref (500 ms) puis retour au vert | "Mot détecté !" | Compteur incrémenté |

> SF Symbols nécessite `NSImage::imageWithSystemSymbolName_accessibilityDescription_`
> (dispo via `icrate`). Alternative simple : utiliser du texte (emoji ou ASCII `⚫`)
> pour éviter une dépendance graphique supplémentaire.

**Approche recommandée pour le Proof of Concept :** utiliser des emojis dans le titre du `NSStatusItem` :
- `🎤` = en écoute
- `⏸` = déconnecté

C'est **zéro asset graphique**, pur texte, et rendu natif par AppKit.

---

## 6. Topologie des channels UI

| Channel | Type | Capacité | Producteur | Consommateur |
|---|---|---|---|---|
| `(tx_event, rx_event)` | `crossbeam_channel::unbounded<UiEvent>` | unbounded | `IpcClient` (thread) | `UiApp` pulle via NSTimer/NSRunLoop |

> unbounded car les événements UI sont rares (quelques par minute), pas de risque de backlog.

```rust
enum UiEvent {
    Connected,
    Disconnected,
    WakeWordDetected { timestamp: Instant },
    Error(String),
}
```

---

## 7. Client socket (IpcClient)

### Spécifications

| Critère | Valeur |
|---|---|
| Protocole | Unix Domain Socket (`AF_UNIX`, `SOCK_STREAM`) |
| Chemin socket | `/tmp/wakeword_daemon.sock` (configurable) |
| Mode connexion | Non-bloquant + reconnexion automatique |
| Parsing | Lit ligne par ligne (`\n`), vérifie `WAKEWORD_DETECTED` |
| Thread | Dédié (`std::thread::spawn`), boucle `connect → read → reconnect` |
| Timeout reconnexion | 2 secondes entre tentatives si daemon absent |

### Pseudo-code

```rust
fn run_client(tx: Sender<UiEvent>, socket_path: String) {
    loop {
        match UnixStream::connect(&socket_path) {
            Ok(stream) => {
                tx.send(UiEvent::Connected).ok();
                let reader = BufReader::new(stream);
                for line in reader.lines() {
                    match line {
                        Ok(msg) if msg.trim() == "WAKEWORD_DETECTED" => {
                            tx.send(UiEvent::WakeWordDetected { timestamp: Instant::now() }).ok();
                        }
                        Err(_) => break, // connexion fermée
                        _ => {} // ignore les lignes inconnues
                    }
                }
                tx.send(UiEvent::Disconnected).ok();
            }
            Err(_) => {
                tx.send(UiEvent::Disconnected).ok();
                thread::sleep(Duration::from_secs(2));
            }
        }
    }
}
```

---

## 8. Contraintes de qualité

| Métrique | Objectif | Niveau |
|---|---|---|
| CPU idle (UI affichée, daemon OK) | < 0.5 % | 🟠 OBLIGATOIRE |
| Mémoire résidente | < 15 Mo | 🟠 OBLIGATOIRE |
| Taille binaire release (UI seule) | < 3 Mo | 🟡 IMPORTANT |
| Latence détection → affichage UI | < 50 ms | 🟡 IMPORTANT |
| Reconnexion automatique | < 3 s après redémarrage daemon | 🟠 OBLIGATOIRE |
| Zéro crash si daemon killé | Obligatoire | 🔴 CRITIQUE |
| Zéro fuite mémoire NSStatusItem | Obligatoire | 🔴 CRITIQUE |
| Fermeture fenêtre = masquer (pas quitter) | Obligatoire | 🔴 CRITIQUE |
| Arrêt propre via menu Quitter | Obligatoire | 🔴 CRITIQUE |
| Compatible macOS 14+ (Sonoma) | Obligatoire | 🔴 CRITIQUE |
| Compatible macOS 15+ (Sequoia) | Obligatoire | 🔴 CRITIQUE |

---

## 9. Tests

### Stratégie de test

| Niveau | Type | Framework | Description |
|---|---|---|---|
| Unitaire | `cargo test` | Rust built-in | Tests de parsing, `UiConfig`, transitions d'état |
| Intégration | `cargo test` | Mocks | Mock server Unix socket → vérifie `UiEvent` émis |
| Manuel | Checklist | Humain | Lancer l'UI + daemon, vérifier l'affichage, tuer le daemon, vérifier reconnexion |

### Tests automatiques (sans écran, headless)

On mock un serveur Unix Domain Socket qui envoie `WAKEWORD_DETECTED\n` :

```rust
#[test]
fn ipc_client_receives_detection_event() {
    let socket_path = "/tmp/ui_test_mock.sock";
    let _ = std::fs::remove_file(socket_path);

    let listener = UnixListener::bind(socket_path).unwrap();
    let (tx, rx) = crossbeam_channel::unbounded();

    let client_thread = std::thread::spawn(move || {
        IpcClient::run(tx, socket_path.to_string());
    });

    // Accepter la connexion du client
    let (mut stream, _) = listener.accept().unwrap();
    stream.write_all(b"WAKEWORD_DETECTED\n").unwrap();

    let event = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(matches!(event, UiEvent::WakeWordDetected { .. }));

    drop(stream); // fermer → client détecte déconnexion
    let event = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(matches!(event, UiEvent::Disconnected));

    // Nettoyage
    let _ = std::fs::remove_file(socket_path);
}
```

---

## 10. Packaging et distribution (macOS .app bundle)

### Méthode de build recommandée

```bash
# 1. Compiler le binaire UI en release
cargo build --release -p ui --target aarch64-apple-darwin

# 2. Créer un .app bundle manuellement (script shell simple)
mkdir -p WordWaker.app/Contents/MacOS
mkdir -p WordWaker.app/Contents/Resources
cp target/release/word-waker-ui WordWaker.app/Contents/MacOS/
cp ui/assets/AppIcon.icns WordWaker.app/Contents/Resources/

# 3. Créer Info.plist (LSUIElement = true → pas d'icône dans le Dock)
cat > WordWaker.app/Contents/Info.plist << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>           <string>Word Waker</string>
  <key>CFBundleExecutable</key>     <string>word-waker-ui</string>
  <key>CFBundleIdentifier</key>     <string>com.wordwaker.ui</string>
  <key>LSUIElement</key>            <true/>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
EOF

# 4. Lancer
open WordWaker.app
```

> `LSUIElement = true` est crucial : l'application vit uniquement dans la barre de menu,
> sans icône dans le Dock. C'est le comportement attendu d'un agent d'arrière-plan.

### Ou : utiliser `cargo-bundle` (option plus automatisée)

```toml
# Dans ui/Cargo.toml
[package.metadata.bundle]
name = "Word Waker"
identifier = "com.wordwaker.ui"
icon = ["assets/AppIcon.icns"]
category = "Utilities"
osx_minimum_system_version = "14.0"
```

```bash
cargo bundle --release -p ui
# → target/release/bundle/osx/Word Waker.app
```

---

## 11. Intégration avec le reste du projet

### Démarrage recommandé (utilisateur final)

```bash
# 1. Installer le daemon (une fois)
sudo cp target/release/word-waker /usr/local/bin/
launchctl load ~/Library/LaunchAgents/com.wordwaker.daemon.plist

# 2. Lancer l'UI (au login, via "Ouverture" dans Préférences Système)
open /Applications/Word\ Waker.app
```

### Relation UI ↔ Daemon

```
┌─────────┐  Unix Domain Socket  ┌──────────┐
│   UI    │◄═════════════════════│  daemon  │
│ (client)│                      │ (server) │
└─────────┘                      └──────────┘
     │                                 │
     │ Affiche l'état                  │ Capture + DSP + ML
     │ Compteur de détections          │ Émet WAKEWORD_DETECTED
     │ Menu Quitter                    │ SIGINT → arrêt propre
     │                                 │
     └──── Aucun couplage fort ───────┘
```

> L'UI ne lance **pas** le daemon. Le daemon est géré par `launchd` (LaunchAgent).
> L'UI est un simple afficheur/client. Ce découplage est intentionnel :
> - Si l'UI crash, le daemon continue de tourner
> - Si le daemon est arrêté, l'UI affiche "déconnecté" et tente de se reconnecter

---

## 12. Structure des fichiers (détaillée)

| Fichier | Niveau | Rôle |
|---|---|---|
| `Cargo.toml` | 🔴 CRITIQUE | Dépendances externes + metadata bundle |
| `src/main.rs` | 🔴 CRITIQUE | `main()` : init config → init logging → lance AppKit run loop + client socket |
| `src/config.rs` | 🟠 OBLIGATOIRE | `UiConfig` : `socket_path`, `reconnect_delay_ms` |
| `src/error.rs` | 🟡 IMPORTANT | `UiError` enum (IpcConnection, StatusBar, AppKit) |
| `src/app.rs` | 🔴 CRITIQUE | `UiApp` : NSApplication setup, NSStatusBar creation, main run loop |
| `src/status_item.rs` | 🟠 OBLIGATOIRE | `MenuBarIcon` : création du NSStatusItem, menu builder, update state |
| `src/socket_client.rs` | 🔴 CRITIQUE | `IpcClient::run()` : thread client socket, parsing, reconnexion |
| `src/delegate.rs` | 🟡 IMPORTANT | `AppDelegate` : callbacks `applicationDidFinishLaunching`, `applicationWillTerminate` |
| `src/panel.rs` | 🟢 OPTIONNEL | `DetectionsPanel` : fenêtre flottante NSPanel avec historique (v2, pas nécessaire en POC) |
| `tests/` | 🟠 OBLIGATOIRE | Tests unitaires + intégration (socket mock) |
| `assets/AppIcon.icns` | 🟢 OPTIONNEL | Icône de l'application (pour le .app bundle) |
| `stack.md` | 🟡 IMPORTANT | Ce fichier |
| `backlog.md` | 🟠 OBLIGATOIRE | Planification des tâches |

---

## 13. Dépendances exclues volontairement

| Crate exclu | Raison |
|---|---|
| `audio_capture` | L'UI ne capture pas d'audio |
| `pipeline_dsp` | L'UI ne fait pas de DSP |
| `inference_ml` | L'UI ne fait pas d'inférence |
| `trigger` | L'UI ne gère pas le trigger |
| `tauri` / `wry` | WebView >50 Mo inacceptable |
| `egui` / `iced` | Pas d'intégration NSStatusBar native |
| `core-foundation` | Redondant avec `icrate` (qui expose déjà CF types) |
