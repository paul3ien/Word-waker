//! Filtre de pré-accentuation : `y[n] = x[n] − α·x[n−1]`.

/// Filtre IIR de pré-accentuation du premier ordre.
///
/// Maintient l'état (`last_sample`) entre les appels successifs à `apply`
/// pour un traitement correct des trames consécutives.
#[derive(Debug, Clone)]
pub struct PreEmphasis {
    /// Coefficient de pré-accentuation (typiquement 0.97).
    pub alpha: f32,
    last_sample: f32,
}

impl PreEmphasis {
    /// Crée un nouveau filtre de pré-accentuation avec le coefficient `alpha`.
    pub fn new(alpha: f32) -> Self {
        Self { alpha, last_sample: 0.0 }
    }

    /// Applique le filtre in-place sur `frame`.
    ///
    /// L'état interne (`last_sample`) est mis à jour et conservé entre les appels.
    pub fn apply(&mut self, frame: &mut [f32]) {
        for sample in frame.iter_mut() {
            let current = *sample;
            *sample = current - self.alpha * self.last_sample;
            self.last_sample = current;
        }
    }

    /// Remet l'état interne à zéro (début d'un nouvel utterance).
    pub fn reset(&mut self) {
        self.last_sample = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::PreEmphasis;

    #[test]
    fn constant_signal() {
        // [1, 1, 1] avec α=0.97, last=0 → [1-0, 1-0.97, 1-0.97] = [1.0, 0.03, 0.03]
        let mut pe = PreEmphasis::new(0.97);
        let mut frame = [1.0_f32, 1.0, 1.0];
        pe.apply(&mut frame);
        assert!((frame[0] - 1.0).abs() < 1e-6, "frame[0]={}", frame[0]);
        assert!((frame[1] - 0.03).abs() < 1e-5, "frame[1]={}", frame[1]);
        assert!((frame[2] - 0.03).abs() < 1e-5, "frame[2]={}", frame[2]);
    }

    #[test]
    fn impulse_signal() {
        // [1, 0, 0] avec α=0.97 → [1.0, -0.97, 0.0]
        let mut pe = PreEmphasis::new(0.97);
        let mut frame = [1.0_f32, 0.0, 0.0];
        pe.apply(&mut frame);
        assert!((frame[0] - 1.0).abs() < 1e-6, "frame[0]={}", frame[0]);
        assert!((frame[1] - (-0.97)).abs() < 1e-6, "frame[1]={}", frame[1]);
        assert!(frame[2].abs() < 1e-6, "frame[2]={}", frame[2]);
    }

    #[test]
    fn silence_signal() {
        let mut pe = PreEmphasis::new(0.97);
        let mut frame = [0.0_f32, 0.0];
        pe.apply(&mut frame);
        assert_eq!(frame, [0.0, 0.0]);
    }

    #[test]
    fn state_propagated_across_calls() {
        // Premier appel : [1.0] → last_sample = 1.0
        // Deuxième appel : [2.0] → 2.0 - 0.97*1.0 = 1.03
        let mut pe = PreEmphasis::new(0.97);
        let mut f1 = [1.0_f32];
        pe.apply(&mut f1);
        let mut f2 = [2.0_f32];
        pe.apply(&mut f2);
        assert!((f2[0] - (2.0 - 0.97 * 1.0)).abs() < 1e-6, "f2[0]={}", f2[0]);
    }

    #[test]
    fn reset_clears_state() {
        let mut pe = PreEmphasis::new(0.97);
        let mut f1 = [1.0_f32];
        pe.apply(&mut f1);
        pe.reset();
        // Après reset, last_sample=0 → même résultat qu'un filtre frais
        let mut f2 = [1.0_f32];
        pe.apply(&mut f2);
        assert!((f2[0] - 1.0).abs() < 1e-6, "f2[0]={}", f2[0]);
    }
}
