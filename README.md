# Ion Sense

Ion Sense is a tray-resident system-awareness utility built with Tauri 2. Its
native detectors route battery, temperature, download, email, and Discord bot
events through one typed dispatcher to a transparent, always-on-top HUD.

## Install on Windows

Release builds produce two equivalent x64 installers under
`src-tauri/target/release/bundle/`:

- `nsis/Ion Sense_<version>_x64-setup.exe` for a straightforward interactive
  installation.
- `msi/Ion Sense_<version>_x64_en-US.msi` for Windows Installer deployment.

Ion Sense starts as a tray application. Left-click the tray icon to open its
compact control window, select **Detector settings**, and enable
**Launch at login** if desired. Quit it from the tray menu.

Local builds are unsigned unless a Windows code-signing certificate is supplied,
so Windows SmartScreen can identify them as coming from an unknown publisher.

## Voice in a release installation

The installer includes the interface, Geist font and license, icons, voice worker
script, and current reference recording. **It does not install Python, PyTorch,
Chatterbox, CUDA, or model weights.** Visual alerts work without these components.
On a new PC, neural ION voice is unavailable until the optional runtime is set up.
Previously cached native audio or an available Windows speech voice can provide
fallback speech; a PC with neither remains visual-only. Settings reports this
state without blocking detector operation.

The current reference is an interim synthetic speaker; see
[voice reference notes](assets/voice/README.md). It is not a final British voice
identity. No pitch/filter changes are applied by this release pass.

Optional runtime setup for a Windows x64 installation:

1. Install Python 3.12 x64. Create a new environment on that PC; copying a
   developer `.venv` is not supported.
2. Create the environment at
   `%LOCALAPPDATA%\com.ionsense.desktop\voice\.venv` and install
   `chatterbox-tts==0.1.7` with its dependencies. Select a compatible PyTorch /
   torchaudio build for the PC (CPU or CUDA). This is an operator-managed setup,
   not an automated or clean-machine-validated installer step.
3. The interpreter must import `torch`, `torchaudio`, and
   `chatterbox.tts_turbo.ChatterboxTurboTTS`. Initial model loading requires
   access to Hugging Face and enough RAM/disk; CUDA additionally requires a
   compatible NVIDIA driver and sufficient VRAM. CPU fallback can be slower
   than an alert's speech deadline.
4. Restart Ion Sense. Open Detector settings and check voice status. A local
   health request to `http://127.0.0.1:8662/health` must report `ready` before
   neural voice is considered available. Listen to a real alert before rollout.

For an explicitly managed environment, `ION_VOICE_PYTHON` may name an absolute
interpreter path. The release always loads `voice/ion_tts.py` from its packaged
resource directory; it never searches the working directory for executable code.
Development builds can use `tools/ion-voice/.venv` inside the checkout.

Generated speech is cached under `%LOCALAPPDATA%\ion-sense\voice-cache`; native
last-good audio is under `%LOCALAPPDATA%\ion-sense\voice-lastgood`. Cache writes
do not target Program Files or the source checkout. `ION_VOICE_CACHE` can override
the synthesis cache, and `ION_VOICE_REFERENCE` can select an original, authorized
reference recording. Clear the native last-good cache when replacing the speaker;
that offline fallback cannot verify the new service fingerprint while offline.

The worker binds only to `127.0.0.1:8662`, restricts browser origins to the Tauri
app, bounds requests, and serializes synthesis. Closing the settings window keeps
monitoring active; **Quit** in the tray menu stops the app and its owned voice
process tree.

## Development

Install the JavaScript and Rust dependencies once, then run the fast development
loop with:

```powershell
pnpm install
pnpm tauri dev
```

The static frontend has no bundler. Tauri watches `frontend/` and recompiles the
Rust backend during `tauri dev`.

The original browser-only prototype remains at
`prototype/hud-prototype.html`. Open it with `?dev=1` to test the six visual
event variants without starting Tauri.

When `tauri dev` is running, the settings window exposes the same six test
events. They call the debug-only Rust command and travel through the real
central dispatcher. From the HUD developer console, the equivalent is:

```js
await window.ionSenseTest("battery_low")
```

Run the complete native test suite with:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
```

Create optimized installers only when preparing a release:

```powershell
pnpm tauri build
```

## Credentials

IMAP passwords and Discord bot tokens are written only to the operating-system
credential vault through the Rust `keyring` integration. They are never stored
in this repository, the JSON settings file, or application logs. Non-secret
detector settings live in the platform application-config directory and are
written atomically with backup recovery.

## Platform notes

- Windows: the HUD uses WebView2 and click-through cursor routing. Standard
  ACPI sensors do not reliably expose CPU package temperature on every PC; an
  unavailable sensor is reported as unavailable rather than as `0°C`.
- macOS: Ion Sense runs with accessory activation policy so it has no Dock
  icon. Transparent native-window behavior differs from Windows.
- Linux: tray icons, transparency, hardware sensors, and the keyring backend
  depend on the desktop compositor and Secret Service session.
- Discord: the official bot API can receive DMs sent to the bot and messages in
  channels the bot can access. It cannot monitor a normal user's private friend
  DMs; self-bot/user-token automation is intentionally unsupported.
- Downloads: completion is recognized when a browser's in-progress file
  (`.crdownload`, `.part`, `.partial`, or `.download`) is renamed or removed,
  or when a new file in the watched folder finishes growing.
- IMAP: password/app-password login and IMAP IDLE are implemented. Providers
  that require OAuth-only authentication need a future XOAUTH2 flow.
- `package_delivered` stays in the shared event contract and dev harness but has
  no detector in this phase.
