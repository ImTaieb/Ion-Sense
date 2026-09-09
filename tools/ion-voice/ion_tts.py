"""ION voice — local Chatterbox-Turbo worker.

Loads the model ONCE (only when an ION speaker reference exists) and serves
synthesis over localhost HTTP. Alert copy is fixed per event type, so every
generated line is cached in memory AND on disk: the first request of a line
synthesizes (~2s), every later one — including after app restarts — serves
from cache in milliseconds.

Speaker identity: Chatterbox-Turbo clones the reference clip. The stock
Chatterbox voice (female) is NEVER used as ION: without a readable reference
the service reports FAILED/reference-missing and stays idle, and the HUD
falls back to the male Web Speech voice.

Endpoints:
  GET  /health -> {"state": "starting|ready|failed", "device": str|null,
                   "reference": str|null, "fingerprint": str|null,
                   "cached": int}
  POST /speak  {"text": str, "profile": "routine|warning|critical"}
            -> audio/wav bytes (cached ION audio is available during startup)
  POST /warm   {"lines": [{"text": str, "profile": str}]}
            -> 202; lines are queued and synthesized once ready

Runs until killed. Idle cost: model resident in VRAM (only with a reference),
no compute loop.
"""
import io
import json
import os
import time
import hashlib
import threading

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

DEVICE_CANDIDATES = ["cuda", "cpu"]
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
REPO_DIR = os.path.dirname(os.path.dirname(SCRIPT_DIR))
DISK_CACHE_DIR = os.environ.get("ION_VOICE_CACHE") or os.path.join(
    os.environ.get("LOCALAPPDATA") or os.path.expanduser("~/.cache"),
    "ion-sense", "voice-cache",
)
# Reference resolution: env override, then repo assets, then tool-local assets.
REFERENCE_CANDIDATES = [
    os.environ.get("ION_VOICE_REFERENCE") or "",
    os.path.join(SCRIPT_DIR, "ion-reference.wav"),
    os.path.join(REPO_DIR, "assets", "voice", "ion-reference.wav"),
    os.path.join(SCRIPT_DIR, "assets", "ion-reference.wav"),
]

tts = None
device_used = None
state = "starting"          # starting | ready | failed
state_detail = None         # why failed (e.g. reference-missing)
reference_path = None
fingerprint = None
wav_cache = {}
pending_warm = []
model_lock = threading.RLock()
torch = None
ta = None

# Delivery profiles: same speaker, subtly different authority. Deliberately
# restrained — Chatterbox exposes exaggeration/pace knobs, not full control.
PROFILE_EXAGGERATION = {"routine": 0.4, "warning": 0.35, "critical": 0.3}
WORKER_LOG = os.path.join(os.path.dirname(os.path.abspath(__file__)), "worker.log")


def log(msg):
    """Lifecycle event -> stdout AND a durable file next to the worker.
    The file survives process death, so a vanished worker always leaves
    an explanation (OOM aborts kill the interpreter before stdout flushes)."""
    line = f"[ion-voice] {msg}"
    print(line, flush=True)
    try:
        stat = os.stat(WORKER_LOG) if os.path.exists(WORKER_LOG) else None
        if stat and stat.st_size > 512 * 1024:  # rotate at 512 KB
            os.replace(WORKER_LOG, WORKER_LOG + ".1")
        with open(WORKER_LOG, "a", encoding="utf-8") as handle:
            handle.write(line + "\n")
    except OSError:
        pass


def resolve_reference():
    for candidate in REFERENCE_CANDIDATES:
        if candidate and os.path.isfile(candidate) and os.path.getsize(candidate) > 0:
            with open(candidate, "rb") as handle:
                digest = hashlib.sha256(handle.read()).hexdigest()[:16]
            return candidate, digest
    return None, None


def cache_key(text, profile):
    # Text + speaker (reference fingerprint) + profile: replacing the
    # reference changes every key, so stale-speaker audio can never replay.
    material = f"v3|{fingerprint}|{profile}|{text}".encode()
    return hashlib.sha256(material).hexdigest()[:24]


