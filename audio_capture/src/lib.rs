//! Crate de capture audio PCM via CoreAudio (macOS).
//!
//! Expose [`AudioCapture`], [`AudioCaptureConfig`] et [`AudioCaptureError`].
//! Le module [`mock`] (feature `mock_audio`) fournit un signal sinusoïdal
//! synthétique pour les tests sans microphone physique.

#![warn(missing_docs)]

/// Modules internes du crate (config, device, FFI, ring buffer, etc.).
pub mod audio_capture;

pub use audio_capture::config::AudioCaptureConfig;
pub use audio_capture::error::AudioCaptureError;
pub use audio_capture::facade::AudioCapture;

#[cfg(feature = "mock_audio")]
pub use audio_capture::mock;
