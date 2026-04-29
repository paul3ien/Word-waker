//! Pipeline DSP : PreEmphasis → Framing → FFT → Mel → MFCC.

/// Gestion des erreurs du pipeline DSP.
pub mod error;
/// Configuration du pipeline DSP.
pub mod config;
/// Bindings FFI Accelerate (vDSP, BLAS, DCT).
pub mod ffi;

pub use error::DspError;
