# ION Sense Voice Pack

ION Sense works without neural voice — alerts render visually and speak via
the built-in Web Speech fallback. The optional Voice Pack adds local neural
TTS (Chatterbox-Turbo on CUDA/CPU) with the ION speaker identity.

## Layout (Voice Pack root)

```
voice/
├── .venv/                 # Python 3.12+ venv with chatterbox-tts + torch
│   └── Scripts/python.exe
├── worker.log             # lifecycle log (auto-rotated at 512 KB)
├── cache/                 # generated PCM16 WAVs (auto-managed)
└── (any files)
```

Reference voice (supply separately):

```
<repo>/assets/voice/ion-reference.wav
```

## Install

1. Copy `tools/ion-voice/` into `%LOCALAPPDATA%/IonSense/voice/`
   (or use `ION_VOICE_PYTHON` env var to point at the venv python).
2. Place `ion-reference.wav` in `assets/voice/` (or next to the exe).
3. Restart ION Sense.

## Detection

ION Sense searches (in order):
1. `ION_VOICE_PYTHON` env var
2. `%LOCALAPPDATA%/IonSense/voice/.venv/Scripts/python.exe` (installed)
3. `<exe_dir>/tools/ion-voice/` (portable)
4. `CARGO_MANIFEST_DIR/../tools/ion-voice/` (dev only)

Reference voice candidates:
1. `<resource_dir>/voice/ion-reference.wav`
2. `<exe_dir>/assets/voice/ion-reference.wav`

If no reference is found, the neural path stays down and the Web Speech
fallback is used. No stock voice is ever used as ION.

## Requirements

- NVIDIA GPU with CUDA (or CPU-only inference, slower)
- ~4 GB VRAM for the model
- Model weights auto-download from Hugging Face on first model load (~3 GB)
- Disk cache persists generated WAVs across restarts
