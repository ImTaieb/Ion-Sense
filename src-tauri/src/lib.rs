mod credentials;
mod detectors;
mod dispatcher;
mod event;
mod settings;

use std::path::PathBuf;
use std::sync::{
    Arc, Mutex, RwLock,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::Duration;

use anyhow::Context;
use credentials::{SecretKind, credential_account, delete_secret, get_secret, set_secret};
use detectors::DetectorRuntime;
use dispatcher::EventDispatcher;
use event::{IonSenseEvent, IonSenseEventType, Severity};
use serde::Serialize;
use settings::AppSettings;
#[cfg(any(target_os = "windows", target_os = "macos"))]
use tauri::window::{Color, Effect, EffectState, EffectsBuilder};
use tauri::{
    App, AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size, State,
    WebviewWindow, WindowEvent,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_autostart::ManagerExt;
use tokio::sync::{Notify, mpsc};

const EVENT_NAME: &str = "ion-sense://trigger";

struct NativeState {
    settings: RwLock<AppSettings>,
    settings_path: PathBuf,
    detectors: Mutex<Option<DetectorRuntime>>,
    dispatcher: EventDispatcher,
    hud: Arc<HudLifecycle>,
}

struct HudLifecycle {
    ready: AtomicBool,
    delivered_timestamp: AtomicU64,
    acknowledged_timestamp: AtomicU64,
    changed: Notify,
}

impl HudLifecycle {
    fn new() -> Self {
        Self {
            ready: AtomicBool::new(false),
            delivered_timestamp: AtomicU64::new(0),
            acknowledged_timestamp: AtomicU64::new(0),
            changed: Notify::new(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CredentialStatus {
    imap_configured: bool,
    discord_configured: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeInfo {
    dev: bool,
    platform: &'static str,
    settings_path: String,
}

#[tauri::command]
fn get_settings(
    window: WebviewWindow,
    state: State<'_, NativeState>,
) -> Result<AppSettings, String> {
    require_window(&window, "settings")?;
    state
        .settings
        .read()
        .map(|settings| settings.clone())
        .map_err(|_| "settings lock is unavailable".to_owned())
}

#[tauri::command]
async fn save_settings(
    window: WebviewWindow,
    app: AppHandle,
    state: State<'_, NativeState>,
    settings: AppSettings,
    imap_password: Option<String>,
    discord_token: Option<String>,
) -> Result<AppSettings, String> {
    require_window(&window, "settings")?;
    let settings = settings.sanitized();
    settings
        .validate()
        .map_err(|error| format!("Invalid settings: {error}"))?;
    let previous = state
        .settings
        .read()
        .map_err(|_| "settings lock is unavailable".to_owned())?
        .clone();

    let imap_password = imap_password.filter(|secret| !secret.is_empty());
    let discord_token = discord_token.filter(|secret| !secret.is_empty());
    if imap_password.is_some() && settings.email.username.is_empty() {
        return Err("An IMAP username is required before saving its password".into());
    }

    let imap_account = credential_account(SecretKind::ImapPassword, &settings.email.username);
    let discord_account = credential_account(SecretKind::DiscordBotToken, "discord-bot");
    if settings.email.enabled
        && imap_password.is_none()
        && get_secret(SecretKind::ImapPassword, &imap_account).is_err()
    {
        return Err("Enable email only after providing an OS-keychain password".into());
    }
    if settings.discord.enabled
        && discord_token.is_none()
        && get_secret(SecretKind::DiscordBotToken, &discord_account).is_err()
    {
        return Err("Enable Discord only after providing an OS-keychain bot token".into());
    }

    let prior_imap_secret = imap_password
        .as_ref()
        .and_then(|_| get_secret(SecretKind::ImapPassword, &imap_account).ok());
    let prior_discord_secret = discord_token
        .as_ref()
        .and_then(|_| get_secret(SecretKind::DiscordBotToken, &discord_account).ok());
    if let Some(secret) = imap_password.as_deref() {
        set_secret(SecretKind::ImapPassword, &imap_account, secret)
            .map_err(|error| format!("Could not save the IMAP password: {error:#}"))?;
    }
    if let Some(secret) = discord_token.as_deref()
        && let Err(error) = set_secret(SecretKind::DiscordBotToken, &discord_account, secret)
    {
        restore_secret(
            SecretKind::ImapPassword,
            &imap_account,
            prior_imap_secret.as_deref(),
            imap_password.is_some(),
        );
        return Err(format!("Could not save the Discord token: {error:#}"));
    }

    if let Err(error) = sync_autostart(&app, settings.launch_at_login) {
        restore_secret(
            SecretKind::ImapPassword,
            &imap_account,
            prior_imap_secret.as_deref(),
            imap_password.is_some(),
        );
        restore_secret(
            SecretKind::DiscordBotToken,
            &discord_account,
            prior_discord_secret.as_deref(),
            discord_token.is_some(),
        );
        return Err(format!("Could not update launch at login: {error}"));
    }

    if let Err(error) = settings.save(&state.settings_path) {
        let _ = sync_autostart(&app, previous.launch_at_login);
        restore_secret(
            SecretKind::ImapPassword,
            &imap_account,
            prior_imap_secret.as_deref(),
            imap_password.is_some(),
        );
        restore_secret(
            SecretKind::DiscordBotToken,
            &discord_account,
            prior_discord_secret.as_deref(),
            discord_token.is_some(),
        );
        return Err(format!("Could not save settings: {error:#}"));
    }

    *state
        .settings
        .write()
        .map_err(|_| "settings lock is unavailable".to_owned())? = settings.clone();
    restart_detectors(&state, &settings)?;

    if previous.email.username != settings.email.username
        && !previous.email.username.trim().is_empty()
    {
        let old_account = credential_account(SecretKind::ImapPassword, &previous.email.username);
        if let Err(error) = delete_secret(SecretKind::ImapPassword, &old_account) {
            eprintln!("Ion Sense could not remove the previous IMAP credential: {error:#}");
        }
    }

    Ok(settings)
}

#[tauri::command]
fn get_runtime_info(
    window: WebviewWindow,
    state: State<'_, NativeState>,
) -> Result<RuntimeInfo, String> {
    require_window(&window, "settings")?;
    Ok(RuntimeInfo {
        dev: tauri::is_dev(),
        platform: std::env::consts::OS,
        settings_path: state.settings_path.display().to_string(),
    })
}

#[tauri::command]
fn get_credential_status(
    window: WebviewWindow,
    state: State<'_, NativeState>,
) -> Result<CredentialStatus, String> {
    require_window(&window, "settings")?;
    let settings = state
        .settings
        .read()
        .map_err(|_| "settings lock is unavailable".to_owned())?;
    let imap_account = credential_account(SecretKind::ImapPassword, &settings.email.username);
    let discord_account = credential_account(SecretKind::DiscordBotToken, "discord-bot");
    Ok(CredentialStatus {
        imap_configured: !settings.email.username.trim().is_empty()
            && get_secret(SecretKind::ImapPassword, &imap_account).is_ok(),
        discord_configured: get_secret(SecretKind::DiscordBotToken, &discord_account).is_ok(),
    })
}

#[tauri::command]
async fn clear_imap_password(
    window: WebviewWindow,
    state: State<'_, NativeState>,
) -> Result<AppSettings, String> {
    require_window(&window, "settings")?;
    let mut settings = state
        .settings
        .read()
        .map_err(|_| "settings lock is unavailable".to_owned())?
        .clone();
    let account = credential_account(SecretKind::ImapPassword, &settings.email.username);
    settings.email.enabled = false;
    settings.save(&state.settings_path).map_err(|error| {
        format!("Could not disable email before clearing its password: {error:#}")
    })?;
    *state
        .settings
        .write()
        .map_err(|_| "settings lock is unavailable".to_owned())? = settings.clone();
    restart_detectors(&state, &settings)?;
    delete_secret(SecretKind::ImapPassword, &account).map_err(|error| {
        format!("Email was disabled, but its password could not be cleared: {error:#}")
    })?;
    Ok(settings)
}

#[tauri::command]
async fn clear_discord_token(
    window: WebviewWindow,
    state: State<'_, NativeState>,
) -> Result<AppSettings, String> {
    require_window(&window, "settings")?;
    let mut settings = state
        .settings
        .read()
        .map_err(|_| "settings lock is unavailable".to_owned())?
        .clone();
    let account = credential_account(SecretKind::DiscordBotToken, "discord-bot");
    settings.discord.enabled = false;
    settings.save(&state.settings_path).map_err(|error| {
        format!("Could not disable Discord before clearing its token: {error:#}")
    })?;
    *state
        .settings
        .write()
        .map_err(|_| "settings lock is unavailable".to_owned())? = settings.clone();
    restart_detectors(&state, &settings)?;
    delete_secret(SecretKind::DiscordBotToken, &account).map_err(|error| {
        format!("Discord was disabled, but its token could not be cleared: {error:#}")
    })?;
    Ok(settings)
}

#[tauri::command]
fn hud_ready(
    window: WebviewWindow,
    app: AppHandle,
    state: State<'_, NativeState>,
) -> Result<bool, String> {
    require_window(&window, "hud")?;
    state.hud.ready.store(true, Ordering::Release);
    state.hud.changed.notify_waiters();
    #[cfg(debug_assertions)]
    eprintln!("Ion Sense HUD bridge ready");
    if let Some(hud) = app.get_webview_window("hud") {
        hud.set_ignore_cursor_events(true)
            .map_err(|error| error.to_string())?;
    }
    Ok(tauri::is_dev())
}

#[tauri::command]
fn hud_idle(
    window: WebviewWindow,
    app: AppHandle,
    state: State<'_, NativeState>,
    timestamp: u64,
) -> Result<bool, String> {
    require_window(&window, "hud")?;
    if state.hud.delivered_timestamp.load(Ordering::Acquire) != timestamp {
        return Ok(false);
    }
    if let Some(hud) = app.get_webview_window("hud") {
        hud.set_ignore_cursor_events(true)
            .map_err(|error| error.to_string())?;
        hud.hide().map_err(|error| error.to_string())?;
    }
    state
        .hud
        .acknowledged_timestamp
        .store(timestamp, Ordering::Release);
    state.hud.changed.notify_waiters();
    #[cfg(debug_assertions)]
    eprintln!("Ion Sense HUD returned to idle");
    Ok(true)
}

#[cfg(debug_assertions)]
#[tauri::command]
async fn fire_test_event(
    window: WebviewWindow,
    event_type: String,
    state: State<'_, NativeState>,
) -> Result<(), String> {
    if !tauri::is_dev() {
        return Err("Test events are available only in tauri dev".into());
    }
    if window.label() != "hud" && window.label() != "settings" {
        return Err("This window cannot fire test events".into());
    }
    let event =
        mock_event(&event_type).ok_or_else(|| format!("Unknown event type: {event_type}"))?;
    state
        .dispatcher
        .dispatch(event)
        .await
        .map_err(|_| "central event dispatcher is closed".to_owned())
}

#[cfg(debug_assertions)]
fn mock_event(event_type: &str) -> Option<IonSenseEvent> {
    let (event_type, message, severity) = match event_type {
        "battery_low" => (
            IonSenseEventType::BatteryLow,
            "Battery at 12%. Connect a power source.",
            Severity::Warning,
        ),
        "overheating" => (
            IonSenseEventType::Overheating,
            "CPU package temperature reached 94°C.",
            Severity::Critical,
        ),
        "download_finished" => (
            IonSenseEventType::DownloadFinished,
            "All downloads completed successfully.",
            Severity::Info,
        ),
        "new_email" => (
            IonSenseEventType::NewEmail,
            "Maya Chen sent “Project Aurora — final notes”.",
            Severity::Info,
        ),
        "friend_message" => (
            IonSenseEventType::FriendMessage,
            "Nadia: “You around for a quick co-op run?”",
            Severity::Info,
        ),
        "package_delivered" => (
            IonSenseEventType::PackageDelivered,
            "Your parcel was left at the front door.",
            Severity::Info,
        ),
        _ => return None,
    };
    Some(IonSenseEvent::new(event_type, message, severity))
}

fn require_window(window: &WebviewWindow, expected: &str) -> Result<(), String> {
    if window.label() == expected {
        Ok(())
    } else {
        Err(format!(
            "The {} window cannot invoke this command",
            window.label()
        ))
    }
}

fn restart_detectors(state: &NativeState, settings: &AppSettings) -> Result<(), String> {
    let replacement = DetectorRuntime::start(settings, state.dispatcher.clone());
    let previous = state
        .detectors
        .lock()
        .map_err(|_| "detector lock is unavailable".to_owned())?
        .replace(replacement);
    drop(previous);
    Ok(())
}

fn restore_secret(kind: SecretKind, account: &str, previous: Option<&str>, changed: bool) {
    if !changed {
        return;
    }
    let result = match previous {
        Some(secret) => set_secret(kind, account, secret),
        None => delete_secret(kind, account),
    };
    if let Err(error) = result {
        eprintln!("Ion Sense could not roll back a credential change: {error:#}");
    }
}

fn setup_app(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);

    let settings_path = app.path().app_config_dir()?.join("settings.json");
    let settings = AppSettings::load_or_create(&settings_path)?;
    if let Err(error) = sync_autostart(app.handle(), settings.launch_at_login) {
        eprintln!("Ion Sense could not synchronize launch at login: {error:#}");
    }

    let (dispatcher, receiver) = EventDispatcher::channel(64);
    let hud = Arc::new(HudLifecycle::new());
    let detectors = DetectorRuntime::start(&settings, dispatcher.clone());

    app.manage(NativeState {
        settings: RwLock::new(settings),
        settings_path,
        detectors: Mutex::new(Some(detectors)),
        dispatcher,
        hud: hud.clone(),
    });
    #[cfg(debug_assertions)]
    eprintln!("Ion Sense native state ready");

    setup_tray(app)?;
    if let Some(hud) = app.get_webview_window("hud") {
        configure_hud_backdrop(&hud)?;
        hud.set_ignore_cursor_events(true)?;
    }

    let app_handle = app.handle().clone();
    tauri::async_runtime::spawn(forward_events(app_handle, receiver, hud));
    Ok(())
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn configure_hud_backdrop(hud: &WebviewWindow) -> tauri::Result<()> {
    hud.set_effects(
        EffectsBuilder::new()
            .effects([Effect::FullScreenUI, Effect::Blur])
            .state(EffectState::Active)
            .color(Color(2, 12, 5, 118))
            .build(),
    )
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn configure_hud_backdrop(_hud: &WebviewWindow) -> tauri::Result<()> {
    Ok(())
}

async fn forward_events(
    app: AppHandle,
    mut receiver: mpsc::Receiver<IonSenseEvent>,
    hud_state: Arc<HudLifecycle>,
) {
    while let Some(event) = receiver.recv().await {
        'delivery: loop {
            wait_for_hud(&hud_state).await;
            hud_state
                .delivered_timestamp
                .store(event.timestamp, Ordering::Release);

            let Some(hud) = app.get_webview_window("hud") else {
                eprintln!("Ion Sense HUD window is unavailable");
                break 'delivery;
            };
            if let Err(error) = position_hud(&hud)
                .and_then(|_| hud.show())
                .and_then(|_| app.emit_to("hud", EVENT_NAME, event.clone()))
            {
                eprintln!("Ion Sense could not deliver an event to the HUD: {error}");
                tokio::time::sleep(Duration::from_millis(250)).await;
                continue 'delivery;
            }

            #[cfg(debug_assertions)]
            eprintln!(
                "Ion Sense delivered {:?} through {EVENT_NAME}",
                event.event_type
            );

            loop {
                let changed = hud_state.changed.notified();
                if hud_state.acknowledged_timestamp.load(Ordering::Acquire) == event.timestamp {
                    break 'delivery;
                }
                if !hud_state.ready.load(Ordering::Acquire) {
                    continue 'delivery;
                }
                let _ = tokio::time::timeout(Duration::from_secs(1), changed).await;
            }
        }
    }
}

async fn wait_for_hud(hud: &HudLifecycle) {
    loop {
        let changed = hud.changed.notified();
        if hud.ready.load(Ordering::Acquire) {
            return;
        }
        let _ = tokio::time::timeout(Duration::from_secs(1), changed).await;
    }
}

fn position_hud(hud: &WebviewWindow) -> tauri::Result<()> {
    let Some(monitor) = hud.primary_monitor()? else {
        return Ok(());
    };
    let monitor_position = monitor.position();
    let monitor_size = monitor.size();
    hud.set_position(Position::Physical(PhysicalPosition::new(
        monitor_position.x,
        monitor_position.y,
    )))?;
    hud.set_size(Size::Physical(PhysicalSize::new(
        monitor_size.width,
        monitor_size.height,
    )))
}

fn setup_tray(app: &App) -> tauri::Result<()> {
    let settings_item = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit Ion Sense", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&settings_item, &separator, &quit_item])?;

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Ion Sense")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "settings" => {
                if let Err(error) = open_settings(app) {
                    eprintln!("Ion Sense could not open settings: {error}");
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) && let Err(error) = open_settings(tray.app_handle())
            {
                eprintln!("Ion Sense could not open settings: {error}");
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

fn open_settings(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("settings") {
        window.show()?;
        window.unminimize()?;
        window.set_focus()?;
    }
    Ok(())
}

fn sync_autostart(app: &AppHandle, should_enable: bool) -> anyhow::Result<()> {
    let autostart = app.autolaunch();
    let enabled = autostart.is_enabled().context("read autostart state")?;
    if should_enable && !enabled {
        autostart.enable().context("enable autostart")?;
    } else if !should_enable && enabled {
        autostart.disable().context("disable autostart")?;
    }
    Ok(())
}

pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .arg("--autostart")
                .build(),
        )
        .setup(setup_app)
        .on_page_load(|webview, payload| {
            if webview.label() == "hud"
                && let Some(state) = webview.try_state::<NativeState>()
            {
                state.hud.ready.store(false, Ordering::Release);
                state.hud.changed.notify_waiters();
            }
            #[cfg(debug_assertions)]
            eprintln!("Ion Sense loaded {} in {}", payload.url(), webview.label());
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        });

    #[cfg(debug_assertions)]
    let builder = builder.invoke_handler(tauri::generate_handler![
        get_settings,
        save_settings,
        get_runtime_info,
        get_credential_status,
        clear_imap_password,
        clear_discord_token,
        hud_ready,
        hud_idle,
        fire_test_event
    ]);

    #[cfg(not(debug_assertions))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        get_settings,
        save_settings,
        get_runtime_info,
        get_credential_status,
        clear_imap_password,
        clear_discord_token,
        hud_ready,
        hud_idle
    ]);

    builder
        .run(tauri::generate_context!())
        .expect("failed to run Ion Sense");
}
