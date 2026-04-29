# Stack Technique — Module `pipeline_dsp`

> Ce fichier documente les technologies, dépendances et contraintes propres au module de traitement du signal (DSP).
> Le module consomme des samples PCM bruts depuis `audio_capture` et produit des matrices MFCC prêtes pour l'inférence ML.
> Il doit être utilisable de manière **autonome** (entrée simulée, sortie loggée) et **intégré** dans le pipeline complet.

---

## Légende des niveaux d'importance

| Niveau | Signification |
|--------|---------------|
| 🔴 CRITIQUE | Bloquant — le module ne peut pas fonctionner sans |
| 🟠 OBLIGATOIRE | Requis pour respecter les contraintes de performance et de fidélité du signal |
| 🟡 IMPORTANT | Fortement recommandé, contournement possible à court terme uniquement |
| 🟢 OPTIONNEL | Amélioration ou outillage, non bloquant |

---

## 1. Langage & Toolchain

| Technologie | Version min | Niveau | Justification |
|---|---|---|---|
| Rust | 1.78+ | 🔴 CRITIQUE | Langage principal — calcul numérique, ownership strict sur les buffers intermédiaires |
| Cargo | (bundled) | 🔴 CRITIQUE | Build system, feature flags, benchmark runner |
| Target `aarch64-apple-darwin` | macOS 14+ | 🔴 CRITIQUE | Seule cible — optimisations SIMD NEON via Accelerate activées uniquement sur Apple Silicon |
| Edition Rust 2021 | — | 🟠 OBLIGATOIRE | Resolver v2, syntaxe moderne |
| `rustfmt` | (stable) | 🟡 IMPORTANT | Formatage — le DSP produit beaucoup de code numérique dense |
| `clippy` | (stable) | 🟡 IMPORTANT | Linting — détecter les calculs en virgule flottante potentiellement incorrects |

---

## 2. Bibliothèque de calcul numérique — `Accelerate.framework`

| Technologie | Niveau | Justification |
|---|---|---|
| `Accelerate.framework` | 🔴 CRITIQUE | Bibliothèque C Apple pré-installée sur tout Mac — expose vDSP, BLAS, LAPACK optimisés SIMD ARM |
| `vDSP_vmul` | 🔴 CRITIQUE | Multiplication vecteur-vecteur SIMD — utilisé pour l'application de la fenêtre de Hann |
| `vDSP_create_fftsetup` / `vDSP_fft_zrip` | 🔴 CRITIQUE | FFT rapide radix-2 — cœur du calcul spectral |
| `vDSP_zvmags` | 🔴 CRITIQUE | Calcul des magnitudes du spectre complexe après FFT |
| `vDSP_destroy_fftsetup` | 🔴 CRITIQUE | Libération du setup FFT — doit être appelé dans `Drop` |
| `vDSP_DCT_CreateSetup` / `vDSP_DCT_Execute` | 🔴 CRITIQUE | DCT-II pour le calcul des MFCC depuis les log-énergies Mel |
| `cblas_sgemv` | 🔴 CRITIQUE | Produit matrice-vecteur BLAS — application du banc de filtres Mel (matrice 40×N_FFT) |
| Accès via `extern "C"` direct | 🔴 CRITIQUE | Pas de wrapper Obj-C — linkage direct depuis Rust via `build.rs` |
| `vDSP_vdbcon` | 🟠 OBLIGATOIRE | Conversion en dB des énergies (alternative à `f32::ln` en boucle) |
| `vDSP_vsadd` / `vDSP_vsmul` | 🟡 IMPORTANT | Opérations scalaires vectorisées utiles pour la normalisation |

---

## 3. Pipeline DSP — Étapes et technologies associées

### 3.1 Pré-accentuation (filtre IIR)

| Technologie | Niveau | Justification |
|---|---|---|
| Rust pur (itératif) | 🔴 CRITIQUE | Filtre IIR `y[n] = x[n] − α·x[n−1]` avec α=0.97 — trop simple pour FFI, pas de dépendance externe |
| Paramètre α configurable | 🟠 OBLIGATOIRE | Doit être dans la config pour faciliter les tests de régression |