def disk_path(key):
    return os.path.join(DISK_CACHE_DIR, f"{key}.wav")


def cached_audio(text, profile):
    """Read completed audio without waiting for model loading or generation."""
    key = cache_key(text, profile)
    if key in wav_cache:
        return wav_cache[key]
    try:
        with open(disk_path(key), "rb") as handle:
            audio = handle.read()
        if audio[:4] != b"RIFF" or audio[8:12] != b"WAVE":
            return None
        wav_cache[key] = audio
        return audio
    except OSError:
        return None


MAX_TEXT_CHARS = 400


def polish_audio(audio, sample_rate):
    """Trim dead air (keep a 25ms natural lead-in), apply 15ms edge fades,
    and normalize peaks to 0.85 so every alert reads at consistent loudness.
    No other processing — pristine speech, just immediate."""
    import torch as _torch
    samples = audio.squeeze()
    if samples.numel() < sample_rate // 10:
        return audio
    amplitude = samples.abs()
    peak = float(amplitude.max())
    threshold = max(peak * 0.015, 1e-4)
    active = (amplitude > threshold).nonzero().flatten()
    if active.numel() == 0:
        return audio
    lead = int(sample_rate * 0.025)
    tail_pad = int(sample_rate * 0.06)
    start = max(0, int(active[0]) - lead)
    end = min(samples.numel(), int(active[-1]) + tail_pad)
    samples = samples[start:end]
    fade = int(sample_rate * 0.015)
    if fade > 0 and samples.numel() > fade * 2:
        ramp = _torch.linspace(0.0, 1.0, fade)
        samples = samples.clone()
        samples[:fade] *= ramp
        samples[-fade:] *= ramp.flip(0)
    current_peak = float(samples.abs().max())
    if current_peak > 0:
        samples = samples * min(0.85 / current_peak, 4.0)
    return samples.unsqueeze(0)


def synthesize(text, profile):
    # One model invocation/cache writer at a time, including startup warming.
    with model_lock:
        return _synthesize(text, profile)


def _synthesize(text, profile):
    key = cache_key(text, profile)
    cached = cached_audio(text, profile)
    if cached is not None:
        return cached, 0.0
    cached_disk = disk_path(key)
    started = time.time()
    with torch.no_grad():
        wav = tts.generate(
            text,
            audio_prompt_path=reference_path,
            exaggeration=PROFILE_EXAGGERATION.get(profile, 0.4),
            cfg_weight=0.5,
            temperature=0.65,
        )
    audio = wav.detach().cpu()
    audio = polish_audio(audio, tts.sr)
    buffer = io.BytesIO()
    # 16-bit PCM: universally decodable (rodio/hound rejects IEEE-float WAVs).
    ta.save(buffer, audio, tts.sr, format="wav", encoding="PCM_S", bits_per_sample=16)
    elapsed = time.time() - started
    audio = buffer.getvalue()
    wav_cache[key] = audio
    try:
        os.makedirs(DISK_CACHE_DIR, exist_ok=True)
        temporary = cached_disk + ".tmp"
        with open(temporary, "wb") as handle:
            handle.write(audio)
        os.replace(temporary, cached_disk)
    except OSError as error:
        # A read-only/full cache must never discard successfully generated audio.
        print(f"Ion Sense voice cache unavailable: {error}", flush=True)
    print(f"[ion-voice] '{text[:40]}' ({profile}) in {elapsed:.2f}s", flush=True)
    return audio, elapsed


