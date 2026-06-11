# Backlog — Crate `ui`

> Découpage industriel des tâches élémentaires pour la conception, l'implémentation et la validation de l'application graphique (UI) Word Waker.
> Chaque tâche est atomique, testable, et les tests s'incrémentent avec les fonctionnalités.
> L'UI doit être **compilable, lançable et arrêtable proprement** à tout moment du développement.

---

## Légende

| Symbole | Signification |
|---|---|
| `[SETUP]` | Infrastructure, environnement, configuration |
| `[IMPL]` | Implémentation d'une fonctionnalité |
| `[TEST-I]` | Test d'intégration (socket mock, simule le daemon) |
| `[TEST-U]` | Test unitaire (parsing, config, structures) |
| `[VALID]` | Validation manuelle sur macOS |
| `[ ]` | Non commencé |
| `[x]` | Terminé |

---

## PARTIE 0 — Installation & Configuration de l'environnement

> **Objectif :** Avoir un crate `ui` qui compile avec `cargo check`, sans modifier les crates existants.

---

### P0.1 — Création de la structure du crate

- [x] `[SETUP]` Créer le crate `ui` comme binary crate dans le workspace (`cargo new --bin ui`)
- [x] `[SETUP]` Ajouter `ui` dans le `[workspace]` du `Cargo.toml` racine
- [x] `[SETUP]` Créer l'arborescence : `src/`, `src/config.rs`, `src/error.rs`, `src/app.rs`, `src/status_item.rs`, `src/socket_client.rs`, `src/delegate.rs`
- [x] `[SETUP]` Créer le dossier `tests/` pour les tests d'intégration
- [x] `[TEST-U]` **Test de smoke :** `cargo check -p ui` — passe sans erreur

### P0.2 — Configuration des dépendances

- [x] `[SETUP]` Ajouter dans `[dependencies]` :
  - `icrate = { version = "0.1.2", features = ["AppKit", "AppKit_NSApplication", "AppKit_NSStatusBar", "AppKit_NSStatusItem", "AppKit_NSMenu", "AppKit_NSMenuItem", "AppKit_NSImage", "AppKit_NSRunningApplication", "AppKit_NSButton", "AppKit_NSTextField", "AppKit_NSView", "AppKit_NSLayoutConstraint", "Foundation", "Foundation_NSThread", "Foundation_NSRunLoop", "Foundation_NSTimer", "Foundation_NSProcessInfo"] }`
  - `objc2 = "0.6"`
  - `objc2-foundation = "0.3"`
  - `crossbeam-channel = "0.5"`
  - `anyhow = "1.0"`
  - `tracing = "0.1"`
  - `tracing-subscriber = { version = "0.3", features = ["env-filter"] }`
- [x] `[SETUP]` Ajouter `name = "word-waker-ui"` dans `[[bin]]` pour que le binaire ait un nom distinct du daemon
- [x] `[TEST-U]` **Test :** `cargo build -p ui` — compile sans erreur

### P0.3 — Configuration runtime

- [x] `[SETUP]` Créer `src/config.rs` avec la struct `UiConfig` :
  - `socket_path: String` — chemin du socket IPC (défaut `/tmp/wakeword_daemon.sock`)
  - `reconnect_delay_ms: u64` — délai entre tentatives de reconnexion (défaut `2000`)
  - `poll_interval_ms: u64` — intervalle de polling des événements (défaut `100`)
- [x] `[IMPL]` Implémenter `UiConfig::from_env()` — lit `WAKEWORD_SOCKET_PATH`, `WAKEWORD_UI_RECONNECT_MS` depuis les variables d'environnement avec valeurs par défaut
- [x] `[IMPL]` Implémenter `UiConfig::default()`
- [x] `[TEST-U]` **Test :** `UiConfig::from_env()` avec variables non définies → valeurs par défaut valides
- [x] `[TEST-U]` **Test :** `UiConfig::from_env()` avec `WAKEWORD_SOCKET_PATH=/tmp/custom.sock` → valeur personnalisée respectée

