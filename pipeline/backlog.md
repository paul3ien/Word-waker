# Backlog — Module `pipeline_dsp`

> Découpage industriel des tâches élémentaires pour la conception, l'implémentation et la validation du module DSP.
> Chaque tâche est atomique, testable, et les tests s'incrémentent avec les fonctionnalités.
> Le module doit être **compilable, testable et exécutable de manière isolée** (sans `audio_capture` ni `inference`) à tout moment.

---

## Légende

| Symbole | Signification |
|---|---|
| `[SETUP]` | Infrastructure, environnement, configuration |
| `[IMPL]` | Implémentation d'une fonctionnalité |
| `[TEST-U]` | Test unitaire (logique pure, signal synthétique, sans hardware) |
| `[TEST-I]` | Test d'intégration (chaîne complète ou couplage avec un autre module) |
| `[TEST-N]` | Test de validation numérique (comparaison vs référence librosa) |
| `[TEST-P]` | Test de performance / benchmark |
| `[VALID]` | Validation manuelle ou instrumentée |
| `[ ]` | Non commencé |
| `[x]` | Terminé |

---

## PARTIE 0 — Installation & Configuration de l'environnement

> **Objectif :** Avoir un crate `pipeline_dsp` qui compile, linke `Accelerate.framework`, et passe un `cargo check` propre.

---

### P0.1 — Vérification des prérequis système

- [x] `[SETUP]` Vérifier la présence d'`Accelerate.framework` : `ls /Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/System/Library/Frameworks/Accelerate.framework`
- [x] `[SETUP]` Vérifier la présence des headers vDSP : `vDSP.h`, `cblas.h` dans le SDK
- [x] `[SETUP]` Vérifier que Python 3.10+ et `librosa` sont disponibles (nécessaire pour générer les références numériques de validation)
- [x] `[TEST-U]` **Test de smoke :** Compiler un programme C minimal qui inclut `<Accelerate/Accelerate.h>` — valide que les headers sont accessibles

### P0.2 — Création de la structure du crate

- [x] `[SETUP]` Créer (ou confirmer) le crate `pipeline_dsp` comme library crate dans le workspace
- [x] `[SETUP]` Créer l'arborescence : `src/pipeline_dsp/`, `tests/`, `benches/`, `fixtures/` (pour les données de référence)
- [x] `[SETUP]` Créer le `build.rs` avec `println!("cargo:rustc-link-lib=framework=Accelerate")`
- [x] `[TEST-U]` **Test de smoke :** `cargo check -p pipeline_dsp` — doit passer sans erreur

### P0.3 — Configuration des dépendances

- [x] `[SETUP]` Ajouter `libc = "0.2"` dans `[dependencies]`
- [x] `[SETUP]` Ajouter `anyhow = "1.0"` dans `[dependencies]`
- [x] `[SETUP]` Ajouter `crossbeam-channel = "0.5"` dans `[dependencies]`
- [x] `[SETUP]` Ajouter `tracing = "0.1"` et `tracing-subscriber = "0.3"` dans `[dependencies]`
- [x] `[SETUP]` Ajouter `approx = "0.5"` dans `[dev-dependencies]` (pour les comparaisons flottantes dans les tests)
- [x] `[SETUP]` Ajouter `criterion = "0.5"` dans `[dev-dependencies]` avec `[[bench]]` dans `Cargo.toml`
- [x] `[TEST-U]` **Test :** `cargo build -p pipeline_dsp` — doit compiler et linker Accelerate sans erreur

### P0.4 — Feature flags & mode standalone

- [x] `[SETUP]` Déclarer feature `mock_input` — active un générateur de signal synthétique (sinus, silence, bruit blanc) à la place du `Receiver<Vec<f32>>`
- [x] `[SETUP]` Déclarer feature `standalone` — active un `example` exécutable en autonomie
- [x] `[SETUP]` Générer les fichiers de référence numérique : script Python `scripts/generate_mfcc_reference.py` qui lit un signal WAV synthétique et écrit la matrice MFCC de référence dans `fixtures/reference_mfcc.json`
- [x] `[TEST-U]` **Test :** `cargo build -p pipeline_dsp --features mock_input,standalone` — doit compiler

---

## PARTIE 1 — Gestion des erreurs et configuration

> **Objectif :** Définir les contrats de configuration et d'erreur avant toute implémentation DSP.

