import concurrent.futures
import importlib.util
import io
import json
import pathlib
import tempfile
import threading
import unittest
import urllib.error
import urllib.request
import wave
from unittest.mock import patch

ROOT = pathlib.Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('ion_tts', ROOT / 'tools/ion-voice/ion_tts.py')
voice = importlib.util.module_from_spec(spec)
spec.loader.exec_module(voice)

class VoiceTests(unittest.TestCase):
    def setUp(self):
        voice.state = 'starting'
        voice.fingerprint = 'test-speaker'
        voice.wav_cache.clear()
        voice.pending_warm.clear()
        self.server = voice.ThreadingHTTPServer(('127.0.0.1', 0), voice.Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()

    def request(self, path, payload=None, origin='http://tauri.localhost', raw=None, headers=None):
        data = raw if raw is not None else (json.dumps(payload).encode() if payload is not None else None)
        req = urllib.request.Request(f'http://127.0.0.1:{self.server.server_port}{path}', data=data,
            headers={'Host':'127.0.0.1:8662','Origin':origin,'Content-Type':'application/json', **(headers or {})})
        try:
            response = urllib.request.urlopen(req, timeout=3)
        except urllib.error.HTTPError as error:
            response = error
        return response.status, response.read(), response.headers

    def test_health_and_origin(self):
        code, body, headers = self.request('/health')
        self.assertEqual(code, 200)
        self.assertEqual(json.loads(body)['state'], 'starting')
        self.assertEqual(headers['Access-Control-Allow-Origin'], 'http://tauri.localhost')
        self.assertEqual(self.request('/health', origin='https://untrusted.example')[0], 403)
        self.assertEqual(self.request('/health', headers={'Host':'untrusted.example'})[0], 403)

    def test_request_bounds(self):
        self.assertEqual(self.request('/speak', raw=b'bad json')[0], 400)
        self.assertEqual(self.request('/speak', payload=[])[0], 400)
        self.assertEqual(self.request('/speak', raw=b'x'*32769)[0], 413)
        self.assertEqual(self.request('/warm', payload={'lines':[{}]*25})[0], 400)
        self.assertEqual(self.request('/speak', payload={'text':'hello'})[0], 503)
        voice.state = 'ready'
        self.assertEqual(self.request('/speak', payload={'text':'hello','profile':[]})[0], 400)

    def test_warm_queue_bounded(self):
        for i in range(4):
            self.assertEqual(self.request('/warm', payload={'lines':[{'text':f'{i}:{j}'} for j in range(24)]})[0], 202)
        self.assertEqual(len(voice.pending_warm), 24)

    def test_cache_hit_does_not_load_model(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(voice, 'DISK_CACHE_DIR', directory):
            buffer = io.BytesIO()
            with wave.open(buffer, 'wb') as wav:
                wav.setnchannels(1); wav.setsampwidth(2); wav.setframerate(24000); wav.writeframes(b'\0\0'*2400)
            audio = buffer.getvalue()
            pathlib.Path(voice.disk_path(voice.cache_key('hello','routine'))).write_bytes(audio)
            self.assertEqual(voice.synthesize('hello','routine'), (audio, 0.0))
            pathlib.Path(voice.disk_path(voice.cache_key('hello','routine'))).unlink()
            self.assertEqual(voice.synthesize('hello','routine'), (audio, 0.0))

    def test_generation_serialized(self):
        active = 0
        peak = 0
        def fake(*args):
            nonlocal active, peak
            active += 1; peak = max(peak,active)
            threading.Event().wait(.01)
            active -= 1
            return b'audio', 0
        with patch.object(voice, '_synthesize', fake), concurrent.futures.ThreadPoolExecutor(6) as pool:
            list(pool.map(lambda _: voice.synthesize('hello','routine'), range(12)))
        self.assertEqual(peak, 1)

    def test_failure_recovery_and_safe_error(self):
        voice.state = 'ready'
        with patch.object(voice, 'synthesize', side_effect=RuntimeError('sensitive implementation detail')):
            code, body, _ = self.request('/speak', payload={'text':'hello'})
            self.assertEqual(code, 500)
            self.assertNotIn(b'sensitive', body)
        with patch.object(voice, 'synthesize', return_value=(b'audio',0)):
            self.assertEqual(self.request('/speak', payload={'text':'hello'})[:2], (200,b'audio'))

    def test_missing_reference_is_explicit(self):
        with patch.object(voice, 'REFERENCE_CANDIDATES', []):
            voice.load_model()
            self.assertEqual((voice.state,voice.state_detail), ('failed','reference-missing'))

    def test_starting_cache_bypasses_busy_model_for_every_profile(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(voice, 'DISK_CACHE_DIR', directory):
            audio = b'RIFF' + b'\0' * 4 + b'WAVE' + b'\0' * 32
            lines = [('battery', 'warning'), ('temperature', 'critical'),
                     ('package', 'routine'), ('friend', 'routine')]
            for text, profile in lines:
                pathlib.Path(voice.disk_path(voice.cache_key(text, profile))).write_bytes(audio)
            # The request handler runs in another thread. Holding this lock
            # proves serving existing audio does not wait for generation.
            with voice.model_lock, patch.object(voice, 'synthesize', side_effect=AssertionError('model used')):
                for text, profile in lines:
                    code, body, headers = self.request('/speak', {'text': text, 'profile': profile})
                    self.assertEqual((code, body), (200, audio))
                    self.assertEqual(headers['X-ION-Voice-Provider'], 'chatterbox-cache')
            voice.fingerprint = 'different-speaker'
            self.assertEqual(self.request('/speak', {'text': 'friend'})[0], 503)

    def test_starting_cache_miss_recovers_without_provider_switch(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(voice, 'DISK_CACHE_DIR', directory):
            self.assertEqual(self.request('/speak', {'text': 'new line'})[0], 503)
            voice.state = 'ready'
            with patch.object(voice, 'synthesize', return_value=(b'audio', 0.2)):
                code, _, headers = self.request('/speak', {'text': 'new line'})
                self.assertEqual(code, 200)
                self.assertEqual(headers['X-ION-Voice-Provider'], 'chatterbox-live')

    def test_invalid_disk_cache_is_a_miss(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(voice, 'DISK_CACHE_DIR', directory):
            pathlib.Path(voice.disk_path(voice.cache_key('bad', 'routine'))).write_bytes(b'partial')
            self.assertIsNone(voice.cached_audio('bad', 'routine'))

if __name__ == '__main__': unittest.main(verbosity=2)