---

## PARTIE 1 — Client socket IPC

> **Objectif :** Un `IpcClient` qui se connecte au daemon via Unix Domain Socket, lit les notifications, et émet des événements. Aucune UI à ce stade — le test valide par mock server.

---

### P1.1 — Structure des événements

- [x] `[IMPL]` Créer `src/socket_client.rs` avec l'enum `UiEvent` :
  ```rust
  pub enum UiEvent {
      Connected,
      Disconnected,
      WakeWordDetected { timestamp: std::time::Instant },
      Error(String),
  }
  ```
- [x] `[TEST-U]` **Test :** `UiEvent` dérive `Debug`, `Send`, `Sync`
- [x] `[TEST-U]` **Test :** Un `UiEvent::WakeWordDetected` créé avec `Instant::now()` a un timestamp dans les 100 dernières ms

### P1.2 — Connexion socket client

- [x] `[IMPL]` Implémenter `IpcClient::connect(socket_path: &str) -> io::Result<UnixStream>` — tente de se connecter au socket, retourne `Err` si daemon absent
- [x] `[IMPL]` Implémenter `IpcClient::read_loop(stream: UnixStream, tx: &Sender<UiEvent>)` :
  - Wrap le stream dans un `BufReader`
  - Lit ligne par ligne
  - Si ligne == `"WAKEWORD_DETECTED"` → émet `UiEvent::WakeWordDetected`
  - Si erreur de lecture → retourne (le caller gère la reconnexion)
- [x] `[TEST-I]` **Test :** Mock server envoie `WAKEWORD_DETECTED\n` → `UiEvent::WakeWordDetected` reçu
- [x] `[TEST-I]` **Test :** Mock server envoie `WAKEWORD_DETECTED` sans `\n` (flush manuel) → toujours reçu après flush
- [x] `[TEST-I]` **Test :** Mock server ferme la connexion → `UiEvent::Disconnected` reçu
- [x] `[TEST-I]` **Test :** Socket inexistant → `UiEvent::Disconnected` ou `Error` reçu

### P1.3 — Boucle de reconnexion automatique

- [x] `[IMPL]` Implémenter `IpcClient::run(tx: Sender<UiEvent>, config: UiConfig)` — boucle infinie :
  ```
  loop {
      connect() → Ok(stream) → Connected event → read_loop(stream) → Disconnected event
      connect() → Err(_) → Disconnected event → sleep(reconnect_delay_ms) → retry
  }
  ```
- [x] `[TEST-I]` **Test :** Daemon mock démarre après 3 s d'attente → le client finit par se connecter et recevoir des événements
- [x] `[TEST-I]` **Test :** Daemon mock est killé → `Disconnected` puis `Connected` après redémarrage du mock
- [x] `[TEST-I]` **Test :** Le client ne spamme pas les `Disconnected` — exactement 1 événement par transition d'état

### P1.4 — Résistance aux malformations

- [x] `[TEST-I]` **Test :** Le daemon envoie des lignes vides → ignorées, pas de crash
- [x] `[TEST-I]` **Test :** Le daemon envoie une ligne de 10 Ko → ignorée (pas `WAKEWORD_DETECTED`), pas de crash, pas de OOM
- [x] `[TEST-I]` **Test :** Le daemon envoie des données binaires → pas de crash, pas de panic
- [x] `[TEST-I]` **Test :** Le daemon ferme la connexion brutalement (RST) → `Disconnected` émis, pas de panic

---

## PARTIE 2 — Application NSStatusBar minimale

> **Objectif :** Une app macOS qui vit dans la barre de menu, affiche un emoji, et propose un menu Quitter. Pas encore de connexion socket — on valide juste le cycle de vie AppKit.

---

### P2.1 — Initialisation NSApplication

