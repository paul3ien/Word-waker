//! Pipeline DSP : PreEmphasis → Framing → FFT → Mel → MFCC.

/// Configuration du pipeline DSP.
pub mod config;
/// Gestion des erreurs du pipeline DSP.
pub mod error;
/// Bindings FFI Accelerate (vDSP, BLAS, DCT).
pub mod ffi;
/// FFT réelle via vDSP_fft_zrip → spectre de magnitude.
pub mod fft;
/// Découpage en trames avec overlap.
pub mod framing;
/// Banc de filtres Mel (matrice triangulaire row-major, application via cblas_sgemv).
pub mod mel_filterbank;
/// Log des énergies Mel, DCT-II via vDSP, extraction des 13 coefficients MFCC.
pub mod mfcc;
/// Façade de haut niveau : Framer + FrameProcessor + MfccAccumulator.
pub mod pipeline;
/// Filtre de pré-accentuation IIR du premier ordre.
pub mod preemphasis;
/// Processeur de trame (chaîne complète) et accumulateur de trames MFCC.
///
/// `FrameProcessor` : PreEmphasis → HannWindow → FFT → MelFilterbank → log\_mel → DCT → \[f32;13\]
pub mod processor;
/// Thread DSP runner : lit des batches et envoie des matrices MFCC via channels.
pub mod runner;
/// Fenêtrage de Hann via vDSP_vmul.
pub mod windowing;

pub use error::DspError;
