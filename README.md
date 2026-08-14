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
compact control window, use the gear to edit detector settings, and enable
**Launch at login** if desired. Quit it from the tray menu.

Local builds are unsigned unless a Windows code-signing certificate is supplied,
so Windows SmartScreen can identify them as coming from an unknown publisher.

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
- IMAP: password/app-password login and IMAP IDLE are implemented. Providers
  that require OAuth-only authentication need a future XOAUTH2 flow.
- `package_delivered` stays in the shared event contract and dev harness but has
  no detector in this phase.