### 3.2 Fenêtrage de Hann

| Technologie | Niveau | Justification |
|---|---|---|
| Coefficients pré-calculés au démarrage (`Vec<f32>`) | 🔴 CRITIQUE | Calcul unique au démarrage — zéro coût en régime permanent |
| `vDSP_vmul` (FFI Accelerate) | 🔴 CRITIQUE | Multiplication SIMD de la trame par les coefficients de fenêtre |
| Taille de trame : 400 samples (25 ms) | 🔴 CRITIQUE | Paramètre imposé par le modèle ML — ne pas modifier |
| Pas de trame : 160 samples (10 ms) | 🔴 CRITIQUE | Overlap 60% — produit 98 trames par seconde (≈1 s d'audio) |

### 3.3 FFT

| Technologie | Niveau | Justification |
|---|---|---|
| `vDSP_fft_zrip` — FFT réelle radix-2 | 🔴 CRITIQUE | FFT sur signaux réels — deux fois plus rapide que la FFT complexe générale |
| Taille FFT : 512 points (puissance de 2 ≥ 400) | 🔴 CRITIQUE | Contrainte du setup `vDSP_create_fftsetup` (log2n = 9) |
| `DSPSplitComplex` | 🔴 CRITIQUE | Format split complexe attendu par vDSP — parties réelle et imaginaire séparées |
| `vDSP_zvmags` | 🔴 CRITIQUE | Calcul des magnitudes |
| Setup FFT créé une seule fois (dans `new`) | 🟠 OBLIGATOIRE | Création du setup coûteuse — doit être réutilisée entre les trames |

### 3.4 Banc de filtres Mel

| Technologie | Niveau | Justification |
|---|---|---|
| 40 filtres triangulaires sur l'échelle Mel | 🔴 CRITIQUE | Paramètre imposé par le modèle ML |
| Conversion Hz → Mel : `m = 2595·log10(1 + f/700)` | 🔴 CRITIQUE | Formule standard — implémentée en Rust pur au démarrage |
| Matrice de filtres pré-calculée `[[f32; N_FFT/2]; 40]` | 🔴 CRITIQUE | Calcul unique au démarrage — réutilisée à chaque trame |
| `cblas_sgemv` (BLAS Accelerate) | 🔴 CRITIQUE | Application de la matrice sur le spectre de magnitude — produit matrice-vecteur |
| Fréquence min / max : 20 Hz / 8000 Hz | 🟠 OBLIGATOIRE | Plage spectrale standard pour la voix humaine |

### 3.5 MFCC via DCT-II

| Technologie | Niveau | Justification |
|---|---|---|
| `f32::ln` (Rust std) | 🔴 CRITIQUE | Logarithme naturel des énergies Mel — appliqué avant la DCT |
| `vDSP_DCT_Execute` (Accelerate) | 🔴 CRITIQUE | DCT-II vectorisée sur les 40 log-énergies Mel |
| 13 coefficients MFCC retenus (sur 40) | 🔴 CRITIQUE | Standard de facto pour la reconnaissance vocale — imposé par le modèle ML |
| Setup DCT créé une seule fois (dans `new`) | 🟠 OBLIGATOIRE | Même logique que le setup FFT |

---

## 4. Format de sortie du module

| Paramètre | Valeur | Niveau | Justification |
|---|---|---|---|
| Type de sortie | `[[f32; 13]; 98]` | 🔴 CRITIQUE | Matrice row-major contiguë — format exact attendu par le bridge Core ML |
| Nombre de trames | 98 | 🔴 CRITIQUE | ≈ 1 seconde d'audio à 10 ms de pas — fenêtre d'inférence du modèle |
| Nombre de coefficients | 13 | 🔴 CRITIQUE | Imposé par l'architecture du modèle CNN |
| Représentation mémoire | Tableau C contigu (`as_ptr()`) | 🔴 CRITIQUE | Doit être sérialisable sans copie pour le passage FFI au bridge Core ML |

---

## 5. Dépendances Cargo (module `pipeline_dsp`)

| Crate | Version | Niveau | Rôle |
|---|---|---|---|
| `libc` | 0.2+ | 🔴 CRITIQUE | Types C pour les bindings vDSP / BLAS : `c_void`, `c_float`, `c_int` |
| `anyhow` | 1.0+ | 🟠 OBLIGATOIRE | Propagation d'erreurs pour l'initialisation des setups FFT et DCT |
| `tracing` | 0.1+ | 🟡 IMPORTANT | Logs structurés (latence par trame, anomalies numériques) |
| `crossbeam-channel` | 0.5+ | 🟠 OBLIGATOIRE | Réception des batches PCM depuis `audio_capture`, émission des matrices MFCC |
| `approx` | 0.5+ | 🟡 IMPORTANT | Comparaisons flottantes dans les tests (`assert_abs_diff_eq!`) |
| `cc` (build-dep) | 1.0+ | 🔴 CRITIQUE | Linkage d'`Accelerate.framework` via `build.rs` |

---

## 6. Structure du module (organisation des fichiers)

| Fichier | Niveau | Rôle |
|---|---|---|
| `src/pipeline_dsp/mod.rs` | 🔴 CRITIQUE | Point d'entrée public, re-exports, struct `DspPipeline` |
| `src/pipeline_dsp/config.rs` | 🔴 CRITIQUE | `DspConfig` : tous les paramètres configurables (sample_rate, n_fft, n_mels, n_mfcc, etc.) |
| `src/pipeline_dsp/error.rs` | 🔴 CRITIQUE | `DspError` : `FftSetupFailed`, `DctSetupFailed`, `InvalidFrameSize`, `NumericalOverflow` |
| `src/pipeline_dsp/ffi.rs` | 🔴 CRITIQUE | Déclarations `extern "C"` de toutes les fonctions vDSP et BLAS utilisées |
| `src/pipeline_dsp/preemphasis.rs` | 🔴 CRITIQUE | Filtre IIR de pré-accentuation |
| `src/pipeline_dsp/framing.rs` | 🔴 CRITIQUE | Découpage du signal en trames avec overlap |
| `src/pipeline_dsp/windowing.rs` | 🔴 CRITIQUE | Fenêtre de Hann pré-calculée + application via vDSP_vmul |
| `src/pipeline_dsp/fft.rs` | 🔴 CRITIQUE | Wrapper `VDspFft` autour du setup vDSP + `forward()` |
| `src/pipeline_dsp/mel_filterbank.rs` | 🔴 CRITIQUE | Calcul de la matrice de filtres + `apply()` via cblas_sgemv |
| `src/pipeline_dsp/mfcc.rs` | 🔴 CRITIQUE | Log des énergies + DCT-II → coefficients MFCC |
| `src/pipeline_dsp/runner.rs` | 🟠 OBLIGATOIRE | Thread DSP : réception batches, accumulation, traitement, émission MFCC |
| `tests/pipeline_dsp_integration.rs` | 🟡 IMPORTANT | Tests d'intégration isolés (avec signal synthétique) |
| `build.rs` | 🔴 CRITIQUE | `cargo:rustc-link-lib=framework=Accelerate` |

---

## 7. Interface publique du module (contrat)

| Symbole | Niveau | Description |
|---|---|---|
| `DspPipeline::new(config: DspConfig) -> Result<Self, DspError>` | 🔴 CRITIQUE | Initialise tous les setups FFT/DCT, pré-calcule fenêtre Hann et filtres Mel |
| `DspPipeline::process(samples: &[f32]) -> Option<[[f32;13];98]>` | 🔴 CRITIQUE | Traite un batch de samples, retourne `Some(mfcc)` quand 98 trames sont accumulées |
| `DspPipeline::start(rx: Receiver<Vec<f32>>, tx: Sender<[[f32;13];98]>)` | 🟠 OBLIGATOIRE | Démarre le thread DSP : lit les batches PCM, émet les matrices MFCC |
| `DspPipeline::stop()` | 🟠 OBLIGATOIRE | Arrête proprement le thread DSP |
| `DspConfig` (struct) | 🔴 CRITIQUE | Paramètres : `sample_rate`, `frame_size`, `hop_size`, `n_fft`, `n_mels`, `n_mfcc`, `alpha` |
| `DspError` (enum) | 🔴 CRITIQUE | Erreurs typées couvrant toutes les étapes |
| `Drop for DspPipeline` | 🟠 OBLIGATOIRE | `stop()` automatique si non appelé manuellement |

---

## 8. Contraintes de qualité & métriques du module

| Métrique | Objectif | Niveau |
|---|---|---|
| Fidélité numérique vs librosa (Python) | Erreur < 1e-3 par coefficient MFCC | 🔴 CRITIQUE |
| Latence de traitement par seconde d'audio | < 10 ms (temps réel × 100) | 🟠 OBLIGATOIRE |
| Zéro allocation dynamique en régime permanent | Tous les buffers pré-alloués dans `new` | 🟠 OBLIGATOIRE |
| Aucune fuite mémoire sur les setups FFT/DCT | Zéro fuite (validé AddressSanitizer) | 🔴 CRITIQUE |
| Consommation CPU du module isolé en boucle continue | < 0,1 % | 🟠 OBLIGATOIRE |
| Compilable et testable sans `audio_capture` ni `inference` | Obligatoire | 🔴 CRITIQUE |
| Couverture de tests unitaires | > 85 % des fonctions DSP | 🟡 IMPORTANT |

---

## 9. Outillage de test & profiling

| Outil | Niveau | Usage |
|---|---|---|
| `cargo test` | 🔴 CRITIQUE | Tests unitaires et d'intégration |
| `cargo test --features mock_input` | 🟠 OBLIGATOIRE | Tests avec signal synthétique (sinus, silence, bruit blanc) sans `audio_capture` |
| Signal de référence librosa (Python) | 🔴 CRITIQUE | Fichier `.npy` ou `.json` pré-généré contenant les MFCC de référence pour la validation numérique |
| `approx::assert_abs_diff_eq!` | 🔴 CRITIQUE | Comparaisons flottantes précises dans les tests |
| `AddressSanitizer` | 🟠 OBLIGATOIRE | Détection de fuites sur les setups `c_void` FFT et DCT |
| `cargo-criterion` | 🟡 IMPORTANT | Benchmarks par étape (pré-accentuation, FFT, Mel, MFCC) |
| `cargo-flamegraph` | 🟢 OPTIONNEL | Flamegraphs pour identifier le goulot d'étranglement dans la chaîne |
| `Instruments.app` → CPU Profiler | 🟡 IMPORTANT | Validation en conditions réelles avec `audio_capture` branché |

---

## 10. Contraintes d'intégration dans le pipeline complet

| Contrainte | Niveau | Description |
|---|---|---|
| Interface entrée : `Receiver<Vec<f32>>` | 🔴 CRITIQUE | Reçoit les batches PCM depuis le thread consommateur de `audio_capture` |
| Interface sortie : `Sender<[[f32;13];98]>` | 🔴 CRITIQUE | Émet les matrices MFCC vers le module `inference` |
| Pas de dépendance vers `audio_capture`, `inference`, `trigger` | 🔴 CRITIQUE | Découplage strict — le module ne connaît que ses types d'entrée/sortie |
| Accumulation interne sur une fenêtre glissante de 98 trames | 🟠 OBLIGATOIRE | Le module maintient un état interne (`VecDeque` de trames) pour émettre à la bonne fréquence |
| Feature flag `standalone` | 🟠 OBLIGATOIRE | Permet un binaire d'exemple autonome alimenté par un fichier WAV ou un signal synthétique |
| Compatibilité du format de sortie avec le bridge Core ML | 🔴 CRITIQUE | `[[f32;13];98]` en mémoire contiguë row-major — vérifié par un test de layout mémoire |
