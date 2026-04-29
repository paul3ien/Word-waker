# Stack Technique — Module `inference_ml`

> Ce fichier documente les technologies, dépendances et contraintes propres au module d'inférence ML.
> Le module reçoit une matrice MFCC `[[f32;13];98]` depuis `pipeline_dsp` et retourne un score de probabilité `f32` vers `trigger`.
> Il doit être utilisable de manière **autonome** (matrice synthétique en entrée) et **intégré** dans le pipeline complet.

---

## Architecture de l'étage — clarification critique

> **L'inférence ne passe PAS par un socket.** Le modèle s'exécute **dans le même processus** que le daemon Rust,
> via un bridge Objective-C++ compilé en bibliothèque statique (`.a`) et linké directement dans le binaire final.
>
> - **Offline (hors runtime)** : Python + PyTorch → `coremltools` → `.mlpackage` → `xcrun coremlcompiler` → `.mlmodelc`
> - **Runtime (in-process)** : `Rust` → `extern "C"` FFI → `coreml_bridge.mm` (`.a` statique) → `CoreML.framework` → ANE

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
| Rust | 1.78+ | 🔴 CRITIQUE | Cœur du module — FFI, gestion du cycle de vie du handle Core ML |
| Cargo | (bundled) | 🔴 CRITIQUE | Build system, compilation du bridge Obj-C++ via `cc` crate |
| `cc` crate (build-dep) | 1.0+ | 🔴 CRITIQUE | Compile `coreml_bridge.mm` en `libcoreml_bridge.a` et le linke statiquement |
| Clang++ (Xcode CLT) | (bundled) | 🔴 CRITIQUE | Seul compilateur capable de compiler l'Objective-C++ sur macOS |
| Target `aarch64-apple-darwin` | macOS 14+ | 🔴 CRITIQUE | ANE uniquement disponible sur Apple Silicon — pas de fallback x86 |
| `-fobjc-arc` | — | 🔴 CRITIQUE | Automatic Reference Counting — obligatoire pour la gestion mémoire des objets Obj-C |
| `-std=c++20` | — | 🟠 OBLIGATOIRE | Standard C++ du bridge |
| `-mmacosx-version-min=14.0` | — | 🟠 OBLIGATOIRE | Cible macOS 14+ pour les APIs Core ML récentes |
| Edition Rust 2021 | — | 🟠 OBLIGATOIRE | Resolver v2 |

---

## 2. Frameworks Apple (linkage statique dans le binaire final)

| Framework | Niveau | Justification |
|---|---|---|
| `CoreML.framework` | 🔴 CRITIQUE | Framework principal — `MLModel`, `MLMultiArray`, `MLDictionaryFeatureProvider`, `MLFeatureValue` |
| `Foundation.framework` | 🔴 CRITIQUE | Requis par Core ML — `NSString`, `NSURL`, `NSArray`, `NSError`, `NSDictionary` |
| `CoreFoundation.framework` (implicite) | 🟠 OBLIGATOIRE | `CFBridgingRetain` / `CFBridgingRelease` — gestion du handle opaque entre ARC et Rust |

---

## 3. Bridge Objective-C++ (`coreml_bridge.mm`)

> Surface minimale — objectif < 200 lignes. N'expose que trois fonctions C vers Rust.

### 3.1 API C exposée au Rust

| Fonction C | Niveau | Description |
|---|---|---|
| `coreml_load(path: *const c_char) -> *mut c_void` | 🔴 CRITIQUE | Charge le `.mlmodelc`, configure `MLComputeUnitsAll` (CPU + GPU + ANE), retourne un handle opaque |
| `coreml_infer(handle, mfcc_flat: *const f32, len: usize) -> f32` | 🔴 CRITIQUE | Construit `MLMultiArray` [1,1,98,13], appelle `predictionFromFeatures`, retourne le score wake-word |
| `coreml_free(handle: *mut c_void)` | 🔴 CRITIQUE | Libère le `MLModel` via `CFBridgingRelease` |

### 3.2 Technologies internes au bridge

