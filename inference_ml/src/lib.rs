//! # inference_ml
//!
//! Module d'inférence wake-word basé sur Core ML (Apple Neural Engine).
//!
//! ## Architecture
//!
//! ```text
//! [canal MFCC]  ──►  InferenceEngine  ──►  InferenceRunner (thread)
//!                                               │
//!                                          CoreMLModel (FFI)
//!                                               │
//!                                       coreml_bridge.mm (ObjC++)
//!                                               │
//!                                        MLModel (Core ML)
//! ```
//!
//! ## Utilisation minimale
//!
//! ```no_run
//! use crossbeam_channel::bounded;
//! use inference_ml::{InferenceConfig, InferenceEngine};
//!
//! let config = InferenceConfig {
//!     model_path: "path/to/WakeWord.mlmodelc".into(),
//!     ..Default::default()
//! };
//!
//! let mut engine = InferenceEngine::new(config).unwrap();
//!
//! let (tx_in, rx_in) = bounded::<[[f32; 13]; 98]>(8);
//! let (tx_out, rx_out) = bounded::<f32>(8);
//! engine.start(rx_in, tx_out).unwrap();
//!
//! tx_in.send([[0.0f32; 13]; 98]).unwrap();
//! let score = rx_out.recv().unwrap(); // ∈ [0.0, 1.0]
//!
//! engine.stop().unwrap();
//! ```

pub mod config;
pub mod engine;
pub mod error;
pub mod ffi;
pub mod model;
pub mod runner;

pub use config::InferenceConfig;
pub use engine::InferenceEngine;
pub use error::InferenceError;
pub use model::CoreMLModel;
pub use runner::InferenceRunner;