- [x] `[IMPL]` Créer `src/app.rs` avec `UiApp` :
  - `new(config: UiConfig) -> Result<Self, UiError>`
  - `run(&self)` — lance `NSApplication::run()` (boucle événementielle AppKit)
- [x] `[IMPL]` Implémenter `NSApplication::sharedApplication()` avec `MainThreadMarker`
- [x] `[IMPL]` Configurer `LSUIElement = true` via `NSApplication::setActivationPolicy(NSApplicationActivationPolicy::Accessory)` — pas d'icône dans le Dock
- [x] `[VALID]` Lancer le binaire → aucune fenêtre, rien dans le Dock, processus visible dans Activity Monitor

### P2.2 — NSStatusBar avec emoji

- [x] `[IMPL]` Créer `src/status_item.rs` avec `MenuBarIcon` :
  - `new() -> Result<Self, UiError>` — crée un `NSStatusItem` dans la barre de menu
  - `set_title(emoji: &str)` — définit le texte affiché (ex: `"🎤"`)
  - `set_tooltip(text: &str)` — définit le tooltip au survol
- [x] `[IMPL]` Créer le `NSStatusItem` via `NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength)`
- [x] `[IMPL]` Définir le bouton du status item avec `setTitle_` et `setToolTip_`
- [x] `[VALID]` Lancer le binaire → emoji `🎤` visible dans la barre de menu (à droite, près de l'horloge)
- [x] `[VALID]` Survoler l'emoji → tooltip "Word Waker" visible

### P2.3 — Menu contextuel

- [x] `[IMPL]` Ajouter `MenuBarIcon::build_menu() -> NSMenu` — crée le menu déroulant :
  - Item "À propos de Word Waker" (désactivé en POC, version affichée en tooltip)
  - Item "---" (séparateur)
  - Item "Quitter" — avec callback `NSApplication::terminate()`
- [x] `[IMPL]` Associer le menu au `NSStatusItem` via `setMenu_`
- [x] `[VALID]` Clic gauche sur l'emoji → menu apparaît avec "Quitter"
- [x] `[VALID]` Clic sur "Quitter" → le processus se termine proprement (exit 0)

### P2.4 — AppDelegate (cycle de vie)

- [x] `[IMPL]` Créer `src/delegate.rs` avec un `AppDelegate` qui implémente :
  - `applicationDidFinishLaunching` — log "UI démarrée"
  - `applicationWillTerminate` — log "UI arrêtée", cleanup (joindre le thread socket)
- [x] `[IMPL]` Enregistrer le delegate via `NSApplication::setDelegate_`
- [x] `[VALID]` Quitter via le menu → logs `[INFO] UI arrêtée` visibles dans la console
- [x] `[VALID]` `leaks -- word-waker-ui` après arrêt → zéro leak NSObject/NSMenu

---

## PARTIE 3 — Intégration socket ↔ UI

> **Objectif :** Connecter le `IpcClient` à l'UI. Les événements socket mettent à jour l'affichage du `NSStatusItem` en temps réel.

---

### P3.1 — Bridge thread socket → main thread UI

- [x] `[IMPL]` Dans `UiApp::run()` :
  1. Créer `(tx_event, rx_event) = crossbeam_channel::unbounded::<UiEvent>()`
  2. Spawner `std::thread::spawn(move || IpcClient::run(tx_event, config))`
  3. Installer un `NSTimer` qui fire toutes les 100 ms et lit les événements depuis `rx_event`
- [x] `[IMPL]` Le callback du timer `try_recv()` les événements et les applique à l'UI
- [x] `[TEST-I]` **Test (avec mock, headless) :** Envoyer `WAKEWORD_DETECTED\n` via mock server → le `StatusItem` change d'emoji brièvement (via inspection de l'état interne)
- [x] `[TEST-I]` **Test :** Mock server s'arrête → `Disconnected` → l'état interne passe à `Disconnected`
- [x] `[TEST-I]` **Test :** Mock server redémarre → `Connected` → l'état repasse à `Connected`

