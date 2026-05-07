#!/usr/bin/env python3
"""
Génère fixtures/numerical_references.json avec les valeurs de référence
intermédiaires de chaque étape DSP pour la trame 0 du signal sinus 440 Hz.

Utilisé par les tests [TEST-N] du crate pipeline_dsp pour valider chaque
étape indépendamment (pré-accentuation, Hann, FFT, Mel, DCT, MFCC).

Convention : reproduit EXACTEMENT le comportement du pipeline Rust/vDSP.
  - Pré-accentuation : IIR premier ordre, last_sample=0 (début du signal)
  - Hann : w[n] = 0.5 * (1 - cos(2π*n/(N-1)))
  - FFT : |numpy.fft.rfft| * 2.0  (convention vDSP_fft_zrip)
  - Mel : filtres triangulaires non normalisés, hauteur 1.0
  - Log : ln naturel, clamp à f32::EPSILON avant le log
  - DCT-II : scipy.fft.dct(x, type=2, norm=None) / 2.0  (convention vDSP)

Usage : python3 scripts/generate_numerical_references.py
"""
import json
import numpy as np
from scipy.fft import dct
from pathlib import Path

SAMPLE_RATE = 16_000
FREQ_HZ    = 440.0
FRAME_SIZE = 400
N_FFT      = 512
N_MELS     = 40
N_MFCC     = 13
FMIN       = 20.0
FMAX       = 8_000.0
ALPHA      = 0.97
F32_EPSILON = np.finfo(np.float32).eps

OUTPUT = Path(__file__).parent.parent / "fixtures" / "numerical_references.json"

# ── Signal brut (1 s de sinus 440 Hz, float32) ────────────────────────────────
n_samples = SAMPLE_RATE
signal = (np.sin(2 * np.pi * FREQ_HZ * np.arange(n_samples) / SAMPLE_RATE)
          .astype(np.float32))

# ── Trame 0 (samples 0..400) ──────────────────────────────────────────────────
raw_frame = signal[:FRAME_SIZE].copy()

# ── Étape 1 : Pré-accentuation (last_sample=0, début de signal) ──────────────
preemph = np.empty(FRAME_SIZE, dtype=np.float32)
prev = 0.0  # last_sample initial
for n in range(FRAME_SIZE):
    current = float(raw_frame[n])
    preemph[n] = current - ALPHA * prev
    prev = current

# ── Étape 2 : Fenêtre de Hann (N=400) ────────────────────────────────────────
hann = (0.5 * (1.0 - np.cos(2.0 * np.pi * np.arange(FRAME_SIZE)
               / (FRAME_SIZE - 1)))).astype(np.float32)

# ── Étape 3 : Application Hann sur la trame pré-accentuée ────────────────────
windowed = (preemph * hann).astype(np.float32)

# ── Étape 4 : FFT (convention vDSP : ×2 vs numpy) ────────────────────────────
n_fft_bins = N_FFT // 2
padded = np.zeros(N_FFT, dtype=np.float32)
padded[:FRAME_SIZE] = windowed
fft_mags = (np.abs(np.fft.rfft(padded))[:n_fft_bins].astype(np.float32) * 2.0)

# ── Étape 5 : Banc de filtres Mel (miroir exact de MelFilterbank::new) ───────
def hz_to_mel(hz): return 2595.0 * np.log10(1.0 + hz / 700.0)
def mel_to_hz(mel): return 700.0 * (10.0 ** (mel / 2595.0) - 1.0)

mel_min = hz_to_mel(FMIN)
mel_max = hz_to_mel(FMAX)
mel_points  = np.linspace(mel_min, mel_max, N_MELS + 2, dtype=np.float32)
bin_centers = (mel_to_hz(mel_points) * N_FFT / SAMPLE_RATE).astype(np.float32)

mel_matrix = np.zeros((N_MELS, n_fft_bins), dtype=np.float32)
for m in range(N_MELS):
    left   = float(bin_centers[m])
    center = float(bin_centers[m + 1])
    right  = float(bin_centers[m + 2])
    for k in range(n_fft_bins):
        fk = float(k)
        if left <= fk <= center:
            mel_matrix[m, k] = (fk - left) / max(center - left, F32_EPSILON)
        elif center < fk <= right:
            mel_matrix[m, k] = (right - fk) / max(right - center, F32_EPSILON)

# ── Étape 6 : Énergies Mel ────────────────────────────────────────────────────
mel_energies = (mel_matrix @ fft_mags).astype(np.float32)

# ── Étape 7 : Log-Mel ─────────────────────────────────────────────────────────
log_mel = np.log(np.where(mel_energies <= 0.0, F32_EPSILON, mel_energies)
                 .astype(np.float32)).astype(np.float32)

# ── Étape 8 : DCT-II (convention vDSP : scipy / 2) ───────────────────────────
def next_valid_dct_size(n):
    best = float('inf')
    for f in [1, 3, 5, 15]:
        size = f * 16
        while size < n:
            size *= 2
        if size < best:
            best = size
    return int(best)

n_padded = next_valid_dct_size(N_MELS)  # 48 pour N_MELS=40
padded_log = np.zeros(n_padded, dtype=np.float64)
padded_log[:N_MELS] = log_mel.astype(np.float64)
dct_output = (dct(padded_log, type=2, norm=None) / 2.0).astype(np.float32)
mfcc_frame0 = dct_output[:N_MFCC].tolist()

# ── Sauvegarde ────────────────────────────────────────────────────────────────
out = {
    "_comment": (
        "Référence intermédiaire pour les [TEST-N] du crate pipeline_dsp. "
        "Trame 0 du signal sinus 440 Hz, 16 kHz. Convention vDSP : FFT×2, DCT=scipy(norm=None)/2."
    ),
    "frame_size": FRAME_SIZE,
    "n_fft": N_FFT,
    "n_mels": N_MELS,
    "n_mfcc": N_MFCC,
    "sample_rate": SAMPLE_RATE,
    "preemphasis_frame0": preemph.tolist(),
    "hann_400": hann.tolist(),
    "fft_mags_frame0": fft_mags.tolist(),
    "mel_matrix_flat": mel_matrix.flatten().tolist(),
    "mel_energies_frame0": mel_energies.tolist(),
    "log_mel_frame0": log_mel.tolist(),
    "mfcc_frame0": mfcc_frame0,
}

OUTPUT.parent.mkdir(parents=True, exist_ok=True)
with open(OUTPUT, "w") as f:
    json.dump(out, f, indent=2)

print(f"✅ Fixture générée → {OUTPUT}")
print(f"   preemphasis[0]    = {preemph[0]:.8f}  (attendu ≈ sin(0) = 0.0)")
print(f"   hann[200]         = {hann[200]:.8f}  (attendu 1.0)")
print(f"   fft_mags[peak]    = {fft_mags.max():.4f}  @ bin {fft_mags.argmax()}")
print(f"   mel_energies max  = {mel_energies.max():.4f}")
print(f"   mfcc_frame0[:4]   = {mfcc_frame0[:4]}")
