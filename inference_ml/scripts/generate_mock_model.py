#!/usr/bin/env python3
"""
Génère le modèle CoreML mock WakeWordMock.mlmodelc dans fixtures/mock_model/.

Le modèle est un réseau de neurones minimaliste :
  - Entrée  : mfcc_input   shape [1, 98, 13]  Float32  (C×H×W)
  - Sortie  : classLabel_probs  shape [2]  Float32  (toujours [0.5, 0.5])

Note : Python 3.14 ne dispose pas de libcoremlpython/libmilstoragepython,
donc on utilise le format NeuralNetwork (pré-MIL) exporté en .mlmodel puis
compilé avec xcrun coremlcompiler.
"""

import os
import shutil
import subprocess
import sys

# ─── Chemins ─────────────────────────────────────────────────────────────────
SCRIPT_DIR   = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT    = os.path.dirname(SCRIPT_DIR)
FIXTURES_DIR = os.path.join(REPO_ROOT, "fixtures", "mock_model")
MLMODEL_PATH = os.path.join(FIXTURES_DIR, "WakeWordMock.mlmodel")
MLMODELC_DIR = os.path.join(FIXTURES_DIR, "WakeWordMock.mlmodelc")

os.makedirs(FIXTURES_DIR, exist_ok=True)

# ─── Imports coremltools ──────────────────────────────────────────────────────
try:
    import coremltools.models.datatypes as datatypes
    from coremltools.models.neural_network import NeuralNetworkBuilder
    from coremltools.models import MLModel
    import numpy as np
except ImportError as e:
    sys.exit(f"[ERREUR] Import échoué : {e}\nInstaller : pip3 install coremltools numpy")

# ─── Construction du modèle ───────────────────────────────────────────────────
# Input 3D [C=1, H=98, W=13] — shape compatible avec le compilateur CoreML
# (les inputs 4D [1,1,98,13] sont rejetés par le NeuralNetwork validator)
N_CHANNELS, N_FRAMES, N_MFCC = 1, 98, 13
N_IN  = N_CHANNELS * N_FRAMES * N_MFCC   # 1274
N_OUT = 2

print(f"Construction du modèle mock (input=[{N_CHANNELS},{N_FRAMES},{N_MFCC}], output=[{N_OUT}])…")

input_features  = [("mfcc_input",        datatypes.Array(N_CHANNELS, N_FRAMES, N_MFCC))]
output_features = [("classLabel_probs",  datatypes.Array(N_OUT))]

builder = NeuralNetworkBuilder(input_features, output_features)

# Flatten [1,98,13] → [1274]
builder.add_flatten("flatten", 0, "mfcc_input", "flat")

# Dense : W=0 (toutes les sorties = biais = 0.5)
W = np.zeros((N_OUT, N_IN), dtype=np.float32)
b = np.array([0.5, 0.5], dtype=np.float32)
builder.add_inner_product(
    "fc", W, b, N_IN, N_OUT, True, "flat", "classLabel_probs"
)

# ─── Export .mlmodel ──────────────────────────────────────────────────────────
model = MLModel(builder.spec)
model.save(MLMODEL_PATH)
print(f"  .mlmodel sauvegardé : {MLMODEL_PATH}")

# ─── Compilation → .mlmodelc ─────────────────────────────────────────────────
if os.path.exists(MLMODELC_DIR):
    shutil.rmtree(MLMODELC_DIR)

result = subprocess.run(
    ["xcrun", "coremlcompiler", "compile", MLMODEL_PATH, FIXTURES_DIR],
    capture_output=True,
    text=True,
)
if result.returncode != 0:
    sys.exit(
        f"[ERREUR] xcrun coremlcompiler a échoué :\n{result.stderr}"
    )

# ─── Vérification ────────────────────────────────────────────────────────────
assert os.path.isdir(MLMODELC_DIR), f".mlmodelc absent : {MLMODELC_DIR}"
files = os.listdir(MLMODELC_DIR)
print(f"  .mlmodelc compilé  : {MLMODELC_DIR}")
print(f"  Contenu : {files}")

size_kb = sum(
    os.path.getsize(os.path.join(MLMODELC_DIR, f))
    for f in files
    if os.path.isfile(os.path.join(MLMODELC_DIR, f))
) // 1024
print(f"  Taille  : ~{size_kb} Ko")
print("OK — modèle mock généré avec succès.")
