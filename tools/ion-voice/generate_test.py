"""Standalone Chatterbox-Turbo test: load, generate §B12 lines, benchmark."""
import sys, time
import torchaudio as ta
from chatterbox.tts_turbo import ChatterboxTurboTTS

started = time.time()
model = ChatterboxTurboTTS.from_pretrained(device="cuda")
print(f"load: {time.time() - started:.1f}s on cuda", flush=True)

lines = [
    ("routine", "All systems are nominal."),
    ("routine", "Your download has completed."),
    ("warning", "Battery level is critically low. Connect a power source."),
    ("critical", "Thermal activity has reached a critical level."),
]
for profile, text in lines:
    t0 = time.time()
    wav = model.generate(text, audio_prompt_path=None)
    dt = time.time() - t0
    out = f"tools/ion-voice/test-{profile}-{abs(hash(text)) % 10000}.wav"
    ta.save(out, wav.detach().cpu(), model.sr)
    print(f"gen[{profile}] {dt:.2f}s audio={wav.shape[-1] / model.sr:.1f}s -> {out}", flush=True)
print("DONE")
