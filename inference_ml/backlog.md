# Backlog — Module `inference_ml`

> Découpage industriel des tâches élémentaires pour la conception, l'implémentation et la validation du module d'inférence ML.
> Chaque tâche est atomique, testable, et les tests s'incrémentent avec les fonctionnalités.
> Le module doit être **compilable, testable et exécutable de manière isolée** (sans `pipeline_dsp` ni `trigger`) à tout moment.
>
> **Rappel architecture :** le modèle s'exécute **in-process** via un bridge Objective-C++ statique. Pas de socket, pas de process externe.

---

## Légende

| Symbole | Signification |
|---|---|
| `[SETUP]` | Infrastructure, environnement, configuration |
| `[IMPL-RS]` | Implémentation Rust |
| `[IMPL-MM]` | Implémentation Objective-C++ (fichier `.mm`) |
| `[TEST-U]` | Test unitaire (logique pure, sans modèle réel) |
| `[TEST-I]` | Test d'intégration (avec `.mlmodelc` mock ou réel) |
| `[TEST-P]` | Test de performance / benchmark |
| `[VALID]` | Validation manuelle ou instrumentée |
| `[ ]` | Non commencé |
| `[x]` | Terminé |

---

## PARTIE 0 — Installation & Configuration de l'environnement

> **Objectif :** Avoir un crate `inference_ml` qui compile, linke `CoreML.framework`, compile le bridge `.mm`, et passe `cargo check`.

---

### P0.1 — Vérification des prérequis système

- [x] `[SETUP]` Vérifier la présence de `CoreML.framework` : `ls /Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/System/Library/Frameworks/CoreML.framework`
- [x] `[SETUP]` Vérifier que `clang++` supporte `-fobjc-arc` et l'import `<CoreML/CoreML.h>` : `echo '#import <CoreML/CoreML.h>' | clang++ -fobjc-arc -x objective-c++ -fsyntax-only -`
- [x] `[SETUP]` Vérifier que `xcrun coremlcompiler` est disponible : `xcrun coremlcompiler --help` *(nécessite `sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer`)*
- [x] `[SETUP]` Vérifier Python 3.10+ et `coremltools` pour la génération du modèle mock : `python3 -c "import coremltools; print(coremltools.__version__)"` *(Python 3.14, coremltools 9.0 — format NeuralNetwork utilisé car libmilstoragepython absent sur Python 3.14 ; input shape [1,98,13] au lieu de [1,1,98,13])*
- [x] `[TEST-U]` **Test de smoke :** Les commandes ci-dessus s'exécutent sans erreur

### P0.2 — Création de la structure du crate

- [x] `[SETUP]` Créer (ou confirmer) le crate `inference_ml` dans le workspace
- [x] `[SETUP]` Créer l'arborescence : `src/inference_ml/`, `src/bridge/`, `tests/`, `fixtures/mock_model/`
- [x] `[SETUP]` Créer le fichier `src/bridge/coreml_bridge.mm` vide
- [x] `[SETUP]` Créer le `build.rs` vide
- [x] `[TEST-U]` **Test de smoke :** `cargo check -p inference_ml` — passe sans erreur

### P0.3 — Configuration des dépendances Cargo

- [x] `[SETUP]` Ajouter `libc = "0.2"` dans `[dependencies]`
- [x] `[SETUP]` Ajouter `anyhow = "1.0"` dans `[dependencies]`
- [x] `[SETUP]` Ajouter `crossbeam-channel = "0.5"` dans `[dependencies]`
- [x] `[SETUP]` Ajouter `tracing = "0.1"` et `tracing-subscriber = "0.3"` dans `[dependencies]`
- [x] `[SETUP]` Ajouter `cc = "1.0"` dans `[build-dependencies]`
- [x] `[TEST-U]` **Test :** `cargo build -p inference_ml` — compile sans erreur (bridge vide acceptable à ce stade)

### P0.4 — Configuration du `build.rs` complet