def load_model():
    global tts, device_used, state, state_detail, reference_path, fingerprint, torch, ta
    import platform
    log(f"worker pid={os.getpid()} python={platform.python_version()} "
        f"machine={platform.machine()}")
    log(f"model_cache={os.environ.get('HF_HOME', os.path.expanduser('~/.cache/huggingface'))}")
    reference_path, fingerprint = resolve_reference()
    if reference_path is None:
        # NEVER fall back to the stock Chatterbox speaker as ION.
        state = "failed"
        state_detail = "reference-missing"
        log("ION VOICE REFERENCE: MISSING — FALLBACK ACTIVE "
            "(supply assets/voice/ion-reference.wav)")
        return
    log(f"ION VOICE REFERENCE: {os.path.basename(reference_path)} (fp {fingerprint})")
    try:
        import torch
        import torchaudio as ta
        from chatterbox.tts_turbo import ChatterboxTurboTTS
        log(f"torch={torch.__version__} cuda_build={torch.version.cuda} "
            f"cuda_available={torch.cuda.is_available()}")
        if torch.cuda.is_available():
            props = torch.cuda.get_device_properties(0)
            log(f"gpu={props.name} vram_total={props.total_memory // (1024 * 1024)}MB")
    except Exception as error:  # noqa: BLE001
        state = "failed"
        state_detail = "dependencies-unavailable"
        log(f"dependencies unavailable: {error}")
        return
    for device in DEVICE_CANDIDATES:
        started = time.time()
        log(f"model-load start device={device}")
        try:
            tts = ChatterboxTurboTTS.from_pretrained(device=device)
            device_used = device
            log(f"model-load end device={device} in {time.time() - started:.1f}s")
            break
        except Exception as error:  # noqa: BLE001 - fall through to CPU
            log(f"{device} model load failed: {error}")
    if tts is None:
        # Exit so the supervisor respawns later (e.g. once memory pressure
        # passes); the HUD falls back to Web Speech meanwhile.
        state = "failed"
        state_detail = "model-unavailable"
        print("[ion-voice] model load failed; exiting for supervised respawn", flush=True)
        time.sleep(2)
        os._exit(1)
    state = "ready"
    log(f"READY device={device_used} load={time.time() - started:.1f}s "
        f"vram={torch.cuda.memory_allocated(0) // (1024 * 1024) if device_used == 'cuda' else 0}MB")
    with model_lock:
        queued = pending_warm[:]
        pending_warm.clear()
    for line in queued:
        try:
            synthesize(str(line.get("text", "")), str(line.get("profile", "routine")))
        except Exception as error:  # noqa: BLE001
            print(f"[ion-voice] warm failed: {error}", flush=True)


MAX_BODY_BYTES = 32768
MAX_WARM_LINES = 24
ALLOWED_ORIGINS = {"http://tauri.localhost", "https://tauri.localhost", "tauri://localhost"}


def normalized_line(line):
    if not isinstance(line, dict):
        raise ValueError("invalid line")
    text = line.get("text", "")
    profile = line.get("profile", "routine")
    if not isinstance(text, str) or not text.strip() or len(text) > MAX_TEXT_CHARS:
        raise ValueError("invalid text")
    if not isinstance(profile, str) or profile not in PROFILE_EXAGGERATION:
        raise ValueError("invalid profile")
    return {"text": text.strip(), "profile": profile}