### P3.2 — États visuels du StatusItem

- [x] `[IMPL]` Implémenter `MenuBarIcon::update_state(state: &DaemonState)` :
  - `Connected` → titre `"🎤"`, tooltip `"Word Waker — en écoute"`
  - `Disconnected` → titre `"⏸"`, tooltip `"Word Waker — daemon déconnecté"`
  - `Detected` → flash titre `"🎤✅"` pendant 500 ms, puis retour à `"🎤"`
- [x] `[IMPL]` Implémenter le flash via `NSTimer` (ou `dispatch_after` via GCD) — pas de `sleep()` bloquant le thread UI
- [x] `[VALID]` Lancer daemon + UI → `🎤` visible, tooltip OK
- [x] `[VALID]` Tuer le daemon (kill) → l'UI passe à `⏸` dans les 5 secondes
- [x] `[VALID]` Redémarrer le daemon → l'UI repasse à `🎤` automatiquement
- [x] `[VALID]` Prononcer le mot-clé → flash `🎤✅` visible, puis retour à `🎤`

### P3.3 — Compteur de détections dans le menu

- [x] `[IMPL]` Ajouter un item de menu `"Détections : 0"` mis à jour à chaque `WakeWordDetected`
- [x] `[IMPL]` Le compteur persiste jusqu'à fermeture de l'UI (pas de reset)
- [x] `[VALID]` 3 détections successives → menu affiche `"Détections : 3"`
- [x] `[TEST-U]` **Test unitaire :** Simuler 5 `WakeWordDetected` → compteur = 5, pas de panic sur overflow

### P3.4 — Gestion des erreurs socket