- [x] `[SETUP]` Dans `build.rs`, appeler `cc::Build::new()` avec `.file("src/bridge/coreml_bridge.mm")`, `.flag("-fobjc-arc")`, `.flag("-std=c++20")`, `.flag("-mmacosx-version-min=14.0")`, `.compile("coreml_bridge")`
- [x] `[SETUP]` Ajouter dans `build.rs` : `println!("cargo:rustc-link-lib=framework=CoreML")`
- [x] `[SETUP]` Ajouter : `println!("cargo:rustc-link-lib=framework=Foundation")`
- [x] `[SETUP]` Ajouter : `println!("cargo:rustc-link-lib=framework=CoreFoundation")`
- [x] `[TEST-U]` **Test :** `cargo build -p inference_ml` avec le bridge `.mm` vide mais syntaxiquement valide — linke sans erreur

### P0.5 — Feature flags

- [x] `[SETUP]` Déclarer feature `mock_model` — active un stub Rust qui contourne le FFI et retourne toujours 0.5
- [x] `[SETUP]` Déclarer feature `standalone` — active un `example` exécutable en autonomie
- [x] `[TEST-U]` **Test :** `cargo build -p inference_ml --features mock_model,standalone` — compile

---

## PARTIE 1 — Gestion des erreurs et configuration

> **Objectif :** Typer toutes les erreurs possibles avant toute implémentation.

---

### P1.1 — Type d'erreur du module

- [x] `[IMPL-RS]` Créer `src/inference_ml/error.rs`
- [x] `[IMPL-RS]` Définir l'enum `InferenceError` avec les variantes : `ModelNotFound(String)`, `LoadFailed(String)`, `NullHandle`, `InvalidInputShape { expected: (usize,usize,usize,usize), got: usize }`, `InferenceFailed(String)`, `OutputNotFound(String)`, `ChannelClosed`
- [x] `[IMPL-RS]` Implémenter `std::fmt::Display` pour `InferenceError`
- [x] `[IMPL-RS]` Implémenter `std::error::Error` pour `InferenceError`
- [x] `[TEST-U]` **Test unitaire :** Chaque variante produit un message `Display` non vide et distinct
- [x] `[TEST-U]` **Test unitaire :** `InferenceError` est `Send + Sync`

### P1.2 — Configuration

- [x] `[IMPL-RS]` Créer `src/inference_ml/config.rs`
- [x] `[IMPL-RS]` Définir la struct `InferenceConfig` : `model_path: String`, `input_name: String`, `output_name: String`, `n_frames: usize`, `n_mfcc: usize`
- [x] `[IMPL-RS]` Implémenter `Default` : `input_name="mfcc_input"`, `output_name="classLabel_probs"`, `n_frames=98`, `n_mfcc=13`
- [x] `[IMPL-RS]` Implémenter `validate(&self) -> Result<(), InferenceError>` : vérifier que `model_path` n'est pas vide, que `n_frames > 0`, que `n_mfcc > 0`
- [x] `[TEST-U]` **Test unitaire :** Config valide avec un chemin non vide → `validate()` retourne `Ok`
- [x] `[TEST-U]` **Test unitaire :** `model_path` vide → `validate()` retourne `Err(ModelNotFound)`
- [x] `[TEST-U]` **Test unitaire :** `n_frames = 0` → `validate()` retourne `Err`

---

## PARTIE 2 — Génération du modèle mock (artifact offline)

> **Objectif :** Avoir un `.mlmodelc` minimal et valide dans `fixtures/` pour tous les tests d'intégration,
> sans avoir besoin d'un entraînement PyTorch complet.

---

### P2.1 — Script de génération du modèle mock

