//! Bindings FFI vers Accelerate (vDSP, DCT, BLAS).
//!
//! Toutes les fonctions sont `unsafe`. Le linkage avec `Accelerate.framework`
//! est assuré par `build.rs`.

use libc::c_void;

// ---------------------------------------------------------------------------
// Constantes vDSP
// ---------------------------------------------------------------------------

/// Algorithme FFT radix-2.
pub const K_FFT_RADIX2: i32 = 0;
/// Direction FFT directe (analyse).
pub const K_FFT_DIRECTION_FORWARD: i32 = 1;

// ---------------------------------------------------------------------------
// Constantes DCT
// ---------------------------------------------------------------------------

/// Type de DCT : DCT-II (utilisé pour les MFCC).
pub const V_DSP_DCT_II: i32 = 2;

// ---------------------------------------------------------------------------
// Constantes BLAS
// ---------------------------------------------------------------------------

/// Ordre row-major pour cblas_sgemv.
pub const CBLAS_ROW_MAJOR: i32 = 101;
/// Pas de transposition pour cblas_sgemv.
pub const CBLAS_NO_TRANS: i32 = 111;

// ---------------------------------------------------------------------------
// Structures
// ---------------------------------------------------------------------------

/// Représentation split-complex utilisée par vDSP pour les FFT.
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
    pub fn vDSP_create_fftsetup(log2n: u32, radix: i32) -> *mut c_void;

    /// Exécute une FFT réelle in-place (split-complex).
    pub fn vDSP_fft_zrip(
        setup: *mut c_void,
        signal: *mut DSPSplitComplex,
        stride: u32,
        log2n: u32,
        direction: i32,
    );

    /// Calcule le carré du module de chaque bin complexe (puissance spectrale).
    pub fn vDSP_zvmags(
        input: *const DSPSplitComplex,
        i_stride: u32,
        output: *mut f32,
        o_stride: u32,
        n: u32,
    );

    /// Multiplication élément-par-élément de deux vecteurs float.
    pub fn vDSP_vmul(
        a: *const f32,
        ia: u32,
        b: *const f32,
        ib: u32,
        c: *mut f32,
        ic: u32,
        n: u32,
    );

    /// Libère un plan FFT vDSP.
    pub fn vDSP_destroy_fftsetup(setup: *mut c_void);

    // --- vDSP DCT ---

    /// Crée un plan DCT vDSP de taille n.
    pub fn vDSP_DCT_CreateSetup(prev: *mut c_void, n: u32, dct_type: i32) -> *mut c_void;

    /// Exécute une DCT vDSP.
    pub fn vDSP_DCT_Execute(setup: *mut c_void, input: *const f32, output: *mut f32);

    // --- BLAS ---

    /// Produit matrice-vecteur : y = alpha*A*x + beta*y.
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
            let setup = vDSP_create_fftsetup(9, K_FFT_RADIX2);
            assert!(!setup.is_null(), "vDSP_create_fftsetup a retourné NULL");
            vDSP_destroy_fftsetup(setup);
        }
    }

    #[test]
    fn linkage_dct_create_destroy() {
        // Taille valide pour vDSP DCT : f*2^n avec f∈{1,3,5,15} et n≥4.
        // 64 = 1*2^6 ✓ (40 = 5*2^3 est invalide → crash interne vDSP)
        unsafe {
            let setup = vDSP_DCT_CreateSetup(ptr::null_mut(), 64, V_DSP_DCT_II);
            assert!(!setup.is_null(), "vDSP_DCT_CreateSetup a retourné NULL");
            // Pas de vDSP_DCT_DestroySetup public — le process nettoie à sa fin.
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
                1, 1,         // m=1, n=1
                1.0,          // alpha
                a.as_ptr(), 1,
                x.as_ptr(), 1,
                0.0,          // beta
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