---

### P1.1 — Type d'erreur du module

- [x] `[IMPL]` Créer `src/pipeline_dsp/error.rs`
- [x] `[IMPL]` Définir l'enum `DspError` avec les variantes : `FftSetupFailed`, `DctSetupFailed`, `InvalidFrameSize { expected: usize, got: usize }`, `InvalidSampleRate(f64)`, `NumericalOverflow { step: &'static str }`, `ChannelClosed`
- [x] `[IMPL]` Implémenter `std::fmt::Display` pour `DspError`
- [x] `[IMPL]` Implémenter `std::error::Error` pour `DspError`
- [x] `[TEST-U]` **Test unitaire :** Chaque variante produit un message `Display` non vide et distinct
- [x] `[TEST-U]` **Test unitaire :** `DspError` est `Send + Sync`

### P1.2 — Configuration du pipeline DSP

- [x] `[IMPL]` Créer `src/pipeline_dsp/config.rs`
- [x] `[IMPL]` Définir la struct `DspConfig` avec les champs : `sample_rate: f64`, `frame_size: usize`, `hop_size: usize`, `n_fft: usize`, `n_mels: usize`, `n_mfcc: usize`, `alpha: f32`, `mel_fmin: f32`, `mel_fmax: f32`, `n_frames: usize`
- [x] `[IMPL]` Implémenter `Default` : `sample_rate=16000.0`, `frame_size=400`, `hop_size=160`, `n_fft=512`, `n_mels=40`, `n_mfcc=13`, `alpha=0.97`, `mel_fmin=20.0`, `mel_fmax=8000.0`, `n_frames=98`
- [x] `[IMPL]` Implémenter `validate(&self) -> Result<(), DspError>` : vérifier que `n_fft` est une puissance de 2, `n_fft >= frame_size`, `n_mfcc <= n_mels`, `mel_fmax <= sample_rate/2`
- [x] `[TEST-U]` **Test unitaire :** `DspConfig::default().validate()` retourne `Ok`
- [x] `[TEST-U]` **Test unitaire :** `n_fft = 300` (pas puissance de 2) → `validate()` retourne `Err`
- [x] `[TEST-U]` **Test unitaire :** `n_mfcc = 50`, `n_mels = 40` → `validate()` retourne `Err`
- [x] `[TEST-U]` **Test unitaire :** `mel_fmax = 9000.0`, `sample_rate = 16000.0` → `validate()` retourne `Err`

---

## PARTIE 2 — Bindings FFI Accelerate

> **Objectif :** Déclarer toutes les fonctions C nécessaires avant d'en avoir besoin, et vérifier le linkage.

---

### P2.1 — Déclarations FFI vDSP

- [x] `[IMPL]` Créer `src/pipeline_dsp/ffi.rs`
- [x] `[IMPL]` Déclarer la constante `kFFTRadix2: i32 = 0`
- [x] `[IMPL]` Déclarer la constante `kFFTDirection_Forward: i32 = 1`
- [x] `[IMPL]` Déclarer la struct `DSPSplitComplex { realp: *mut f32, imagp: *mut f32 }`
- [x] `[IMPL]` Déclarer `extern "C"` : `vDSP_create_fftsetup(log2n: u32, radix: i32) -> *mut c_void`
- [x] `[IMPL]` Déclarer `extern "C"` : `vDSP_fft_zrip(setup: *mut c_void, signal: *mut DSPSplitComplex, stride: u32, log2n: u32, direction: i32)`
- [x] `[IMPL]` Déclarer `extern "C"` : `vDSP_zvmags(input: *const DSPSplitComplex, i_stride: u32, output: *mut f32, o_stride: u32, n: u32)`
- [x] `[IMPL]` Déclarer `extern "C"` : `vDSP_vmul(a: *const f32, ia: u32, b: *const f32, ib: u32, c: *mut f32, ic: u32, n: u32)`
- [x] `[IMPL]` Déclarer `extern "C"` : `vDSP_destroy_fftsetup(setup: *mut c_void)`
- [x] `[TEST-U]` **Test de linkage :** Appeler `vDSP_create_fftsetup(9, 0)` puis `vDSP_destroy_fftsetup` dans un test — doit s'exécuter sans segfault

### P2.2 — Déclarations FFI DCT et BLAS

- [x] `[IMPL]` Déclarer la constante `vDSP_DCT_II: i32 = 2`
- [x] `[IMPL]` Déclarer `extern "C"` : `vDSP_DCT_CreateSetup(prev: *mut c_void, n: u32, dct_type: i32) -> *mut c_void`
- [x] `[IMPL]` Déclarer `extern "C"` : `vDSP_DCT_Execute(setup: *mut c_void, input: *const f32, output: *mut f32)`
- [x] `[IMPL]` Déclarer `extern "C"` : `cblas_sgemv(order: i32, trans: i32, m: i32, n: i32, alpha: f32, a: *const f32, lda: i32, x: *const f32, incx: i32, beta: f32, y: *mut f32, incy: i32)`
- [x] `[IMPL]` Déclarer les constantes BLAS : `CblasRowMajor: i32 = 101`, `CblasNoTrans: i32 = 111`
- [x] `[TEST-U]` **Test de linkage DCT :** Créer et libérer un setup DCT de taille 40 — sans segfault
- [x] `[TEST-U]` **Test de linkage BLAS :** Appeler `cblas_sgemv` sur des vecteurs triviaux (1×1) — vérifier le résultat `2×3=6`

---

## PARTIE 3 — Pré-accentuation

> **Objectif :** Implémenter et valider le filtre IIR `y[n] = x[n] − α·x[n−1]`.

---

### P3.1 — Implémentation

- [x] `[IMPL]` Créer `src/pipeline_dsp/preemphasis.rs`
- [x] `[IMPL]` Définir la struct `PreEmphasis { alpha: f32, last_sample: f32 }`
- [x] `[IMPL]` Implémenter `PreEmphasis::new(alpha: f32) -> Self`
- [x] `[IMPL]` Implémenter `apply(&mut self, frame: &mut [f32])` — traitement in-place, maintient `last_sample` entre les appels
- [x] `[IMPL]` Implémenter `reset(&mut self)` — remet `last_sample` à 0.0

### P3.2 — Tests

- [x] `[TEST-U]` **Test unitaire :** Signal constant [1.0, 1.0, 1.0] avec α=0.97 → vérifier les 3 premières sorties manuellement : `[1.0, 0.03, 0.03]`
- [x] `[TEST-U]` **Test unitaire :** Signal impulsion [1.0, 0.0, 0.0] → vérifier `[1.0, -0.97, 0.0]`
- [x] `[TEST-U]` **Test unitaire :** Signal silence [0.0, 0.0] → sortie [0.0, 0.0]
- [x] `[TEST-U]` **Test unitaire :** Vérifier que `last_sample` est bien propagé entre deux appels successifs à `apply` (continuité de traitement)
- [x] `[TEST-U]` **Test unitaire :** `reset()` remet `last_sample` à 0.0 — vérifier l'idempotence
- [ ] `[TEST-N]` **Validation numérique :** Comparer la sortie sur le signal de référence avec `librosa.effects.preemphasis` — erreur max < 1e-5

---

## PARTIE 4 — Découpage en trames (Framing)

> **Objectif :** Découper un flux de samples en trames de taille fixe avec un pas (overlap) configurable.

---

### P4.1 — Implémentation

- [x] `[IMPL]` Créer `src/pipeline_dsp/framing.rs`
- [x] `[IMPL]` Définir la struct `Framer { buffer: Vec<f32>, frame_size: usize, hop_size: usize }`
- [x] `[IMPL]` Implémenter `Framer::new(frame_size: usize, hop_size: usize) -> Self`
- [x] `[IMPL]` Implémenter `push_samples(&mut self, samples: &[f32]) -> Vec<Vec<f32>>` — accumule les samples dans le buffer interne et retourne toutes les trames complètes disponibles (avec overlap)
- [x] `[IMPL]` Implémenter `reset(&mut self)` — vide le buffer interne
- [x] `[IMPL]` Implémenter `pending_samples(&self) -> usize` — nombre de samples dans le buffer en attente

### P4.2 — Tests

- [x] `[TEST-U]` **Test unitaire :** Pousser exactement 400 samples → 1 trame retournée, buffer résiduel de 0 samples (hop=160, premier appel)
- [x] `[TEST-U]` **Test unitaire :** Pousser 160 samples puis 400 samples → vérifier le nombre correct de trames produites
- [x] `[TEST-U]` **Test unitaire :** Pousser 10 000 samples d'un coup → vérifier que le nombre de trames est `floor((10000 - 400) / 160) + 1`
- [x] `[TEST-U]` **Test unitaire :** Vérifier que les trames sont bien de taille `frame_size` (pas de trame partielle)
- [x] `[TEST-U]` **Test unitaire :** Vérifier l'overlap — le premier sample de la trame N+1 est bien le sample `hop_size` de la trame N
- [x] `[TEST-U]` **Test unitaire :** `reset()` — vider le buffer, vérifier `pending_samples() == 0`

---

## PARTIE 5 — Fenêtrage de Hann

> **Objectif :** Appliquer une fenêtre de Hann sur chaque trame via multiplication SIMD (`vDSP_vmul`).

---

### P5.1 — Implémentation

- [x] `[IMPL]` Créer `src/pipeline_dsp/windowing.rs`
- [x] `[IMPL]` Définir la struct `HannWindow { coefficients: Vec<f32>, size: usize }`
- [x] `[IMPL]` Implémenter `HannWindow::new(size: usize) -> Self` — calcule les coefficients `w[n] = 0.5 * (1 − cos(2π·n / (N−1)))` en Rust pur et les stocke
- [x] `[IMPL]` Implémenter `apply(&self, frame: &mut [f32])` — multiplie la trame par les coefficients via `vDSP_vmul` (in-place, résultat dans `frame`)
- [x] `[TEST-U]` **Test unitaire :** `coefficients[0]` et `coefficients[N-1]` sont 0.0 (ou très proche)
- [x] `[TEST-U]` **Test unitaire :** `coefficients[N/2]` est 1.0 (symétrie et maximum)
- [x] `[TEST-U]` **Test unitaire :** Appliquer sur un signal constant [1.0 × 400] → vérifier que la sortie correspond aux coefficients de Hann (erreur < 1e-6)
- [ ] `[TEST-N]` **Validation numérique :** Comparer avec `np.hanning(400)` (Python) — erreur max < 1e-6 par coefficient

---

## PARTIE 6 — FFT

> **Objectif :** Calculer le spectre de magnitude d'une trame via `vDSP_fft_zrip`.

---

### P6.1 — Wrapper `VDspFft`

- [x] `[IMPL]` Créer `src/pipeline_dsp/fft.rs`
- [x] `[IMPL]` Définir la struct `VDspFft { setup: *mut c_void, n: usize, log2n: u32 }` — avec `unsafe impl Send`
- [x] `[IMPL]` Implémenter `VDspFft::new(n: usize) -> Result<Self, DspError>` — appelle `vDSP_create_fftsetup`, retourne `FftSetupFailed` si retour null
- [x] `[IMPL]` Implémenter `forward(&self, frame: &[f32]) -> Vec<f32>` — zero-pads jusqu'à `n_fft`, calcule FFT via `vDSP_fft_zrip`, appelle `vDSP_zvmags`, retourne le vecteur de magnitudes (`n_fft/2` éléments)
- [x] `[IMPL]` Implémenter `Drop for VDspFft` — appelle `vDSP_destroy_fftsetup`
- [x] `[TEST-U]` **Test unitaire :** `VDspFft::new(512)` ne retourne pas d'erreur
- [x] `[TEST-U]` **Test unitaire :** Signal silence (zéros) → toutes les magnitudes sont 0.0 (ou < 1e-10)
- [x] `[TEST-U]` **Test unitaire :** Signal sinusoïdal pur à 1000 Hz → pic de magnitude à la bin fréquentielle correspondante (vérifier l'index `round(1000 * 512 / 16000) = 32`)
- [x] `[TEST-U]` **Test unitaire :** Vérifier que la sortie a exactement `n_fft/2 = 256` éléments
- [ ] `[TEST-N]` **Validation numérique :** Comparer les magnitudes avec `np.abs(np.fft.rfft(frame, n=512))` (Python) — erreur relative < 1e-3 sur chaque bin
- [ ] `[TEST-U]` **Test Drop :** Créer une instance dans un scope et vérifier (avec AddressSanitizer) que le setup est libéré

---

## PARTIE 7 — Banc de filtres Mel

> **Objectif :** Appliquer 40 filtres triangulaires sur l'échelle Mel pour réduire le spectre à 40 énergies Mel.

---

### P7.1 — Calcul de la matrice de filtres

- [x] `[IMPL]` Créer `src/pipeline_dsp/mel_filterbank.rs`
- [x] `[IMPL]` Implémenter `hz_to_mel(hz: f32) -> f32` : `2595.0 * (1.0 + hz / 700.0).log10()`
- [x] `[IMPL]` Implémenter `mel_to_hz(mel: f32) -> f32` : `700.0 * (10.0_f32.powf(mel / 2595.0) - 1.0)`
- [x] `[IMPL]` Définir la struct `MelFilterbank { matrix: Vec<f32>, n_mels: usize, n_fft_bins: usize }` (matrice stockée row-major `n_mels × (n_fft/2)`)
- [x] `[IMPL]` Implémenter `MelFilterbank::new(config: &DspConfig) -> Self` — calcule les centres Mel linéairement espacés, puis les filtres triangulaires et stocke la matrice
- [x] `[TEST-U]` **Test unitaire :** `hz_to_mel(700.0)` retourne ≈ 781.17 (correction : `2595·log10(2) = 781.17`, la valeur 776.45 du backlog était erronée)
- [x] `[TEST-U]` **Test unitaire :** `mel_to_hz(hz_to_mel(1000.0))` ≈ 1000.0 (bijection)
- [x] `[TEST-U]` **Test unitaire :** Vérifier que la matrice a `n_mels × (n_fft/2) = 40 × 256 = 10240` éléments
- [x] `[TEST-U]` **Test unitaire :** Vérifier que chaque filtre (ligne de la matrice) est non négatif et que la somme des coefficients est > 0
- [ ] `[TEST-N]` **Validation numérique :** Comparer la matrice avec `librosa.filters.mel(sr=16000, n_fft=512, n_mels=40, fmin=20, fmax=8000)` — erreur max < 1e-4 par coefficient

### P7.2 — Application des filtres

- [x] `[IMPL]` Implémenter `apply(&self, magnitudes: &[f32]) -> Vec<f32>` — multiplie la matrice par le vecteur de magnitudes via `cblas_sgemv`, retourne un vecteur de 40 énergies Mel
- [x] `[TEST-U]` **Test unitaire :** Spectre plat [1.0 × 256] → vérifier que les 40 énergies sont > 0 et cohérentes avec les largeurs de bande
- [x] `[TEST-U]` **Test unitaire :** Spectre nul [0.0 × 256] → toutes les énergies sont 0.0
- [ ] `[TEST-N]` **Validation numérique :** Appliquer sur un spectre de référence et comparer avec `librosa` — erreur < 1e-3 par filtre

---

## PARTIE 8 — Log-énergies & MFCC via DCT-II

> **Objectif :** Calculer les 13 coefficients MFCC depuis les 40 log-énergies Mel.

---

### P8.1 — Log des énergies Mel

- [x] `[IMPL]` Créer `src/pipeline_dsp/mfcc.rs`
- [x] `[IMPL]` Implémenter `log_mel_energies(mel_energies: &mut [f32])` — applique `f32::ln` in-place, remplace les valeurs ≤ 0 par `f32::EPSILON` avant le log (évite `-inf`)
- [x] `[TEST-U]` **Test unitaire :** Énergie de 1.0 → ln(1.0) = 0.0
- [x] `[TEST-U]` **Test unitaire :** Énergie de 0.0 → `ln(EPSILON)` (pas de panique, pas de `-inf`)
- [x] `[TEST-U]` **Test unitaire :** Énergie négative → remplacée par `f32::EPSILON` avant le log

### P8.2 — Wrapper DCT-II (`VDspDct`)

- [x] `[IMPL]` Définir la struct `VDspDct { setup: *mut c_void, n: usize, n_padded: usize }` — avec `unsafe impl Send` (note: n_padded arrondit au prochain f·2^k valide pour vDSP)
- [x] `[IMPL]` Implémenter `VDspDct::new(n: usize) -> Result<Self, DspError>` — appelle `vDSP_DCT_CreateSetup`, retourne `DctSetupFailed` si null
- [x] `[IMPL]` Implémenter `execute(&self, input: &[f32], output: &mut [f32])` — appelle `vDSP_DCT_Execute` (avec zero-padding transparent si nécessaire)
- [x] `[IMPL]` Implémenter `Drop for VDspDct` — Apple ne documente pas `vDSP_DCT_DestroySetup`; noté dans le code
- [x] `[TEST-U]` **Test unitaire :** `VDspDct::new(40)` ne retourne pas d'erreur (arrondissement interne à 48 = 3·2^4)
- [x] `[TEST-U]` **Test unitaire :** DCT d'un signal impulsion [1, 0, 0, …] (N=16) → X[0]=1.0, X[8]≈√2/2
- [ ] `[TEST-N]` **Validation numérique :** Comparer avec `scipy.fft.dct(x, type=2, norm=None)` — erreur max < 1e-4

### P8.3 — Calcul des 13 coefficients MFCC

- [x] `[IMPL]` Définir la struct `MfccExtractor { dct: VDspDct, n_mfcc: usize, n_mels: usize }`
- [x] `[IMPL]` Implémenter `MfccExtractor::new(config: &DspConfig) -> Result<Self, DspError>`
- [x] `[IMPL]` Implémenter `extract(&self, log_mel: &[f32]) -> [f32; 13]` — exécute la DCT sur 40 entrées (zero-paddée à 48), retourne les 13 premiers coefficients
- [x] `[TEST-U]` **Test unitaire :** Signal log-Mel constant → MFCC[0] dominant (> 10), |MFCC[k]| < |MFCC[0]| pour k=1..12
- [ ] `[TEST-N]` **Validation numérique :** Comparer les 13 MFCC avec `librosa.feature.mfcc` sur le signal de référence — erreur max < 1e-2 par coefficient (tolérance plus large due aux différences de normalisation)

---

## PARTIE 9 — Chaîne DSP complète (intégration des étapes)

> **Objectif :** Assembler toutes les étapes en un processeur de trame unique, validé de bout en bout.

---

### P9.1 — Processeur de trame `FrameProcessor`

- [x] `[IMPL]` Créer `src/pipeline_dsp/processor.rs`
- [x] `[IMPL]` Définir la struct `FrameProcessor` qui agrège : `PreEmphasis`, `HannWindow`, `VDspFft`, `MelFilterbank`, `MfccExtractor`
- [x] `[IMPL]` Implémenter `FrameProcessor::new(config: &DspConfig) -> Result<Self, DspError>`
- [x] `[IMPL]` Implémenter `process_frame(&mut self, frame: &mut [f32]) -> [f32; 13]` — exécute dans l'ordre : pré-accentuation → fenêtrage → FFT → Mel → log → DCT → retourne 13 MFCC
- [x] `[TEST-U]` **Test unitaire :** Signal silence → MFCC[0] << 0, tous coefficients finis
- [x] `[TEST-U]` **Test unitaire :** Même frame traitée par deux processeurs frais → résultats identiques
- [ ] `[TEST-N]` **Validation numérique de bout en bout :** Traiter 98 trames du signal de référence et comparer la matrice MFCC `[98×13]` avec `fixtures/reference_mfcc.json` — erreur max < 1e-2

### P9.2 — Accumulateur de trames `MfccAccumulator`

- [x] `[IMPL]` Définir la struct `MfccAccumulator { frames: VecDeque<[f32;13]>, capacity: usize }`
- [x] `[IMPL]` Implémenter `push(&mut self, mfcc: [f32;13])` — ajoute une trame, fait glisser si plein
- [x] `[IMPL]` Implémenter `is_ready(&self) -> bool` — retourne `true` si `frames.len() == capacity` (98)
- [x] `[IMPL]` Implémenter `get_matrix(&self) -> [[f32;13];98]` — retourne la matrice courante
- [x] `[TEST-U]` **Test unitaire :** Pousser 97 trames → `is_ready() == false`
- [x] `[TEST-U]` **Test unitaire :** Pousser 98 trames → `is_ready() == true`
- [x] `[TEST-U]` **Test unitaire :** Pousser 99 trames → `is_ready() == true` et la première trame a été évincée (fenêtre glissante)
- [x] `[TEST-U]` **Test unitaire :** Vérifier que la mémoire de `get_matrix()` est contiguë et row-major (`&matrix[0] as *const f32` adressable comme tableau C)

---

## PARTIE 10 — Façade publique & thread DSP

> **Objectif :** Exposer une API unifiée avec un thread dédié qui gère le cycle de vie complet.

---

### P10.1 — Façade `DspPipeline`

- [ ] `[IMPL]` Créer ou compléter `src/pipeline_dsp/mod.rs`
- [ ] `[IMPL]` Définir la struct `DspPipeline` qui agrège `Framer`, `FrameProcessor`, `MfccAccumulator`
- [ ] `[IMPL]` Implémenter `DspPipeline::new(config: DspConfig) -> Result<Self, DspError>`
- [ ] `[IMPL]` Implémenter `process_batch(&mut self, samples: &[f32]) -> Vec<[[f32;13];98]>` — traite un batch, retourne zéro ou plusieurs matrices MFCC selon l'accumulation

### P10.2 — Thread DSP runner

- [ ] `[IMPL]` Créer `src/pipeline_dsp/runner.rs`
- [ ] `[IMPL]` Définir la struct `DspRunner { pipeline: DspPipeline, running: Arc<AtomicBool>, thread_handle: Option<JoinHandle<()>> }`
- [ ] `[IMPL]` Implémenter `DspRunner::start(rx: Receiver<Vec<f32>>, tx: Sender<[[f32;13];98]>) -> Result<(), DspError>` — spawn d'un thread qui appelle `process_batch` sur chaque batch reçu et envoie les matrices produites
- [ ] `[IMPL]` Implémenter `DspRunner::stop()` — pose `running` à false et `join` le thread
- [ ] `[IMPL]` Implémenter `Drop for DspRunner` — appelle `stop()` silencieusement
- [ ] `[TEST-I]` **Test d'intégration :** Envoyer 3 secondes de signal synthétique (sinus 440 Hz à 16 kHz) via le channel — vérifier que ≥ 2 matrices MFCC sont reçues
- [ ] `[TEST-I]` **Test d'intégration :** Fermer le `Sender` → vérifier que le thread DSP se termine proprement sans panic
- [ ] `[TEST-I]` **Test d'intégration :** Drop sans stop → vérifier zéro thread zombie

### P10.3 — Mode standalone

- [ ] `[IMPL]` Créer `examples/standalone_dsp.rs` (feature `standalone`)
- [ ] `[IMPL]` L'exemple génère 3 secondes de signal sinusoïdal 440 Hz, le fait passer dans le pipeline DSP, affiche les MFCC de la première matrice produite et log les statistiques (nb matrices, latence moyenne)
- [ ] `[TEST-I]` **Test :** `cargo run --example standalone_dsp --features standalone` — s'exécute sans erreur
- [ ] `[VALID]` **Validation manuelle :** Les valeurs MFCC affichées sont finies (pas de `NaN`, pas d'`inf`)

---

## PARTIE 11 — Suite de tests complète & régression

> **Objectif :** Consolider tous les tests, ajouter les tests de régression manquants, valider la performance.

---

### P11.1 — Organisation des tests

- [ ] `[SETUP]` Créer `tests/pipeline_dsp_integration.rs` — tests d'intégration du crate complet
- [ ] `[SETUP]` Créer `benches/dsp_bench.rs` — benchmarks par étape et de bout en bout
- [ ] `[SETUP]` Ajouter `fixtures/reference_mfcc.json` dans le dépôt (généré par le script Python P0.3)
- [ ] `[SETUP]` Séparer les tests nécessitant le fichier de référence avec `#[cfg(not(feature = "skip_fixtures"))]`

