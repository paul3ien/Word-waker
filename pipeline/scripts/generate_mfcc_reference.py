#!/usr/bin/env python3
"""
Génère fixtures/reference_mfcc.json depuis un signal sinusoïdal 440 Hz à 16 kHz.
Utilisé comme référence numérique pour les tests TEST-N du crate pipeline_dsp.

Usage : python3 scripts/generate_mfcc_reference.py
"""
import json
import numpy as np
import librosa

SAMPLE_RATE = 16_000
DURATION_S = 1.0          # 1 seconde → 98 trames
FREQ_HZ = 440.0
N_FFT = 512
N_MELS = 40
N_MFCC = 13
FRAME_SIZE = 400           # 25 ms
HOP_SIZE = 160             # 10 ms
FMIN = 20.0
FMAX = 8_000.0
PRE_EMPHASIS = 0.97

OUTPUT = "fixtures/reference_mfcc.json"

# Générer le signal
t = np.linspace(0, DURATION_S, int(SAMPLE_RATE * DURATION_S), endpoint=False)
signal = np.sin(2 * np.pi * FREQ_HZ * t).astype(np.float32)

# Pré-accentuation
signal_pe = np.append(signal[0], signal[1:] - PRE_EMPHASIS * signal[:-1])

# MFCC via librosa (center=False pour correspondre au framing manuel)
mfcc = librosa.feature.mfcc(
    y=signal_pe,
    sr=SAMPLE_RATE,
    n_mfcc=N_MFCC,
    n_fft=N_FFT,
    hop_length=HOP_SIZE,
    win_length=FRAME_SIZE,
    n_mels=N_MELS,
    fmin=FMIN,
    fmax=FMAX,
    center=False,
    window="hann",
)
# mfcc shape: (13, n_frames) — on transpose en (n_frames, 13)
mfcc_T = mfcc.T.tolist()

out = {
    "_comment": "Généré par scripts/generate_mfcc_reference.py — ne pas modifier manuellement.",
    "sample_rate": SAMPLE_RATE,
    "n_frames": len(mfcc_T),
    "n_mfcc": N_MFCC,
    "freq_hz": FREQ_HZ,
    "mfcc": mfcc_T,
}

with open(OUTPUT, "w") as f:
    json.dump(out, f, indent=2)

print(f"✅ {len(mfcc_T)} trames × {N_MFCC} MFCC → {OUTPUT}")
