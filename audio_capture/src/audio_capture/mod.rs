/// Configuration de la capture audio.
pub mod config;
/// Thread consommateur du ring buffer.
pub mod consumer;
/// Sélection du device d'entrée CoreAudio.
pub mod device;
/// Types d'erreur du module.
pub mod error;
/// Façade publique (`AudioCapture`).
pub mod facade;
/// Bindings FFI CoreAudio (types et fonctions C).
pub mod ffi;
/// Signal sinusoïdal synthétique pour tests sans microphone.
#[cfg(feature = "mock_audio")]
pub mod mock;
/// Ring buffer lock-free producteur/consommateur.
pub mod ring_buffer;
/// Capture HAL via `AudioDeviceCreateIOProcID`.
pub mod unit;
