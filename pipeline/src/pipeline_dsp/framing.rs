//! Découpage d'un flux de samples PCM en trames de taille fixe avec overlap.

/// Accumule des samples et produit des trames de taille fixe avec un pas configurable.
///
/// Chaque trame a `frame_size` samples. Le curseur avance de `hop_size` après chaque trame,
/// ce qui crée un overlap de `frame_size - hop_size` samples entre trames consécutives.
#[derive(Debug, Clone)]
pub struct Framer {
    buffer: Vec<f32>,
    /// Nombre de samples par trame.
    pub frame_size: usize,
    /// Décalage entre le début de deux trames consécutives.
    pub hop_size: usize,
}

impl Framer {
    /// Crée un nouveau `Framer`.
    ///
    /// # Panics
    /// Panique si `hop_size == 0` ou `frame_size == 0`.
    pub fn new(frame_size: usize, hop_size: usize) -> Self {
        assert!(frame_size > 0, "frame_size doit être > 0");
        assert!(hop_size > 0, "hop_size doit être > 0");
        Self {
            buffer: Vec::new(),
            frame_size,
            hop_size,
        }
    }

    /// Pousse des samples dans le buffer interne et retourne toutes les trames complètes
    /// disponibles. Les samples sont consommés par paquet de `hop_size` à chaque trame.
    pub fn push_samples(&mut self, samples: &[f32]) -> Vec<Vec<f32>> {
        self.buffer.extend_from_slice(samples);
        let mut frames = Vec::new();
        while self.buffer.len() >= self.frame_size {
            frames.push(self.buffer[..self.frame_size].to_vec());
            self.buffer.drain(..self.hop_size);
        }
        frames
    }

    /// Vide le buffer interne (début d'un nouvel utterance).
    pub fn reset(&mut self) {
        self.buffer.clear();
    }

    /// Nombre de samples actuellement en attente dans le buffer interne.
    pub fn pending_samples(&self) -> usize {
        self.buffer.len()
    }
}

#[cfg(test)]
mod tests {
    use super::Framer;

    /// frame_size=400, hop_size=160
    fn default_framer() -> Framer {
        Framer::new(400, 160)
    }

    #[test]
    fn push_exact_frame_size() {
        // 400 samples → 1 trame extraite, drain 160 → buffer résiduel = 240 samples
        let mut fr = default_framer();
        let samples: Vec<f32> = (0..400).map(|i| i as f32).collect();
        let frames = fr.push_samples(&samples);
        assert_eq!(frames.len(), 1, "attendu 1 trame");
        assert_eq!(frames[0].len(), 400);
        assert_eq!(fr.pending_samples(), 240);
    }

    #[test]
    fn push_incremental_then_larger() {
        // Pousser 160 puis 400 samples :
        // Après 160 : buffer=160, pas de trame
        // Après 400 supplémentaires : buffer=560 → trames à 400..560
        //   trame 1 : [0..400] → drain 160 → buffer=400
        //   trame 2 : [160..560] → drain 160 → buffer=240
        // Total : 2 trames
        let mut fr = default_framer();
        let a: Vec<f32> = (0..160).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..400).map(|i| i as f32).collect();
        let f1 = fr.push_samples(&a);
        assert_eq!(f1.len(), 0);
        let f2 = fr.push_samples(&b);
        assert_eq!(f2.len(), 2, "attendu 2 trames, obtenu {}", f2.len());
    }

    #[test]
    fn push_large_chunk() {
        // floor((10000 - 400) / 160) + 1 = floor(9600/160) + 1 = 60 + 1 = 61
        let mut fr = default_framer();
        let samples: Vec<f32> = vec![0.0; 10_000];
        let frames = fr.push_samples(&samples);
        let expected = (10_000 - 400) / 160 + 1;
        assert_eq!(frames.len(), expected, "attendu {} trames, obtenu {}", expected, frames.len());
    }

    #[test]
    fn all_frames_have_correct_size() {
        let mut fr = default_framer();
        let samples: Vec<f32> = vec![1.0; 5_000];
        let frames = fr.push_samples(&samples);
        for (i, f) in frames.iter().enumerate() {
            assert_eq!(f.len(), 400, "trame {} a une taille incorrecte : {}", i, f.len());
        }
    }

    #[test]
    fn overlap_content_correct() {
        // Les samples 160..400 de la trame N doivent être les samples 0..240 de la trame N+1.
        let mut fr = default_framer();
        let samples: Vec<f32> = (0..1000).map(|i| i as f32).collect();
        let frames = fr.push_samples(&samples);
        assert!(frames.len() >= 2, "pas assez de trames pour tester l'overlap");
        // trame 0 → samples [0..400], trame 1 → samples [160..560]
        // overlap = trame0[160..400] == trame1[0..240]
        assert_eq!(&frames[0][160..], &frames[1][..240]);
    }

    #[test]
    fn reset_clears_buffer() {
        let mut fr = default_framer();
        fr.push_samples(&vec![1.0; 300]);
        assert!(fr.pending_samples() > 0);
        fr.reset();
        assert_eq!(fr.pending_samples(), 0);
    }
}
