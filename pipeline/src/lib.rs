//! Crate de traitement du signal DSP pour la reconnaissance vocale.
//!
//! Consomme des samples PCM Float32 16 kHz depuis `audio_capture` et produit
//! des matrices MFCC `[98×13]` prêtes pour l'inférence ML.

#![warn(missing_docs)]

pub mod pipeline_dsp;