### P11.2 — Tests unitaires de régression

- [ ] `[TEST-U]` **Régression :** `DspConfig::default().validate()` → Ok
- [ ] `[TEST-U]` **Régression :** `hz_to_mel` / `mel_to_hz` — bijection sur 10 valeurs tabulées
- [ ] `[TEST-U]` **Régression :** Fenêtre Hann 400 — premier et dernier coefficients sont 0.0
- [ ] `[TEST-U]` **Régression :** FFT signal silence → magnitudes nulles
- [ ] `[TEST-U]` **Régression :** FFT signal sinusoïdal → pic à la bonne bin
- [ ] `[TEST-U]` **Régression :** Log-énergies avec valeur nulle → pas de panique
- [ ] `[TEST-U]` **Régression :** Accumulateur — `is_ready` après 98 trames exactement
- [ ] `[TEST-U]` **Régression :** Tailles des FFI structs (`DSPSplitComplex`)

### P11.3 — Tests de validation numérique de régression

- [ ] `[TEST-N]` **Régression numérique :** Pré-accentuation vs librosa — erreur < 1e-5
- [ ] `[TEST-N]` **Régression numérique :** Fenêtre Hann vs numpy — erreur < 1e-6
- [ ] `[TEST-N]` **Régression numérique :** Magnitudes FFT vs numpy — erreur relative < 1e-3
- [ ] `[TEST-N]` **Régression numérique :** Matrice Mel vs librosa — erreur < 1e-4
- [ ] `[TEST-N]` **Régression numérique :** MFCC bout en bout vs `fixtures/reference_mfcc.json` — erreur < 1e-2