- [x] `[SETUP]` Créer `scripts/generate_mock_model.py`
- [x] `[SETUP]` Le script crée un `CoreML.MLModel` minimal via `coremltools` : entrée `mfcc_input` shape [1,98,13] Float32 *(NeuralNetwork format — Python 3.14 impose .mlmodel car libmilstoragepython absent)*, sortie `classLabel_probs` shape [2] Float32 (retourne toujours [0.5, 0.5])
- [x] `[SETUP]` Le script exporte en `.mlmodel` puis appelle `xcrun coremlcompiler compile` pour produire `.mlmodelc` dans `fixtures/mock_model/`
- [x] `[SETUP]` Versionner `fixtures/mock_model/` dans le dépôt (artefact binaire léger, ~12 Ko)
- [x] `[TEST-U]` **Test de smoke :** `python3 scripts/generate_mock_model.py` s'exécute sans erreur et le répertoire `fixtures/mock_model/WakeWordMock.mlmodelc` existe

### P2.2 — Validation du modèle mock

- [x] `[VALID]` **Validation manuelle :** `ls fixtures/mock_model/WakeWordMock.mlmodelc/` — contient `coremldata.bin`, `model.espresso.*`, `metadata.json` (~12 Ko) ✅

---

## PARTIE 3 — Bindings FFI Rust

> **Objectif :** Déclarer les 3 fonctions C du bridge côté Rust, vérifier que les signatures compilent.

---

### P3.1 — Déclarations FFI

- [x] `[IMPL-RS]` Créer `src/inference_ml/ffi.rs`
- [x] `[IMPL-RS]` Déclarer le type `CoreMLHandle = *mut libc::c_void`
- [x] `[IMPL-RS]` Déclarer `extern "C" { fn coreml_load(path: *const libc::c_char) -> CoreMLHandle; }`
- [x] `[IMPL-RS]` Déclarer `extern "C" { fn coreml_infer(handle: CoreMLHandle, mfcc_flat: *const f32, len: libc::size_t) -> f32; }`
- [x] `[IMPL-RS]` Déclarer `extern "C" { fn coreml_free(handle: CoreMLHandle); }`
- [x] `[TEST-U]` **Test de compilation :** `cargo build -p inference_ml` avec le bridge `.mm` qui définit les 3 fonctions vides — doit linker sans symboles manquants

---

## PARTIE 4 — Bridge Objective-C++ (`coreml_bridge.mm`)

> **Objectif :** Implémenter les 3 fonctions C qui encapsulent l'API Objective-C de Core ML.

---

### P4.1 — Squelette et includes

- [x] `[IMPL-MM]` Ajouter les imports : `#import <CoreML/CoreML.h>`, `#include <stdint.h>`, `#include <string.h>`
- [x] `[IMPL-MM]` Déclarer `extern "C" {` avec les 3 prototypes vides
- [x] `[IMPL-MM]` Définir `typedef void* CoreMLHandle`
- [x] `[TEST-U]` **Test de compilation :** `cargo build -p inference_ml` — le `.mm` compile avec `-fobjc-arc`

### P4.2 — Implémentation de `coreml_load`

- [x] `[IMPL-MM]` Convertir `mlmodelc_path` en `NSString` via `stringWithUTF8String:`
- [x] `[IMPL-MM]` Créer un `NSURL` avec `fileURLWithPath:`
- [x] `[IMPL-MM]` Instancier `MLModelConfiguration` et positionner `computeUnits = MLComputeUnitsAll`
- [x] `[IMPL-MM]` Appeler `[MLModel modelWithContentsOfURL:url configuration:config error:&err]`
- [x] `[IMPL-MM]` Si `err != nil` : logger l'erreur via `NSLog` et retourner `nullptr`
- [x] `[IMPL-MM]` Retourner `(CoreMLHandle)CFBridgingRetain(model)` pour transférer la propriété ARC à Rust
- [x] `[TEST-I]` **Test d'intégration :** Appeler `coreml_load` avec le chemin du modèle mock — doit retourner un handle non null

### P4.3 — Implémentation de `coreml_infer`

