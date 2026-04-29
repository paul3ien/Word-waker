//! Façade de haut niveau qui assemble le pipeline DSP complet.
//!
//! `DspPipeline` agrège `Framer` → `FrameProcessor` → `MfccAccumulator`.

use crate::pipeline_dsp::config::DspConfig;
use crate::pipeline_dsp::error::DspError;
use crate::pipeline_dsp::framing::Framer;
use crate::pipeline_dsp::processor::{FrameProcessor, MfccAccumulator};

/// Pipeline DSP complet : fenêtrage, traitement par trame, accumulation MFCC.
///
/// Utilisez `process_batch` pour traiter un flux de samples PCM.
/// La méthode retourne zéro ou plusieurs matrices `[[f32;13];98]` selon
/// l'état d'accumulation.
pub struct DspPipeline {
    framer: Framer,
    processor: FrameProcessor,
    accumulator: MfccAccumulator,
}

impl DspPipeline {
    /// Construit un `DspPipeline` à partir de la configuration DSP.
    ///
    /// Valide la configuration et retourne `DspError` si elle est invalide.
    pub fn new(config: DspConfig) -> Result<Self, DspError> {
        config.validate()?;
        Ok(Self {
            framer: Framer::new(config.frame_size, config.hop_size),
            processor: FrameProcessor::new(&config)?,
            accumulator: MfccAccumulator::new(config.n_frames),
        })
    }

    /// Traite un batch de samples PCM.
    ///
    /// Retourne zéro ou plusieurs matrices `[[f32;13];98]`.
    /// Chaque matrice est produite dès que l'accumulateur atteint `n_frames` trames.
    ///
    /// Le pipeline conserve son état entre les appels (pré-accentuation, buffer de framing).
    pub fn process_batch(&mut self, samples: &[f32]) -> Vec<[[f32; 13]; 98]> {
        let frames = self.framer.push_samples(samples);
        let mut result = Vec::new();
        for mut frame in frames {
            let mfcc = self.processor.process_frame(&mut frame);
            self.accumulator.push(mfcc);
            if self.accumulator.is_ready() {
                result.push(self.accumulator.get_matrix());
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::DspPipeline;
    use crate::pipeline_dsp::config::DspConfig;

    #[test]
    fn dsp_pipeline_new_default_succeeds() {
        let cfg = DspConfig::default();
        let p = DspPipeline::new(cfg);
        assert!(p.is_ok(), "DspPipeline::new doit réussir avec la config par défaut");
    }

    #[test]
    fn process_batch_silence_no_panic() {
        // Traiter un batch silence : pas de panique, résultats finis
        let cfg = DspConfig::default();
        let mut p = DspPipeline::new(cfg.clone()).unwrap();
        // 98 trames × hop_size + frame_size samples pour garantir au moins une matrice
        let n_samples = cfg.n_frames * cfg.hop_size + cfg.frame_size;
        let samples = vec![0.0f32; n_samples];
        let matrices = p.process_batch(&samples);
        assert!(
            !matrices.is_empty(),
            "Au moins une matrice attendue pour {} samples",
            n_samples
        );
        for m in &matrices {
            for frame in m {
                for &v in frame {
                    assert!(v.is_finite(), "Valeur MFCC non finie dans le résultat");
                }
            }
        }
    }

    #[test]
    fn process_batch_incremental_same_as_bulk() {
        // Traiter le même signal en un seul batch ou en plusieurs batches de 160 samples
        // doit produire le même nombre de matrices.
        let cfg = DspConfig::default();
        let n_samples = cfg.n_frames * cfg.hop_size + cfg.frame_size;
        let samples: Vec<f32> = (0..n_samples)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16000.0).sin())
            .collect();

        // Bulk
        let mut bulk = DspPipeline::new(cfg.clone()).unwrap();
        let bulk_matrices = bulk.process_batch(&samples);

        // Incremental (batches de hop_size)
        let mut incr = DspPipeline::new(cfg.clone()).unwrap();
        let mut incr_matrices = Vec::new();
        for chunk in samples.chunks(cfg.hop_size) {
            incr_matrices.extend(incr.process_batch(chunk));
        }

        assert_eq!(
            bulk_matrices.len(),
            incr_matrices.len(),
            "Nombre de matrices : bulk={} vs incrémental={}",
            bulk_matrices.len(),
            incr_matrices.len()
        );
    }
}
