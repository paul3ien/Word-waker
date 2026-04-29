//! Wrapper autour de `vDSP_fft_zrip` pour le calcul du spectre de magnitude.
//!
//! Flux : interleaved `f32` → split-complex → `vDSP_fft_zrip` → `vDSP_zvmags` → magnitudes.

use libc::c_void;

use crate::pipeline_dsp::error::DspError;
use crate::pipeline_dsp::ffi::{
    vDSP_create_fftsetup, vDSP_destroy_fftsetup, vDSP_fft_zrip, vDSP_zvmags, DSPSplitComplex,
    K_FFT_DIRECTION_FORWARD, K_FFT_RADIX2,
};

/// Wrapper sûr autour du plan FFT vDSP.
///
/// Calcule le spectre de magnitude d'un signal réel en `n_fft / 2` bins.
pub struct VDspFft {
    setup: *mut c_void,
    /// Taille de la FFT (puissance de 2).
    pub n: usize,
    /// log₂(n) — paramètre attendu par vDSP.
    log2n: u32,
}

/// # Safety
/// `vDSP_fft_zrip` est thread-safe en lecture (chaque appel `forward` utilise
/// des buffers locaux ; seul le `setup` est partagé en lecture seule).
unsafe impl Send for VDspFft {}

impl VDspFft {
    /// Crée un plan FFT de taille `n` (doit être une puissance de 2).
    ///
    /// # Errors
    /// Retourne `DspError::FftSetupFailed` si `vDSP_create_fftsetup` retourne NULL.
    pub fn new(n: usize) -> Result<Self, DspError> {
        assert!(
            n.is_power_of_two() && n >= 4,
            "VDspFft: n doit être une puissance de 2 ≥ 4"
        );
        let log2n = n.trailing_zeros();
        let setup = unsafe { vDSP_create_fftsetup(log2n as usize, K_FFT_RADIX2) };
        if setup.is_null() {
            return Err(DspError::FftSetupFailed);
        }
        Ok(Self { setup, n, log2n })
    }

    /// Calcule le spectre de magnitude du `frame` et retourne `n / 2` valeurs.
    ///
    /// Si `frame.len() < n`, le signal est zero-paddé. Si `frame.len() > n`,
    /// seuls les `n` premiers samples sont utilisés.
    ///
    /// # Valeur de retour
    /// Vecteur de `n / 2` magnitudes (non normalisées).
    pub fn forward(&self, frame: &[f32]) -> Vec<f32> {
        let n = self.n;
        let half = n / 2;

        // 1. Zero-pad (ou tronquer) le signal à n_fft samples.
        let mut padded = vec![0.0f32; n];
        let copy_len = frame.len().min(n);
        padded[..copy_len].copy_from_slice(&frame[..copy_len]);

        // 2. Désentrelacer en split-complex : even → realp, odd → imagp.
        //    vDSP_fft_zrip attend N/2 paires complexes pour une FFT réelle de taille N.
        let mut real_buf = vec![0.0f32; half];
        let mut imag_buf = vec![0.0f32; half];
        for i in 0..half {
            real_buf[i] = padded[2 * i];
            imag_buf[i] = padded[2 * i + 1];
        }

        let mut split = DSPSplitComplex {
            realp: real_buf.as_mut_ptr(),
            imagp: imag_buf.as_mut_ptr(),
        };

        // 3. Calculer la FFT in-place (résultat dans split).
        unsafe {
            vDSP_fft_zrip(
                self.setup,
                &mut split,
                1,
                self.log2n as usize,
                K_FFT_DIRECTION_FORWARD,
            );
        }

        // 4. Calculer la puissance spectrale |X[k]|² = realp[k]² + imagp[k]².
        let mut power = vec![0.0f32; half];
        unsafe {
            vDSP_zvmags(
                &split as *const DSPSplitComplex,
                1,
                power.as_mut_ptr(),
                1,
                half,
            );
        }

        // 5. Retourner les magnitudes (racine de la puissance).
        power.iter().map(|p| p.sqrt()).collect()
    }
}

impl Drop for VDspFft {
    fn drop(&mut self) {
        if !self.setup.is_null() {
            unsafe { vDSP_destroy_fftsetup(self.setup) };
            self.setup = std::ptr::null_mut();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::VDspFft;
    use std::f32::consts::PI;

    #[test]
    fn new_512_succeeds() {
        let fft = VDspFft::new(512);
        assert!(fft.is_ok(), "VDspFft::new(512) devrait réussir");
    }

    #[test]
    fn output_length_is_n_over_2() {
        let fft = VDspFft::new(512).unwrap();
        let frame = vec![0.0f32; 400];
        let mags = fft.forward(&frame);
        assert_eq!(mags.len(), 256, "attendu 256 bins, obtenu {}", mags.len());
    }

    #[test]
    fn silence_gives_zero_magnitudes() {
        let fft = VDspFft::new(512).unwrap();
        let frame = vec![0.0f32; 400];
        let mags = fft.forward(&frame);
        for (k, &m) in mags.iter().enumerate() {
            assert!(m < 1e-10, "mags[{}] = {} (attendu ≈ 0)", k, m);
        }
    }

    #[test]
    fn sine_1000hz_peak_at_bin_32() {
        // 1000 Hz * 512 / 16000 Hz = 32.0 → bin 32 exact
        let sample_rate = 16_000_usize;
        let frame_size = 400_usize;
        let n_fft = 512_usize;

        let frame: Vec<f32> = (0..frame_size)
            .map(|n| (2.0 * PI * 1000.0 * n as f32 / sample_rate as f32).sin())
            .collect();

        let fft = VDspFft::new(n_fft).unwrap();
        let mags = fft.forward(&frame);

        // Chercher le bin à l'énergie maximale
        let (peak_bin, peak_val) = mags
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap();

        // Le vrai pic peut être à ±1 bin de 32 à cause du zero-padding partiel
        assert!(
            peak_bin == 31 || peak_bin == 32 || peak_bin == 33,
            "pic attendu autour du bin 32, obtenu bin {} (val={:.2})",
            peak_bin,
            peak_val
        );
    }
}
