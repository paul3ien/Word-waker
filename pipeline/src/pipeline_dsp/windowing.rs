//! Fenêtrage de Hann appliqué via `vDSP_vmul` (Accelerate).

use crate::pipeline_dsp::ffi::vDSP_vmul;
use std::f32::consts::PI;

/// Fenêtre de Hann précalculée de taille fixe.
///
/// `w[n] = 0.5 · (1 − cos(2π·n / (N−1)))` pour n ∈ [0, N−1].
///
/// La multiplication est effectuée via `vDSP_vmul` (Accelerate SIMD).
#[derive(Debug, Clone)]
pub struct HannWindow {
    /// Coefficients de la fenêtre, longueur = `size`.
    pub coefficients: Vec<f32>,
    /// Nombre de samples par trame.
    pub size: usize,
}

impl HannWindow {
    /// Calcule et stocke les coefficients d'une fenêtre de Hann de taille `size`.
    ///
    /// # Panics
    /// Panique si `size < 2`.
    pub fn new(size: usize) -> Self {
        assert!(size >= 2, "HannWindow: size doit être ≥ 2");
        let n_minus_1 = (size - 1) as f32;
        let coefficients = (0..size)
            .map(|n| 0.5 * (1.0 - (2.0 * PI * n as f32 / n_minus_1).cos()))
            .collect();
        Self { coefficients, size }
    }

    /// Applique la fenêtre in-place sur `frame` via `vDSP_vmul`.
    ///
    /// # Panics
    /// Panique si `frame.len() != self.size`.
    pub fn apply(&self, frame: &mut [f32]) {
        assert_eq!(
            frame.len(),
            self.size,
            "HannWindow::apply: frame.len()={} != size={}",
            frame.len(),
            self.size
        );
        // vDSP_vmul ne garantit pas le bon comportement si src et dst se chevauchent.
        // On passe par un buffer temporaire, puis on copie le résultat.
        let mut tmp = vec![0.0f32; self.size];
        unsafe {
            vDSP_vmul(
                frame.as_ptr(),
                1,
                self.coefficients.as_ptr(),
                1,
                tmp.as_mut_ptr(),
                1,
                self.size,
            );
        }
        frame.copy_from_slice(&tmp);
    }
}

#[cfg(test)]
mod tests {
    use super::HannWindow;

    /// Pour une taille impaire N, le coefficient central w[(N-1)/2] est exactement 1.0.
    /// Vérification : w[n] = 0.5*(1-cos(2π·n/(N-1))), n=(N-1)/2 → cos(π)=-1 → w=1.0
    const ODD_SIZE: usize = 5;

    #[test]
    fn boundary_coefficients_are_zero() {
        let w = HannWindow::new(400);
        assert!(
            w.coefficients[0].abs() < 1e-10,
            "w[0] devrait être 0.0, obtenu {}",
            w.coefficients[0]
        );
        assert!(
            w.coefficients[399].abs() < 1e-10,
            "w[399] devrait être 0.0, obtenu {}",
            w.coefficients[399]
        );
    }

    #[test]
    fn center_coefficient_is_one_for_odd_size() {
        // N=5 : w[2] = 0.5*(1-cos(2π*2/4)) = 0.5*(1-cos(π)) = 1.0
        let w = HannWindow::new(ODD_SIZE);
        let center = ODD_SIZE / 2; // = 2
        assert!(
            (w.coefficients[center] - 1.0).abs() < 1e-6,
            "w[{}] devrait être 1.0, obtenu {}",
            center,
            w.coefficients[center]
        );
    }

    #[test]
    fn apply_on_ones_yields_coefficients() {
        // Signal constant 1.0 → après fenêtrage, frame[n] = w[n]
        let w = HannWindow::new(400);
        let mut frame = vec![1.0f32; 400];
        w.apply(&mut frame);
        for (n, (&got, &expected)) in frame.iter().zip(w.coefficients.iter()).enumerate() {
            assert!(
                (got - expected).abs() < 1e-6,
                "frame[{}]: attendu {}, obtenu {}",
                n,
                expected,
                got
            );
        }
    }

    #[test]
    fn symmetry() {
        // La fenêtre de Hann est symétrique : w[n] == w[N-1-n]
        let w = HannWindow::new(400);
        for n in 0..200 {
            assert!(
                (w.coefficients[n] - w.coefficients[399 - n]).abs() < 1e-6,
                "asymétrie à n={}: w[n]={}, w[N-1-n]={}",
                n,
                w.coefficients[n],
                w.coefficients[399 - n]
            );
        }
    }

    #[test]
    fn known_values_size5() {
        // np.hanning(5) = [0, 0.5, 1, 0.5, 0]
        let w = HannWindow::new(5);
        let expected = [0.0f32, 0.5, 1.0, 0.5, 0.0];
        for (n, (&got, &exp)) in w.coefficients.iter().zip(expected.iter()).enumerate() {
            assert!(
                (got - exp).abs() < 1e-6,
                "w[{}]: attendu {}, obtenu {}",
                n,
                exp,
                got
            );
        }
    }
}