- [x] `[IMPL-MM]` Récupérer le `MLModel*` via `(__bridge MLModel*)handle`
- [x] `[IMPL-MM]` Créer `MLMultiArray` avec shape `@[@1, @98, @13]` et type `MLMultiArrayDataTypeFloat32` *(shape 3D — voir note P2.1)*
- [x] `[IMPL-MM]` Copier `mfcc_flat` dans `array.dataPointer` via `memcpy(array.dataPointer, mfcc_flat, len * sizeof(float))`
- [x] `[IMPL-MM]` Créer `MLFeatureValue` → `NSDictionary` → `MLDictionaryFeatureProvider`
- [x] `[IMPL-MM]` Appeler `[model predictionFromFeatures:input error:&err]`
- [x] `[IMPL-MM]` Extraire `[output featureValueForName:@"classLabel_probs"].multiArrayValue`
- [x] `[IMPL-MM]` Retourner `[probs objectAtIndexedSubscript:1].floatValue` (index 1 = wake-word)
- [x] `[IMPL-MM]` En cas d'erreur : logger et retourner `0.0f`
- [x] `[TEST-I]` **Test d'intégration :** Appeler `coreml_infer` avec le modèle mock et une matrice de zéros — doit retourner une valeur dans [0.0, 1.0] sans crash

### P4.4 — Implémentation de `coreml_free`

- [x] `[IMPL-MM]` Appeler `CFBridgingRelease(handle)` pour redonner la propriété à ARC et libérer l'objet
- [x] `[TEST-I]` **Test d'intégration :** `load → free` sans inférence — aucun crash, AddressSanitizer ne rapporte rien

### P4.5 — Robustesse du bridge

- [x] `[IMPL-MM]` `coreml_load` : vérifier que `path != nullptr` avant utilisation
- [x] `[IMPL-MM]` `coreml_infer` : vérifier que `handle != nullptr` et `mfcc_flat != nullptr`
- [x] `[IMPL-MM]` `coreml_free` : vérifier que `handle != nullptr` avant `CFBridgingRelease`
- [x] `[TEST-I]` **Test d'intégration :** `coreml_load(nullptr)` → retourne `nullptr` sans crash
- [x] `[TEST-I]` **Test d'intégration :** `coreml_infer(nullptr, ...)` → retourne 0.0 sans crash

---

## PARTIE 5 — Wrapper Rust `CoreMLModel`

> **Objectif :** Encapsuler le handle FFI dans un type Rust safe, avec gestion automatique de la durée de vie.

---

### P5.1 — Struct et constructeur

- [x] `[IMPL-RS]` Créer `src/inference_ml/model.rs`
- [x] `[IMPL-RS]` Définir `pub struct CoreMLModel { handle: ffi::CoreMLHandle }`
- [x] `[IMPL-RS]` Déclarer `unsafe impl Send for CoreMLModel {}` et `unsafe impl Sync for CoreMLModel {}`
- [x] `[IMPL-RS]` Implémenter `CoreMLModel::load(config: &InferenceConfig) -> Result<Self, InferenceError>` :
  - [x] `[IMPL-RS]` Vérifier que le chemin existe (`std::path::Path::exists`)
  - [x] `[IMPL-RS]` Construire un `CString` depuis `config.model_path`
  - [x] `[IMPL-RS]` Appeler `unsafe { ffi::coreml_load(cstring.as_ptr()) }`
  - [x] `[IMPL-RS]` Vérifier que le handle n'est pas null → `Err(InferenceError::NullHandle)` sinon
- [x] `[TEST-I]` **Test d'intégration :** `CoreMLModel::load` avec le modèle mock → `Ok`
- [x] `[TEST-I]` **Test d'intégration :** `CoreMLModel::load` avec un chemin inexistant → `Err(ModelNotFound)`
- [ ] `[TEST-U]` **Test unitaire (feature mock_model) :** `CoreMLModel::load` retourne un stub sans appeler le bridge FFI

### P5.2 — Méthode d'inférence

