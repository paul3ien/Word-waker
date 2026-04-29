//! Pipeline DSP : PreEmphasis → Framing → FFT → Mel → MFCC.

/// Gestion des erreurs du pipeline DSP.
pub mod error;
/// Configuration du pipeline DSP.
pub mod config;
/// Bindings FFI Accelerate (vDSP, BLAS, DCT).
pub mod ffi;
/// Filtre de pré-accentuation IIR du premier ordre.
pub mod preemphasis;
/// Découpage en trames avec overlap.
pub mod framing;
/// Fenêtrage de Hann via vDSP_vmul.
pub mod windowing;
/// FFT réelle via vDSP_fft_zrip → spectre de magnitude.
pub mod fft;

pub use error::DspError;