| Technologie Obj-C | Niveau | Rôle |
|---|---|---|
| `MLModel modelWithContentsOfURL:configuration:error:` | 🔴 CRITIQUE | Chargement du modèle compilé `.mlmodelc` |
| `MLModelConfiguration` + `MLComputeUnitsAll` | 🔴 CRITIQUE | Active l'ANE — sans cette option, l'inférence reste sur CPU |
| `MLMultiArray initWithShape:dataType:error:` | 🔴 CRITIQUE | Conteneur d'entrée — shape `[1, 1, 98, 13]`, type `Float32` |
| `memcpy` sur `MLMultiArray.dataPointer` | 🔴 CRITIQUE | Copie du tableau MFCC Rust dans le buffer Core ML |
| `MLDictionaryFeatureProvider initWithDictionary:error:` | 🔴 CRITIQUE | Wrapping de l'entrée pour l'API `predictionFromFeatures` |
| `MLFeatureValue featureValueWithMultiArray:` | 🔴 CRITIQUE | Encapsulation du `MLMultiArray` |
| `predictionFromFeatures:error:` | 🔴 CRITIQUE | Appel d'inférence synchrone — délègue à l'ANE si disponible |
| `featureValueForName:@"classLabel_probs"` | 🔴 CRITIQUE | Récupère les probabilités de sortie du modèle |
| `CFBridgingRetain` / `CFBridgingRelease` | 🔴 CRITIQUE | Passage du `MLModel` ARC entre Objective-C et Rust sans fuite mémoire |

---

## 4. Modèle ML — Format et contraintes

### 4.1 Format du modèle (artifact offline)

| Paramètre | Valeur | Niveau | Justification |
|---|---|---|---|
| Format source | `.mlpackage` | 🔴 CRITIQUE | Format de sortie de `coremltools` — contient le modèle et ses métadonnées |
| Format déployé | `.mlmodelc` | 🔴 CRITIQUE | Format binaire compilé par `xcrun coremlcompiler` — seul format lisible par `MLModel` |
| Type de quantification | Float16 + palettisation 8-bit (`DEFAULT_PALETTIZATION`) | 🔴 CRITIQUE | L'ANE Apple Silicon n'accepte pas Int8 — Float16 obligatoire |
| Taille cible | < 1 Mo | 🟠 OBLIGATOIRE | Embarqué dans le binaire ou le bundle |
| Compute units | `MLComputeUnitsAll` | 🔴 CRITIQUE | Permet à Core ML de router vers ANE, GPU ou CPU selon disponibilité |

### 4.2 Interface du modèle (contrat d'entrée/sortie)

| Élément | Valeur | Niveau | Justification |
|---|---|---|---|
| Nom de l'entrée | `"mfcc_input"` | 🔴 CRITIQUE | Nom défini lors de l'export `coremltools` — doit correspondre exactement |
| Shape d'entrée | `[1, 1, 98, 13]` | 🔴 CRITIQUE | Batch × Canaux × Trames × Coefficients — imposé par l'architecture CNN |
| Type d'entrée | `Float32` (côté Core ML) | 🔴 CRITIQUE | `MLMultiArrayDataTypeFloat32` |
| Nom de la sortie | `"classLabel_probs"` | 🔴 CRITIQUE | Vecteur de probabilités softmax — index 1 = wake-word |
| Nombre de classes | 2 | 🔴 CRITIQUE | `[background, wake-word]` |
| Index wake-word | 1 | 🔴 CRITIQUE | Convention d'entraînement — fixée lors de l'export |

### 4.3 Architecture CNN (référence pour la validation)

| Couche | Paramètres | Niveau |
|---|---|---|
| `Conv2d(1→32, 3×3, padding=1)` + BN + ReLU | Input: [1,1,98,13] | 🟡 IMPORTANT |
| `Conv2d(32→32, 3×3, groups=32)` (depthwise) | Separable MobileNet-style | 🟡 IMPORTANT |
| `Conv2d(32→64, 1×1)` (pointwise) + BN + ReLU | — | 🟡 IMPORTANT |
| `AdaptiveAvgPool2d(4,4)` + Flatten | — | 🟡 IMPORTANT |
| `Linear(1024→64)` + ReLU + Dropout(0.3) | — | 🟡 IMPORTANT |
| `Linear(64→2)` + Softmax | Sortie: [1,2] | 🟡 IMPORTANT |

---

## 5. Côté Rust — FFI et wrapper `CoreMLModel`

