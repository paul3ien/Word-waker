//! Bindings FFI vers Accelerate (vDSP, DCT, BLAS).
//!
//! Toutes les fonctions sont `unsafe`. Le linkage avec `Accelerate.framework`
//! est assuré par `build.rs`.
//!
//! Types utilisés :
//! - `vDSP_Length` (`unsigned long`) → `usize` (8 octets sur macOS 64-bit)
//! - `vDSP_Stride` (`long`)          → `isize` (8 octets sur macOS 64-bit)

use libc::c_void;

// ---------------------------------------------------------------------------
// Constantes vDSP
// ---------------------------------------------------------------------------

/// Algorithme FFT radix-2 (`kFFTRadix2 = 0`).
pub const K_FFT_RADIX2: i32 = 0;
/// Direction FFT directe — analyse (`kFFTDirection_Forward = 1`).
pub const K_FFT_DIRECTION_FORWARD: i32 = 1;

// ---------------------------------------------------------------------------
// Constantes DCT
// ---------------------------------------------------------------------------

/// Type de DCT : DCT-II (`vDSP_DCT_II = 2`), utilisé pour les MFCC.
pub const V_DSP_DCT_II: i32 = 2;

// ---------------------------------------------------------------------------
// Constantes BLAS
// ---------------------------------------------------------------------------

/// Ordre row-major pour cblas_sgemv (`CblasRowMajor = 101`).
pub const CBLAS_ROW_MAJOR: i32 = 101;
/// Pas de transposition pour cblas_sgemv (`CblasNoTrans = 111`).
pub const CBLAS_NO_TRANS: i32 = 111;

// ---------------------------------------------------------------------------
// Structures
// ---------------------------------------------------------------------------

/// Représentation split-complex utilisée par vDSP pour les FFT réelles.
#[repr(C)]
pub struct DSPSplitComplex {
    /// Pointeur vers les parties réelles.
    pub realp: *mut f32,
    /// Pointeur vers les parties imaginaires.
    pub imagp: *mut f32,
}

// ---------------------------------------------------------------------------
// Déclarations extern "C"
// ---------------------------------------------------------------------------

extern "C" {
    // --- vDSP FFT ---

    /// Crée un plan FFT vDSP de taille 2^log2n.
    /// `log2n: vDSP_Length` (`unsigned long` → `usize`)
    /// `radix: FFTRadix` (`int` → `i32`)
    pub fn vDSP_create_fftsetup(log2n: usize, radix: i32) -> *mut c_void;

    /// Exécute une FFT réelle in-place en représentation split-complex.
    /// `stride: vDSP_Stride` (`long` → `isize`), `log2n: vDSP_Length` (`usize`)
    pub fn vDSP_fft_zrip(
        setup: *mut c_void,
        signal: *mut DSPSplitComplex,
        stride: isize,
        log2n: usize,
        direction: i32,
    );

    /// Calcule la puissance spectrale (|z|²) de chaque bin complexe.
    /// Strides : `vDSP_Stride` → `isize`, N : `vDSP_Length` → `usize`
    pub fn vDSP_zvmags(
        input: *const DSPSplitComplex,
        i_stride: isize,
        output: *mut f32,
        o_stride: isize,
        n: usize,
    );

    /// Multiplication élément-par-élément de deux vecteurs float.
    /// Strides : `vDSP_Stride` → `isize`, N : `vDSP_Length` → `usize`
    pub fn vDSP_vmul(
        a: *const f32,
        ia: isize,
        b: *const f32,
        ib: isize,
        c: *mut f32,
        ic: isize,
        n: usize,
    );

    /// Libère un plan FFT vDSP.
    pub fn vDSP_destroy_fftsetup(setup: *mut c_void);

    // --- vDSP DCT ---

    /// Crée un plan DCT vDSP.
    /// `n` : `vDSP_Length` → `usize`.
    /// Tailles valides : f·2^k avec f ∈ {1,3,5,15} et k ≥ 4 (ex : 32, 48, 64, 80…).
    pub fn vDSP_DCT_CreateSetup(prev: *mut c_void, n: usize, dct_type: i32) -> *mut c_void;

    /// Exécute une DCT vDSP.
    pub fn vDSP_DCT_Execute(setup: *mut c_void, input: *const f32, output: *mut f32);

    // --- BLAS ---

    /// Produit matrice-vecteur : y = alpha·A·x + beta·y.
    /// Tous les entiers sont `int` (i32) dans cblas.
    pub fn cblas_sgemv(
        order: i32,
        trans: i32,
        m: i32,
        n: i32,
        alpha: f32,
        a: *const f32,
        lda: i32,
        x: *const f32,
        incx: i32,
        beta: f32,
        y: *mut f32,
        incy: i32,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn linkage_fft_create_destroy() {
        // Crée un plan FFT 2^9 = 512 et le libère immédiatement.
        unsafe {
            let setup = vDSP_create_fftsetup(9_usize, K_FFT_RADIX2);
            assert!(!setup.is_null(), "vDSP_create_fftsetup a retourné NULL");
            vDSP_destroy_fftsetup(setup);
        }
    }

    #[test]
    fn linkage_dct_create_destroy() {
        // 64 = 1·2^6 — taille valide (f=1, k=6 ≥ 4).
        unsafe {
            let setup = vDSP_DCT_CreateSetup(ptr::null_mut(), 64_usize, V_DSP_DCT_II);
            assert!(!setup.is_null(), "vDSP_DCT_CreateSetup a retourné NULL");
        }
    }

    #[test]
    fn linkage_blas_sgemv_trivial() {
        // Matrice 1×1 : [2.0], vecteur [3.0] → y = 2×3 = 6.0
        let a: [f32; 1] = [2.0];
        let x: [f32; 1] = [3.0];
        let mut y: [f32; 1] = [0.0];
        unsafe {
            cblas_sgemv(
                CBLAS_ROW_MAJOR,
                CBLAS_NO_TRANS,
                1, 1,
                1.0,
                a.as_ptr(), 1,
                x.as_ptr(), 1,
                0.0,
                y.as_mut_ptr(), 1,
            );
        }
        assert!(
            (y[0] - 6.0_f32).abs() < 1e-5,
            "cblas_sgemv 1×1 attendu 6.0, obtenu {}",
            y[0]
        );
    }
}