### P11.4 — Tests d'intégration de régression

- [ ] `[TEST-I]` **Régression :** Cycle `new → process_batch (3s) → stop` — aucune panique, aucune fuite
- [ ] `[TEST-I]` **Régression :** Deux instances simultanées — setups FFT/DCT distincts, résultats identiques
- [ ] `[TEST-I]` **Régression :** Thread runner → 10 matrices MFCC reçues en moins de 500 ms (signal synthétique)

### P11.5 — Tests de performance

- [ ] `[TEST-P]` **Benchmark :** Latence de `FrameProcessor::process_frame` sur 1 trame — doit être < 0.5 ms
- [ ] `[TEST-P]` **Benchmark :** Throughput du pipeline complet (98 trames) — doit être < 5 ms pour 1 seconde d'audio
- [ ] `[VALID]` **Validation AddressSanitizer :** `RUSTFLAGS="-Z sanitizer=address" cargo +nightly test -p pipeline_dsp` — zéro erreur mémoire, zéro fuite sur les setups `c_void`
- [ ] `[VALID]` **Validation CPU Instruments :** Pipeline en boucle continue pendant 60 s — CPU < 0,1 %

---

## PARTIE 12 — Documentation & Livraison du module

> **Objectif :** Module documenté, propre, versionné, et prêt à être intégré dans le workspace.