| Symbole Rust | Niveau | Description |
|---|---|---|
| `extern "C"` block avec les 3 fonctions bridge | 🔴 CRITIQUE | Déclaration des signatures FFI — doit correspondre exactement au `.mm` |
| `struct CoreMLModel { handle: *mut c_void }` | 🔴 CRITIQUE | Wrapper Rust autour du handle opaque |
| `unsafe impl Send for CoreMLModel` | 🔴 CRITIQUE | `MLModel` est thread-safe en lecture — requis pour usage multi-thread |
| `unsafe impl Sync for CoreMLModel` | 🔴 CRITIQUE | Idem — permet le partage du modèle entre threads |
| `CoreMLModel::load(path: &str) -> Result<Self, InferenceError>` | 🔴 CRITIQUE | Appelle `coreml_load`, vérifie que le handle n'est pas null |
| `CoreMLModel::infer(mfcc: &[[f32;13];98]) -> f32` | 🔴 CRITIQUE | Aplatit la matrice, appelle `coreml_infer` |
| `Drop for CoreMLModel` | 🔴 CRITIQUE | Appelle `coreml_free` — évite la fuite du `MLModel` ARC |
| `std::ffi::CString::new(path)` | 🔴 CRITIQUE | Conversion du path Rust en `*const c_char` null-terminé pour le bridge |

---

## 6. Dépendances Cargo (module `inference_ml`)

| Crate | Version | Niveau | Rôle |
|---|---|---|---|
| `libc` | 0.2+ | 🔴 CRITIQUE | `c_void`, `c_char`, `size_t` — types FFI pour le bridge |
| `anyhow` | 1.0+ | 🟠 OBLIGATOIRE | Propagation d'erreurs pour le chargement et l'inférence |
| `tracing` | 0.1+ | 🟡 IMPORTANT | Logs : temps d'inférence, score retourné, erreurs Core ML |
| `crossbeam-channel` | 0.5+ | 🟠 OBLIGATOIRE | Réception des matrices MFCC, émission des scores vers `trigger` |
| `cc` (build-dep) | 1.0+ | 🔴 CRITIQUE | Compilation du bridge `.mm` → `.a` |

---

## 7. Structure du module (organisation des fichiers)

| Fichier | Niveau | Rôle |
|---|---|---|
| `src/inference_ml/mod.rs` | 🔴 CRITIQUE | Point d'entrée public, re-exports, struct `InferenceEngine` |
| `src/inference_ml/error.rs` | 🔴 CRITIQUE | `InferenceError` : `ModelNotFound`, `LoadFailed`, `InferenceFailed`, `NullHandle`, `InvalidInputShape` |
| `src/inference_ml/ffi.rs` | 🔴 CRITIQUE | Déclarations `extern "C"` des 3 fonctions bridge |
| `src/inference_ml/model.rs` | 🔴 CRITIQUE | Struct `CoreMLModel` — wrapper Rust du handle opaque, `load`, `infer`, `Drop` |
| `src/inference_ml/runner.rs` | 🟠 OBLIGATOIRE | Thread d'inférence : reçoit matrices MFCC, émet scores `f32` |
| `src/bridge/coreml_bridge.mm` | 🔴 CRITIQUE | Bridge Objective-C++ — les 3 fonctions C compilées en `libcoreml_bridge.a` |
| `build.rs` | 🔴 CRITIQUE | Compile le `.mm`, linke `CoreML`, `Foundation`, `Accelerate` |
| `tests/inference_ml_integration.rs` | 🟡 IMPORTANT | Tests d'intégration (avec modèle mock ou modèle réel) |
| `fixtures/mock_model/` | 🟠 OBLIGATOIRE | Modèle `.mlmodelc` minimaliste pour les tests sans entraînement réel |

---

## 8. Interface publique du module (contrat)

| Symbole | Niveau | Description |
|---|---|---|
| `InferenceEngine::new(model_path: &str) -> Result<Self, InferenceError>` | 🔴 CRITIQUE | Charge le `.mlmodelc` et initialise le wrapper |
| `InferenceEngine::infer(mfcc: &[[f32;13];98]) -> Result<f32, InferenceError>` | 🔴 CRITIQUE | Retourne le score de probabilité wake-word ∈ [0.0, 1.0] |
| `InferenceEngine::start(rx, tx)` | 🟠 OBLIGATOIRE | Démarre le thread d'inférence |
| `InferenceEngine::stop()` | 🟠 OBLIGATOIRE | Arrête proprement le thread |
| `InferenceError` (enum) | 🔴 CRITIQUE | Erreurs typées couvrant chargement et inférence |
| `Drop for InferenceEngine` | 🟠 OBLIGATOIRE | `stop()` + libération du handle Core ML |