class Handler(BaseHTTPRequestHandler):
    def setup(self):
        super().setup()
        self.connection.settimeout(5)

    def _allowed(self):
        origin = self.headers.get("Origin")
        if self.headers.get("Host") not in {"127.0.0.1:8662", "localhost:8662"} or (origin is not None and origin not in ALLOWED_ORIGINS):
            self._send(403, b'{"error":"origin not allowed"}')
            return False
        return True

    def _cors(self):
        origin = self.headers.get("Origin")
        if origin in ALLOWED_ORIGINS:
            self.send_header("Access-Control-Allow-Origin", origin)
            self.send_header("Vary", "Origin")

    def _send(self, code, body, content_type="application/json", provider=None):
        self.send_response(code)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        if provider:
            self.send_header("X-ION-Voice-Provider", provider)
            self.send_header("Access-Control-Expose-Headers", "X-ION-Voice-Provider")
        self._cors()
        self.end_headers()
        self.wfile.write(body)

    def do_OPTIONS(self):  # noqa: N802 - CORS preflight for the WebView fetch
        if not self._allowed():
            return
        self.send_response(204)
        self._cors()
        self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.send_header("Content-Length", "0")
        self.end_headers()

    def do_GET(self):  # noqa: N802 - stdlib naming
        if not self._allowed():
            return
        if self.path == "/health":
            body = json.dumps({
                "state": state,
                "device": device_used,
                "detail": state_detail,
                "reference": os.path.basename(reference_path) if reference_path else None,
                "fingerprint": fingerprint,
                "cached": len(wav_cache),
            }).encode()
            self._send(200, body)
        else:
            self._send(404, b"{}")

    def do_POST(self):  # noqa: N802
        if not self._allowed():
            return
        try:
            length = int(self.headers.get("Content-Length", 0))
            if not 0 < length <= MAX_BODY_BYTES:
                self._send(413, b'{"error":"request too large or empty"}')
                return
            if self.headers.get_content_type() != "application/json":
                self._send(415, b'{"error":"JSON required"}')
                return
            payload = json.loads(self.rfile.read(length))
            if not isinstance(payload, dict):
                raise ValueError("invalid request")
        except (ValueError, UnicodeError, OSError):
            self._send(400, b'{"error":"invalid request"}')
            return
        if self.path == "/warm":
            # Warm known alert lines; before ready they are queued and run as
            # soon as the model (and reference) are available.
            try:
                lines = payload.get("lines", [])
                if not isinstance(lines, list) or len(lines) > MAX_WARM_LINES:
                    raise ValueError("invalid warm list")
                lines = [normalized_line(line) for line in lines]
            except ValueError:
                self._send(400, b'{"error":"invalid warm list"}')
                return
            if state == "ready":
                for line in lines[-24:]:
                    try:
                        text = str(line.get("text", ""))[:MAX_TEXT_CHARS]
                        profile = line.get("profile", "routine")
                        if text:
                            synthesize(text, str(profile if profile in PROFILE_EXAGGERATION else "routine"))
                    except Exception as error:  # noqa: BLE001
                        print(f"[ion-voice] warm failed: {error}", flush=True)
            else:
                with model_lock:
                    pending_warm[:] = (pending_warm + lines)[-MAX_WARM_LINES:]
            self._send(202, b"{}")
            return
        if self.path != "/speak":
            self._send(404, b"{}")
            return
        try:
            line = normalized_line(payload)
        except ValueError:
            self._send(400, b'{"error":"invalid line"}')
            return
        # Speaker fingerprint is resolved before serving HTTP. Cached audio
        # needs neither a ready model nor its generation lock; startup must
        # not turn a known ION line into Windows speech.
        audio = cached_audio(line["text"], line["profile"]) if fingerprint else None
        if audio is not None:
            self._send(200, audio, "audio/wav", "chatterbox-cache")
            return
        if state != "ready":
            self._send(503, json.dumps({"state": state, "detail": state_detail}).encode())
            return
        try:
            audio, _elapsed = synthesize(line["text"], line["profile"])
        except Exception as error:  # noqa: BLE001 - never break the alert flow
            print(f"Ion Sense voice synthesis failed: {error}", flush=True)
            self._send(500, b'{"error":"voice unavailable"}')
            return
        self._send(200, audio, "audio/wav",
                   "chatterbox-cache" if _elapsed == 0 else "chatterbox-live")

    def log_message(self, *args):  # silence default request logging
        pass


if __name__ == "__main__":
    import sys
    if "--service" in sys.argv:
        reference_path, fingerprint = resolve_reference()
        server = ThreadingHTTPServer(("127.0.0.1", 8662), Handler)
        threading.Thread(target=load_model, daemon=True).start()
        print("[ion-voice] service listening on 127.0.0.1:8662", flush=True)
        server.serve_forever()
    else:
        # Supervisor: model loading aborts the whole interpreter when the
        # machine is under memory pressure (uncatchable Rust allocation
        # failure), so the worker is respawned with backoff — indefinitely,
        # so the neural voice becomes available as soon as the machine can
        # load it. While the worker is down the port is closed and the HUD
        # uses its Web Speech fallback.
        import subprocess
        import sys
        attempt = 0
        while True:
            attempt += 1
            print(f"[ion-voice] worker starting (attempt {attempt})", flush=True)
            result = subprocess.run([sys.executable, os.path.abspath(__file__), "--service"])
            print(f"[ion-voice] worker exited ({result.returncode}); "
                  f"respawn in 90s", flush=True)
            time.sleep(90)
