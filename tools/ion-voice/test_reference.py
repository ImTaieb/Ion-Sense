"""Voice-lab: generate the standard ION test lines with a candidate reference.

Usage:
  tools/ion-voice/.venv/Scripts/python.exe tools/ion-voice/test_reference.py <candidate.wav>

Writes one WAV per test line to .tmp/ion-voice-tests/<candidate-name>/ so
candidate speaker references can be compared quickly. Generated audio is
never committed.
"""
import os
import sys
import time

import torch
import torchaudio as ta

TEST_LINES = [
    "All systems are nominal.",
    "Your download has completed.",
    "Battery level is critically low.",
    "Thermal activity has reached a critical level.",
    "You have a new message.",
    "I've detected unusual system activity.",
]


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__)
        return 2
    reference = os.path.abspath(sys.argv[1])
    if not os.path.isfile(reference):
        print(f"reference not found: {reference}")
        return 2

    from chatterbox.tts_turbo import ChatterboxTurboTTS

    device = "cuda" if torch.cuda.is_available() else "cpu"
    started = time.time()
    model = ChatterboxTurboTTS.from_pretrained(device=device)
    print(f"model ready on {device} in {time.time() - started:.1f}s")

    name = os.path.splitext(os.path.basename(reference))[0]
    out_dir = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                           ".tmp", "ion-voice-tests", name)
    os.makedirs(out_dir, exist_ok=True)

    for index, line in enumerate(TEST_LINES, 1):
        started = time.time()
        wav = model.generate(line, audio_prompt_path=reference)
        out_path = os.path.join(out_dir, f"{index:02d}.wav")
        ta.save(out_path, wav.detach().cpu(), model.sr)
        seconds = wav.shape[-1] / model.sr
        print(f"[{index}/{len(TEST_LINES)}] {time.time() - started:5.2f}s "
              f"audio {seconds:4.1f}s  {line}")
    print(f"done -> {out_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
