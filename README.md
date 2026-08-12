# Ion Sense

Ion Sense is a tray-resident desktop HUD built with Tauri 2. The native backend
detects local system events and routes every notification through one typed event
contract to a transparent, always-on-top webview.

## Development

This phase is intentionally development-only. Install the JavaScript and Rust
dependencies once, then keep the app running with:

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

## Credentials

IMAP passwords and Discord bot tokens are written only to the operating system
credential vault through the Rust `keyring` integration. They are never stored
in this repository or in the JSON settings file.

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