- [x] `[IMPL-RS]` Implémenter `CoreMLModel::infer(&self, mfcc: &[[f32;13];98]) -> Result<f32, InferenceError>` :
  - [x] `[IMPL-RS]` Aplatir `mfcc` en `Vec<f32>` via `mfcc.iter().flatten().copied().collect()`
  - [x] `[IMPL-RS]` Appeler `unsafe { ffi::coreml_infer(self.handle, flat.as_ptr(), flat.len()) }`
  - [x] `[IMPL-RS]` Vérifier que le score retourné est dans [0.0, 1.0] (invariant de contrat) → `Err(InferenceFailed)` sinon
- [x] `[TEST-I]` **Test d'intégration :** Inférence sur matrice de zéros → score dans [0.0, 1.0]
- [x] `[TEST-I]` **Test d'intégration :** Inférence sur matrice de valeurs aléatoires → score dans [0.0, 1.0]
- [ ] `[TEST-U]` **Test unitaire :** Score = -0.1 retourné par le bridge → `Err(InferenceFailed)` (validation de l'invariant)
- [ ] `[TEST-U]` **Test unitaire :** Score = 1.1 → `Err(InferenceFailed)`

### P5.3 — Drop et gestion mémoire

- [x] `[IMPL-RS]` Implémenter `Drop for CoreMLModel` : appelle `unsafe { ffi::coreml_free(self.handle) }` si le handle n'est pas null
- [x] `[TEST-I]` **Test d'intégration :** Créer une instance dans un scope, vérifier via AddressSanitizer que le `MLModel` est libéré à la sortie du scope — zéro fuite
- [x] `[TEST-I]` **Test d'intégration :** Créer et dropper 1000 instances successives → consommation mémoire stable

---

## PARTIE 6 — Thread d'inférence (`InferenceRunner`)

> **Objectif :** Implémenter le thread dédié qui tourne en continu, reçoit des matrices MFCC et émet des scores.

---

### P6.1 — Structure du runner

- [x] `[IMPL-RS]` Créer `src/inference_ml/runner.rs`
- [x] `[IMPL-RS]` Définir `pub struct InferenceRunner { model: Arc<CoreMLModel>, running: Arc<AtomicBool>, thread_handle: Option<JoinHandle<()>> }`
- [x] `[IMPL-RS]` Implémenter `InferenceRunner::new(model: CoreMLModel) -> Self`

### P6.2 — Boucle d'inférence

- [x] `[IMPL-RS]` Implémenter `InferenceRunner::start(rx: Receiver<[[f32;13];98]>, tx: Sender<f32>) -> Result<(), InferenceError>` :
  - [x] `[IMPL-RS]` Spawner un thread qui boucle sur `rx.recv()` (bloquant — pas de polling)
  - [x] `[IMPL-RS]` Pour chaque matrice reçue : appeler `model.infer(mfcc)`, envoyer le score via `tx.send(score)`
  - [x] `[IMPL-RS]` Si `rx.recv()` retourne `Err` (channel fermé) : sortir de la boucle proprement
  - [x] `[IMPL-RS]` Logger le score et la latence via `tracing::debug!`
- [x] `[IMPL-RS]` Implémenter `InferenceRunner::stop()` : poser `running` à false, `join` le thread
- [x] `[IMPL-RS]` Implémenter `Drop for InferenceRunner` : appelle `stop()` silencieusement

### P6.3 — Tests du runner

- [x] `[TEST-I]` **Test d'intégration :** Envoyer 5 matrices via le channel, vérifier que 5 scores sont reçus dans [0.0, 1.0]
- [x] `[TEST-I]` **Test d'intégration :** Fermer le `Sender` → le thread se termine proprement, `stop()` ne panic pas
- [x] `[TEST-I]` **Test d'intégration :** Drop du runner sans `stop()` explicite → thread terminé, zéro thread zombie
- [x] `[TEST-I]` **Test d'intégration :** Inférence sur 100 matrices consécutives → consommation mémoire stable (pas de fuite par appel)

---

## PARTIE 7 — Façade publique `InferenceEngine`

> **Objectif :** Exposer une API unifiée et stable, seule surface visible depuis le reste du workspace.

---

### P7.1 — Struct `InferenceEngine`

- [x] `[IMPL-RS]` Créer ou compléter `src/inference_ml/mod.rs`
- [x] `[IMPL-RS]` Définir `pub struct InferenceEngine { runner: InferenceRunner, config: InferenceConfig }`
- [x] `[IMPL-RS]` Implémenter `InferenceEngine::new(config: InferenceConfig) -> Result<Self, InferenceError>` : valide config, charge `CoreMLModel`, crée `InferenceRunner`
- [x] `[IMPL-RS]` Implémenter `InferenceEngine::start(rx: Receiver<[[f32;13];98]>, tx: Sender<f32>) -> Result<(), InferenceError>`
- [x] `[IMPL-RS]` Implémenter `InferenceEngine::stop() -> Result<(), InferenceError>`
- [x] `[IMPL-RS]` Implémenter `Drop for InferenceEngine` — appelle `stop()` silencieusement

### P7.2 — Tests de la façade

- [x] `[TEST-I]` **Test d'intégration :** Cycle complet `new → start → 3 inférences → stop` avec modèle mock — tous les scores dans [0.0, 1.0]
- [ ] `[TEST-I]` **Test d'intégration :** Deux cycles `start/stop` consécutifs — idempotence, zéro panique
- [x] `[TEST-I]` **Test d'intégration :** Drop sans stop → propre

### P7.3 — Mode standalone

- [x] `[IMPL-RS]` Créer `examples/standalone_inference.rs` (feature `standalone`)
- [x] `[IMPL-RS]` L'exemple charge le modèle mock, génère 10 matrices MFCC aléatoires, affiche les scores et la latence médiane, puis s'arrête
- [x] `[TEST-I]` **Test :** `cargo run --example standalone_inference --features standalone` — s'exécute sans erreur
- [x] `[VALID]` **Validation manuelle :** Les scores affichés sont dans [0.0, 1.0] et la latence est < 50 ms (CPU fallback acceptable ici) — latence médiane ~62 µs ✅

---

## PARTIE 8 — Validation ANE et performance

> **Objectif :** Confirmer que l'inférence est bien déléguée à l'Apple Neural Engine, pas seulement exécutée sur CPU.

---

### P8.1 — Validation Instruments

- [ ] `[VALID]` **Validation Instruments → Neural Engine :** Lancer `cargo run --example instruments_loop -p inference_ml` pendant 30 s — vérifier dans l'onglet "Neural Engine" qu'il y a de l'activité ANE non nulle *(binary créé : `examples/instruments_loop.rs`)*
- [ ] `[VALID]` **Validation Instruments → CPU Profiler :** Confirmer que la consommation CPU du thread d'inférence est < 0,1 % en régime permanent
- [ ] `[VALID]` **Validation Instruments → Allocations :** Vérifier que l'empreinte mémoire du modèle chargé est < 5 Mo

### P8.2 — Benchmarks

- [x] `[TEST-P]` **Benchmark :** Latence d'une inférence sur ANE — `cargo bench -p inference_ml` — médiane **~17 µs** (< 5 ms requis) ✅
- [x] `[TEST-P]` **Benchmark :** Throughput : **~62 000 inf/s** (bien > 20 inf/s requis) ✅
- [ ] `[TEST-P]` **Benchmark :** Latence en CPU-only (`MLComputeUnitsCPUOnly`) — à mesurer pour documenter le gain ANE

---

## PARTIE 9 — Suite de tests complète & régression

> **Objectif :** Consolider tous les tests, garantir la non-régression, valider la mémoire.

---

### P9.1 — Organisation des tests

- [x] `[SETUP]` Créer `tests/inference_ml_integration.rs` — tests d'intégration du crate complet
- [x] `[SETUP]` Séparer les tests nécessitant le modèle mock (`#[cfg(not(feature = "skip_fixtures"))]`) de ceux nécessitant un modèle réel
- [x] `[SETUP]` Créer un module `src/inference_ml/mock.rs` (feature `mock_model`) : `CoreMLModel` stub qui retourne toujours 0.5 sans appeler le bridge

### P9.2 — Tests unitaires de régression

- [x] `[TEST-U]` **Régression :** `InferenceConfig::default()` avec model_path renseigné → `validate()` retourne `Ok`
- [x] `[TEST-U]` **Régression :** Toutes les variantes de `InferenceError` ont un `Display` non vide
- [x] `[TEST-U]` **Régression :** Score invalide (< 0.0 ou > 1.0) → `Err(InferenceFailed)`
- [x] `[TEST-U]` **Régression :** `InferenceError` est `Send + Sync`

### P9.3 — Tests d'intégration de régression (modèle mock)

- [x] `[TEST-I]` **Régression (mock) :** `new → start → 10 inférences → stop` — aucune panique
- [x] `[TEST-I]` **Régression (mock) :** `new → drop` (sans start) — zéro fuite
- [x] `[TEST-I]` **Régression (mock) :** 1000 `load → infer → free` consécutifs — mémoire stable
- [x] `[TEST-I]` **Régression (mock) :** Channel fermé pendant l'inférence → thread se termine proprement

### P9.4 — Tests AddressSanitizer

- [x] `[VALID]` **AddressSanitizer :** `RUSTFLAGS="-Z sanitizer=address" cargo +nightly test -p inference_ml --features mock_model` — zéro erreur mémoire
- [x] `[VALID]` **AddressSanitizer :** Idem avec le modèle mock réel (`.mlmodelc`) — zéro fuite sur le handle ARC

---

## PARTIE 10 — Documentation & Livraison du module

> **Objectif :** Module documenté, propre, prêt à être intégré.

---

### P10.1 — Documentation

- [ ] `[SETUP]` Ajouter doc-comments `///` sur tous les types et fonctions publics (`InferenceEngine`, `InferenceConfig`, `InferenceError`, `CoreMLModel`)
- [ ] `[SETUP]` Écrire un doc-example dans `InferenceEngine::new` montrant le cycle minimal `new/start/stop`
- [ ] `[SETUP]` Documenter dans le doc-comment de `CoreMLModel` la note de sécurité : "Le handle `MLModel` Obj-C est thread-safe en lecture — `unsafe impl Send/Sync` justifiés"
- [ ] `[TEST-U]` **Test :** `cargo doc --no-deps -p inference_ml` — sans erreur ni warning

### P10.2 — Validation finale

- [ ] `[VALID]` `cargo clippy -p inference_ml -- -D warnings` — zéro warning
- [ ] `[VALID]` `cargo fmt --check -p inference_ml` — code formaté
- [ ] `[VALID]` `cargo test -p inference_ml --features mock_model` — suite mock verte (sans modèle réel)
- [ ] `[VALID]` `cargo build -p inference_ml --release` — compile proprement

### P10.3 — Intégration dans le workspace

- [ ] `[SETUP]` Vérifier que `inference_ml` est bien dans le `[workspace]` racine
- [ ] `[SETUP]` Documenter dans `inference_ml/README.md` : comment générer le modèle mock, comment lancer les tests, comment brancher sur `pipeline_dsp`
- [ ] `[TEST-I]` **Test d'intégration finale workspace :** Depuis un crate `integration_test` factice : simuler `pipeline_dsp → inference_ml` — envoyer des matrices MFCC synthétiques, vérifier que des scores sont reçus — sans erreur de compilation ni de runtime

---

## Récapitulatif par partie

| Partie | Thème | Dépend de |
|---|---|---|
| P0 | Installation & Setup | — |
| P1 | Erreurs & Config | P0 |
| P2 | Génération du modèle mock | P0 |
| P3 | Bindings FFI Rust | P1 |
| P4 | Bridge Objective-C++ | P0, P3 |
| P5 | Wrapper `CoreMLModel` | P3, P4 |
| P6 | Thread d'inférence | P5 |
| P7 | Façade publique | P5, P6 |
| P8 | Validation ANE & performance | P7 |
| P9 | Suite de tests complète | P1→P7 |
| P10 | Documentation & Livraison | P9 |