- [x] `[IMPL]` En cas d'erreur fatale du thread socket (3 reconnexions échouées), logger l'erreur et continuer (ne pas crasher l'UI)
- [x] `[TEST-I]` **Test :** Socket path pointe vers un fichier non-socket → `UiEvent::Error` émis, UI pas crashée
- [x] `[TEST-I]` **Test :** Permissions insuffisantes sur le socket → `Error`, UI continue

---

## PARTIE 4 — Fenêtre flottante de détails (v1.1)

> **Objectif :** Une mini-fenêtre NSPanel qui affiche l'historique des détections avec horodatage, ouverte via le menu. **Cette partie est optionnelle pour le POC initial**, mais posée ici pour planification future.

---

### P4.1 — Création de la NSPanel

- [x] `[IMPL]` Créer `src/panel.rs` avec `DetectionsPanel` :
  - `new() -> Result<Self, UiError>`
  - `show()` — rend la fenêtre visible
  - `hide()` — masque la fenêtre
  - `add_detection(timestamp: Instant)` — ajoute une ligne à l'historique
- [x] `[IMPL]` Créer une `NSPanel` avec `NSWindowStyleMask::Titled | NSWindowStyleMask::Closable | NSWindowStyleMask::UtilityWindow`
- [x] `[IMPL]` Configurer `setFloatingPanel(true)` et `setHidesOnDeactivate(false)`
- [x] `[IMPL]` Ajouter un `NSTextView` (ou `NSTableView`) comme vue de contenu avec scroll
- [x] `[VALID]` Ouvrir la fenêtre via le menu → fenêtre flottante visible, scrollable si >20 détections
- [x] `[VALID]` Fermer la fenêtre (bouton rouge) → fenêtre masquée, pas quittée

### P4.2 — Intégration avec le StatusItem menu

- [x] `[IMPL]` Ajouter un item `"Historique..."` dans le menu entre "Détections : N" et "Quitter"
- [x] `[IMPL]` Le callback de l'item appelle `panel.show()`
- [x] `[VALID]` Clic "Historique..." → fenêtre apparaît, focus au champ de texte

---

## PARTIE 5 — Tests end-to-end & validation

> **Objectif :** Vérifier le comportement complet en conditions réelles (avec daemon) et en conditions mock (CI).

---

### P5.1 — Validation avec daemon réel

- [ ] `[VALID]` **Démarrage :** `cargo run --release -p ui` — l'UI s'affiche dans la barre de menu, pas de fenêtre, pas d'icône Dock
- [ ] `[VALID]` **Connexion :** Daemon déjà lancé → UI affiche `🎤` en moins d'1 s
- [ ] `[VALID]` **Détection :** Prononcer le mot-clé → flash `🎤✅`, compteur incrémenté dans le menu
- [ ] `[VALID]` **Déconnexion :** `kill` le daemon → UI passe à `⏸` en moins de 5 s
- [ ] `[VALID]` **Reconnexion :** Redémarrer le daemon → UI repasse à `🎤` en moins de 3 s
- [ ] `[VALID]` **Quitter :** Menu → Quitter → processus terminé, exit 0, zéro thread zombie

### P5.2 — Validation sans daemon (mock)

- [x] `[TEST-I]` **Test complet mock :** `IpcClient` + mock server → événements `Connected`, `WakeWordDetected`, `Disconnected` tous reçus dans l'ordre
- [x] `[TEST-I]` **Test de non-régression :** `cargo test -p ui` — tous les tests passent sans daemon ni écran
- [ ] `[VALID]` `cargo test -p ui` fonctionne en CI (GitHub Actions macOS runner)

### P5.3 — Robustesse macOS

- [ ] `[VALID]` **Sleep/wake :** Mettre le Mac en veille 30 s, réveiller → UI toujours fonctionnelle, reconnexion automatique au daemon
- [ ] `[VALID]` **Multi-instances :** Lancer 2 UI en parallèle → les deux reçoivent les notifications (le daemon accepte N clients)
- [ ] `[VALID]` **Mémoire longue durée :** Laisser tourner 24 h → `leaks` ne montre aucune fuite, RSS stable
- [ ] `[VALID]` **Dark mode :** Passer en dark mode → l'emoji reste lisible
- [ ] `[VALID]` **Accessibilité :** VoiceOver lit le tooltip du StatusItem

### P5.4 — Intégration workspace

- [x] `[VALID]` `cargo build --release --workspace` — compile daemon + UI sans conflit de dépendances
- [x] `[VALID]` `cargo test --workspace` — tous les tests passent (audio_capture, pipeline_dsp, inference_ml, trigger, integration_test, daemon, ui)
- [ ] `[VALID]` `cargo clippy --workspace -- -D warnings` — zéro warning

---

## PARTIE 6 — Performance

> **Objectif :** Vérifier que l'UI ne consomme pas de ressources inutiles.

---

### P6.1 — CPU idle

- [ ] `[VALID]` UI lancée, daemon OK, aucune détection — `ps -o %cpu= -p <pid>` sur 10 échantillons → CPU moyen < 0.5 %
- [ ] `[VALID]` Thread socket bloqué sur `read()` → ne consomme pas de CPU en idle
- [ ] `[VALID]` NSTimer 100 ms → overhead < 0.1 %

### P6.2 — Mémoire

- [ ] `[VALID]` RSS après 1 h d'exécution < 15 Mo
- [ ] `[VALID]` 1000 détections reçues → RSS stable (le compteur est un simple u64, pas de fuite)

### P6.3 — Latence de détection

- [ ] `[VALID]` Mesurer le délai entre l'émission `WAKEWORD_DETECTED\n` par le daemon et le flash UI → < 50 ms
- [ ] `[VALID]` La latence n'augmente pas après 1000 détections

### P6.4 — Build release

- [ ] `[VALID]` `cargo build --release -p ui` — compile sans warning
- [ ] `[VALID]` Taille du binaire `target/release/word-waker-ui` < 3 Mo (strip + LTO thin)

---

## PARTIE 7 — Packaging .app bundle

> **Objectif :** Produire un `.app` macOS natif exécutable par double-clic ou via `open`.

---

### P7.1 — Script de packaging manuel

- [ ] `[IMPL]` Créer `scripts/package_ui.sh` :
  - Compile `cargo build --release -p ui`
  - Crée `WordWaker.app/Contents/MacOS/`, `Resources/`
  - Copie le binaire
  - Génère `Info.plist` avec `LSUIElement = true`
  - Optionnel : copie une icône `AppIcon.icns`
- [ ] `[VALID]` `bash scripts/package_ui.sh` → `WordWaker.app` créé
- [ ] `[VALID]` `open WordWaker.app` → l'UI démarre dans la barre de menu

### P7.2 — Signature & notarisation (optionnel, distribution)

- [ ] `[VALID]` `codesign -s - --force --deep WordWaker.app` — signé ad-hoc
- [ ] `[VALID]` `spctl -a -v WordWaker.app` — accepté par Gatekeeper (signature ad-hoc suffit pour usage local)

### P7.3 — Lancement automatique au login

- [ ] `[SETUP]` Documenter l'ajout dans `Préférences Système → Général → Ouverture`
- [ ] `[VALID]` Ajouter `WordWaker.app` aux éléments d'ouverture → l'UI démarre au login, dans la barre de menu
- [ ] `[VALID]` Le daemon (LaunchAgent) démarre aussi au login → UI affiche `🎤` automatiquement

---

## Résumé — Ordre de réalisation recommandé

```
P0 (setup) → P1 (socket client) → P2 (NSStatusBar) → P3 (intégration) → P5 (validation)
                                                                              ↓
                                                                         P6 (perf) → P7 (packaging)
                                                                              ↓
                                                                         P4 (panel, v1.1)
```

> **POC minimal viable = P0 + P1 + P2 + P3.1-3.2.** L'utilisateur voit un emoji dans la barre
> de menu qui change d'état. C'est 300-400 lignes de Rust, livrable en 1-2 jours.

---

## Notes d'implémentation

### Gestion du Main Thread

AppKit exige que toutes les mutations UI se fassent sur le main thread. Le `MainThreadMarker` de `objc2` garantit cela au niveau du type.

```rust
use objc2::rc::Retained;
use objc2_foundation::MainThreadMarker;

let mtm = MainThreadMarker::new().expect("must be called from main thread");
// mtm est passé aux fonctions qui créent des objets UI
```

Le thread socket n'a pas accès à `mtm`. Il envoie des `UiEvent` via un channel, et c'est le `NSTimer` sur le main thread qui les consomme et met à jour l'UI.

### Tests headless

Les tests UI ne peuvent pas vraiment "afficher" d'UI dans un runner cargo test. La stratégie est :
1. **Tests unitaires purs** — `UiConfig`, `UiEvent`, parsing, logique métier. Aucune dépendance AppKit.
2. **Tests d'intégration socket** — Mock serveur Unix socket. Aucune dépendance AppKit non plus.
3. **Tests UI (AppKit)** — Option `#[cfg(test)]` avec un mock `MainThreadMarker` ou exécution via `cargo run --example smoke_test`

### Alternative plus simple pour le POC : utiliser `objc2` sans `icrate`

Si `icrate` s'avère trop lourd à configurer (features, linking), on peut descendre d'un niveau et utiliser directement `objc2` avec des appels `msg_send!` manuels vers AppKit. C'est plus verbeux (~600 lignes vs 300) mais ne dépend que de `objc2` + `objc2-foundation`.

```rust
// Exemple sans icrate, juste objc2 :
use objc2::msg_send;
use objc2::rc::Retained;
use objc2_foundation::NSDictionary;

let status_bar: Retained<NSStatusBar> = unsafe {
    msg_send![class!(NSStatusBar), systemStatusBar]
};
```

**Recommandation :** Commencer avec `icrate` (plus rapide à développer). Si problèmes de compilation, revenir à `objc2` pur.
