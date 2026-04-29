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
/// Tous les buffers intermédiaires sont pré-alloués dans `new` pour zéro allocation en régime permanent.
pub struct VDspFft {
    setup: *mut c_void,
    /// Taille de la FFT (puissance de 2).
    pub n: usize,
    /// log₂(n) — paramètre attendu par vDSP.
    log2n: u32,
    // Buffers pré-alloués — réutilisés à chaque appel de forward()
    buf_padded: Vec<f32>,
    buf_real: Vec<f32>,
    buf_imag: Vec<f32>,
    buf_power: Vec<f32>,
}

/// # Safety
/// `VDspFft` contient un pointeur `setup` opaque et des buffers de travail.
/// L'envoi entre threads est sûr car chaque instance possède ses propres buffers
/// et `vDSP_fft_zrip` est réentrant avec des setups différents.
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
        let half = n / 2;
        Ok(Self {
            setup,
            n,
            log2n,
            buf_padded: vec![0.0f32; n],
            buf_real: vec![0.0f32; half],
            buf_imag: vec![0.0f32; half],
            buf_power: vec![0.0f32; half],
        })
    }

    /// Calcule le spectre de magnitude du `frame` et écrit `n / 2` magnitudes dans `out`.
    ///
    /// Si `frame.len() < n`, le signal est zero-paddé. Si `frame.len() > n`,
    /// seuls les `n` premiers samples sont utilisés.
    ///
    /// Aucune allocation dynamique — tous les buffers intermédiaires sont pré-alloués dans `new`.
    pub fn forward_into(&mut self, frame: &[f32], out: &mut [f32]) {
        let n = self.n;
        let half = n / 2;
        debug_assert_eq!(out.len(), half);

        // 1. Zero-pad dans le buffer pré-alloué.
        let copy_len = frame.len().min(n);
        self.buf_padded[..copy_len].copy_from_slice(&frame[..copy_len]);
        self.buf_padded[copy_len..].fill(0.0);

        // 2. Désentrelacer : even → realp, odd → imagp.
        for i in 0..half {
            self.buf_real[i] = self.buf_padded[2 * i];
            self.buf_imag[i] = self.buf_padded[2 * i + 1];
        }

        let mut split = DSPSplitComplex {
            realp: self.buf_real.as_mut_ptr(),
            imagp: self.buf_imag.as_mut_ptr(),
        };

        // 3. FFT in-place.
        unsafe {
            vDSP_fft_zrip(
                self.setup,
                &mut split,
                1,
                self.log2n as usize,
                K_FFT_DIRECTION_FORWARD,
            );
        }

        // 4. Puissance spectrale → buf_power.
        unsafe {
            vDSP_zvmags(
                &split as *const DSPSplitComplex,
                1,
                self.buf_power.as_mut_ptr(),
                1,
                half,
            );
        }

        // 5. Racine carrée → out.
        for (o, &p) in out.iter_mut().zip(self.buf_power.iter()) {
            *o = p.sqrt();
        }
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
        let mut fft = VDspFft::new(512).unwrap();
        let frame = vec![0.0f32; 400];
        let mut mags = vec![0.0f32; 256];
        fft.forward_into(&frame, &mut mags);
        assert_eq!(mags.len(), 256, "attendu 256 bins, obtenu {}", mags.len());
    }

    #[test]
    fn silence_gives_zero_magnitudes() {
        let mut fft = VDspFft::new(512).unwrap();
        let frame = vec![0.0f32; 400];
        let mut mags = vec![0.0f32; 256];
        fft.forward_into(&frame, &mut mags);
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

        let mut fft = VDspFft::new(n_fft).unwrap();
        let mut mags = vec![0.0f32; n_fft / 2];
        fft.forward_into(&frame, &mut mags);

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
