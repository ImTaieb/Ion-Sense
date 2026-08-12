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

## Credentials

IMAP passwords and Discord bot tokens are written only to the operating system
credential vault through the Rust `keyring` integration. They are never stored
in this repository or in the JSON settings file.

