#!/usr/bin/env python3
"""
Génère fixtures/reference_mfcc.json depuis un signal sinusoïdal 440 Hz à 16 kHz.
Ce script reproduit EXACTEMENT le comportement de notre pipeline Rust :

  - Pré-accentuation : IIR premier ordre appliqué trame par trame avec état
    (last_sample = dernier échantillon brut de la trame précédente).
  - Fenêtrage : Hann : w[n] = 0.5 * (1 - cos(2π*n/(N-1)))
  - FFT : magnitudes = |numpy.fft.rfft| * 2.0  (vDSP_fft_zrip × 2 par rapport à la norme standard)
  - Filterbank Mel : filtres triangulaires non normalisés, hauteur 1.0
  - Logarithme : ln naturel, clamp à f32::EPSILON avant le log
  - DCT-II : scipy.fft.dct(x, type=2, norm=None) / 2.0  (convention vDSP)

Usage : python3 scripts/generate_mfcc_reference.py
"""
import json
import numpy as np
from scipy.fft import dct

SAMPLE_RATE = 16_000
DURATION_S = 1.0
FREQ_HZ = 440.0
FRAME_SIZE = 400
HOP_SIZE = 160
N_FFT = 512
N_MELS = 40
N_MFCC = 13
FMIN = 20.0
FMAX = 8_000.0
ALPHA = 0.97
F32_EPSILON = np.finfo(np.float32).eps

OUTPUT = "fixtures/reference_mfcc.json"

# ---------------------------------------------------------------------------
# Signal
# ---------------------------------------------------------------------------
n_samples = int(SAMPLE_RATE * DURATION_S)
signal = np.sin(2 * np.pi * FREQ_HZ * np.arange(n_samples) / SAMPLE_RATE).astype(np.float32)

# ---------------------------------------------------------------------------
# Hann window (formule identique à notre Rust : w[n] = 0.5*(1-cos(2π*n/(N-1))))
# ---------------------------------------------------------------------------
hann = (0.5 * (1.0 - np.cos(2.0 * np.pi * np.arange(FRAME_SIZE) / (FRAME_SIZE - 1)))).astype(np.float32)

# ---------------------------------------------------------------------------
# Filterbank Mel (miroir exact de MelFilterbank::new en Rust)
# ---------------------------------------------------------------------------
def hz_to_mel(hz):
    return 2595.0 * np.log10(1.0 + hz / 700.0)

def mel_to_hz(mel):
    return 700.0 * (10.0 ** (mel / 2595.0) - 1.0)

n_fft_bins = N_FFT // 2
mel_min = hz_to_mel(FMIN)
mel_max = hz_to_mel(FMAX)
n_points = N_MELS + 2
mel_points = np.linspace(mel_min, mel_max, n_points, dtype=np.float32)
bin_centers = (mel_to_hz(mel_points) * N_FFT / SAMPLE_RATE).astype(np.float32)

matrix = np.zeros((N_MELS, n_fft_bins), dtype=np.float32)
for m in range(N_MELS):
    left = float(bin_centers[m])
    center = float(bin_centers[m + 1])
    right = float(bin_centers[m + 2])
    for k in range(n_fft_bins):
        fk = float(k)
        if left <= fk <= center:
            val = (fk - left) / max(center - left, F32_EPSILON)
        elif center < fk <= right:
            val = (right - fk) / max(right - center, F32_EPSILON)
        else:
            val = 0.0
        matrix[m, k] = val

# ---------------------------------------------------------------------------
# Traitement trame par trame (miroir de FrameProcessor::process_frame)
# ---------------------------------------------------------------------------
mfcc_frames = []
last_sample = 0.0  # état pré-accentuation
frame_start = 0

while frame_start + FRAME_SIZE <= len(signal):
    # 1. Extraire la trame brute
    raw_frame = signal[frame_start:frame_start + FRAME_SIZE].copy()

    # 2. Pré-accentuation IIR avec état (miroir de PreEmphasis::apply)
    frame = np.empty(FRAME_SIZE, dtype=np.float32)
    prev = last_sample
    for n in range(FRAME_SIZE):
        current = float(raw_frame[n])
        frame[n] = current - ALPHA * prev
        prev = current
    # Mettre à jour l'état : last_sample = dernier échantillon BRUT
    last_sample = float(raw_frame[-1])

    # 3. Fenêtrage de Hann
    frame = frame * hann

    # 4. Zero-padding et FFT
    padded = np.zeros(N_FFT, dtype=np.float32)
    padded[:FRAME_SIZE] = frame
    # vDSP_fft_zrip donne 2× la norme standard
    rfft_out = np.fft.rfft(padded)[:n_fft_bins]
    mags = np.abs(rfft_out).astype(np.float32) * 2.0

    # 5. Filterbank Mel
    mel_energies = (matrix @ mags).astype(np.float32)

    # 6. Logarithme naturel avec clamp epsilon (miroir de log_mel_energies)
    mel_energies = np.where(mel_energies <= 0.0, F32_EPSILON, mel_energies).astype(np.float32)
    log_mel = np.log(mel_energies).astype(np.float32)

    # 7. DCT-II (convention vDSP : scipy norm=None / 2)
    # next_valid_dct_size(40) = 48 (= 3*2^4), même calcul que Rust VDspDct::new(40)
    def next_valid_dct_size(n):
        best = float('inf')
        for f in [1, 3, 5, 15]:
            size = f * 16  # k_min = 4
            while size < n:
                size *= 2
            if size < best:
                best = size
        return int(best)

    n_padded = next_valid_dct_size(N_MELS)
    padded_log_mel = np.zeros(n_padded, dtype=np.float64)
    padded_log_mel[:N_MELS] = log_mel.astype(np.float64)
    dct_out = (dct(padded_log_mel, type=2, norm=None) / 2.0).astype(np.float32)
    mfcc_frames.append(dct_out[:N_MFCC].tolist())

    frame_start += HOP_SIZE

# ---------------------------------------------------------------------------
# Sauvegarde
# ---------------------------------------------------------------------------
out = {
    "_comment": (
        "Généré par scripts/generate_mfcc_reference.py — ne pas modifier manuellement. "
        "Reproduit la convention vDSP : FFT×2 (vs numpy), DCT = scipy(norm=None)/2."
    ),
    "sample_rate": SAMPLE_RATE,
    "n_frames": len(mfcc_frames),
    "n_mfcc": N_MFCC,
    "freq_hz": FREQ_HZ,
    "mfcc": mfcc_frames,
}

with open(OUTPUT, "w") as f:
    json.dump(out, f, indent=2)

print(f"✅ {len(mfcc_frames)} trames × {N_MFCC} MFCC → {OUTPUT}")
print(f"   Première trame[:4] : {mfcc_frames[0][:4]}")