---

## 9. Contraintes de qualité & métriques du module

| Métrique | Objectif | Niveau |
|---|---|---|
| Latence d'inférence (ANE) | < 5 ms par appel | 🔴 CRITIQUE |
| Latence d'inférence (CPU fallback) | < 50 ms | 🟠 OBLIGATOIRE |
| Utilisation ANE | Vérifiée via `Instruments → Neural Engine` | 🔴 CRITIQUE |
| Zéro fuite mémoire sur le handle `MLModel` | Zéro fuite (AddressSanitizer) | 🔴 CRITIQUE |
| Score de sortie dans [0.0, 1.0] | Invariant de contrat | 🔴 CRITIQUE |
| Compilable et testable sans `pipeline_dsp` ni `trigger` | Obligatoire | 🔴 CRITIQUE |
| Thread-safety : inférence concurrente | `Send + Sync` sur `CoreMLModel` | 🔴 CRITIQUE |

---

## 10. Outillage de test & profiling

| Outil | Niveau | Usage |
|---|---|---|
| `cargo test` | 🔴 CRITIQUE | Tests unitaires et d'intégration |
| `cargo test --features mock_model` | 🔴 CRITIQUE | Tests sans modèle entraîné réel — modèle factice retournant 0.5 |
| Modèle `.mlmodelc` minimal (`fixtures/mock_model/`) | 🔴 CRITIQUE | Permet les tests d'intégration du cycle load/infer/free sans PyTorch |
| `AddressSanitizer` | 🟠 OBLIGATOIRE | Valider zéro fuite du handle ARC bridgé |
| `Instruments.app → Neural Engine` | 🔴 CRITIQUE | Confirmer que l'ANE est effectivement sollicité (pas seulement CPU) |
| `Instruments.app → Allocations` | 🟠 OBLIGATOIRE | Vérifier l'empreinte mémoire du modèle chargé |
| `cargo-criterion` | 🟡 IMPORTANT | Benchmark de la latence d'inférence |

---

## 11. Chaîne offline — Entraînement & Conversion (hors scope runtime)

> Cette partie concerne les outils offline, exécutés une seule fois pour produire le `.mlmodelc`.
> Elle n'est **pas** une dépendance runtime du module.

| Outil | Version | Niveau | Rôle |
|---|---|---|---|
| Python | 3.10+ | 🟠 OBLIGATOIRE | Environnement d'entraînement |
| PyTorch | 2.x | 🟠 OBLIGATOIRE | Entraînement du CNN wake-word |
| `coremltools` | 8.x | 🟠 OBLIGATOIRE | Conversion PyTorch → `.mlpackage` avec Float16 + `DEFAULT_PALETTIZATION` |
| `xcrun coremlcompiler` | (Xcode CLT) | 🔴 CRITIQUE | Compilation `.mlpackage` → `.mlmodelc` — artifact déployé dans le daemon |
| Google Speech Commands dataset | — | 🟡 IMPORTANT | Dataset public pour pré-entraînement (yes/no) avant fine-tuning wake-word |

---

## 12. Contraintes d'intégration dans le pipeline complet

| Contrainte | Niveau | Description |
|---|---|---|
| Interface entrée : `Receiver<[[f32;13];98]>` | 🔴 CRITIQUE | Reçoit les matrices MFCC depuis `pipeline_dsp` |
| Interface sortie : `Sender<f32>` | 🔴 CRITIQUE | Émet les scores de probabilité vers `trigger` |
| Pas de dépendance vers `audio_capture`, `pipeline_dsp`, `trigger` | 🔴 CRITIQUE | Découplage strict |
| Le chemin du `.mlmodelc` est configuré au démarrage | 🟠 OBLIGATOIRE | Passé via `InferenceEngine::new(path)` — peut venir d'un arg CLI ou d'une config |
| Pas de rechargement du modèle en cours d'exécution | 🟡 IMPORTANT | Le modèle est chargé une fois dans `new`, réutilisé pour toute la durée de vie |
| Feature flag `mock_model` | 🔴 CRITIQUE | Active un stub Rust qui retourne toujours 0.5 — tests sans `.mlmodelc` sur CI |