---

### P12.1 — Documentation

- [ ] `[SETUP]` Ajouter des doc-comments `///` sur tous les types et fonctions publics
- [ ] `[SETUP]` Écrire un doc-example dans `DspPipeline::new` montrant l'instanciation et un appel à `process_batch`
- [ ] `[TEST-U]` **Test :** `cargo doc --no-deps -p pipeline_dsp` — sans erreur ni warning

### P12.2 — Validation finale

- [ ] `[VALID]` `cargo clippy -p pipeline_dsp -- -D warnings` — zéro warning
- [ ] `[VALID]` `cargo fmt --check -p pipeline_dsp` — code formaté
- [ ] `[VALID]` `cargo test -p pipeline_dsp` — suite complète verte
- [ ] `[VALID]` `cargo test -p pipeline_dsp --features mock_input` — suite mock verte
- [ ] `[VALID]` `cargo build -p pipeline_dsp --release` — compile proprement

### P12.3 — Intégration dans le workspace

- [ ] `[SETUP]` Vérifier que `pipeline_dsp` est bien dans le `[workspace]` racine
- [ ] `[SETUP]` Documenter dans `pipeline/README.md` : prérequis (Accelerate, Python/librosa pour les fixtures), comment générer `reference_mfcc.json`, comment lancer les tests, comment brancher le module sur `audio_capture`
- [ ] `[TEST-I]` **Test d'intégration finale workspace :** Depuis un crate `integration_test` factice, brancher `audio_capture` (mode mock) → `pipeline_dsp` → vérifier que des matrices MFCC sont reçues côté consommateur — sans erreur de compilation ni de runtime

---

## Récapitulatif par partie

| Partie | Thème | Dépend de |
|---|---|---|
| P0 | Installation & Setup | — |
| P1 | Erreurs & Config | P0 |
| P2 | Bindings FFI Accelerate | P0 |
| P3 | Pré-accentuation | P1 |
| P4 | Framing | P1 |
| P5 | Fenêtrage Hann | P1, P2 |
| P6 | FFT | P2, P5 |
| P7 | Banc de filtres Mel | P1, P2, P6 |
| P8 | Log + DCT → MFCC | P2, P7 |
| P9 | Chaîne complète & accumulateur | P3, P4, P5, P6, P7, P8 |
| P10 | Façade publique & thread | P9 |
| P11 | Suite de tests complète | P1→P10 |
| P12 | Documentation & Livraison | P11 |
