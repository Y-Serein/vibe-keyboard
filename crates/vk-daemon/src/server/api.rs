//! Axum HTTP API — router + all handler functions.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use tower_http::cors::CorsLayer;
use tracing::{info, warn};

use vk_protocol::message::{
    ButtonId, DownlinkMessage, LedColor, PermissionAction, SessionStatus, UplinkMessage,
};
use tokio::sync::oneshot;

use crate::cesp;
use crate::permission::{evaluate_yolo, YoloDecision};
use crate::session::monitor::{parse_hook_event, HookEvent};
use crate::setup::SetupManager;

use super::ipc_handler::{handle_uplink, process_session_event, send_downlink};
use super::scanner::resolve_tmux_client_tty;
use super::state::{DaemonState, PermissionDecision};

// ── HTTP Hook Server ──

pub(super) async fn run_hook_server(
    state: Arc<DaemonState>,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new()
        .route("/event", post(handle_hook_event))
        .route("/health", get(handle_health))
        .route("/sessions", get(handle_sessions))
        .route("/button", post(handle_button_press))
        .route("/button/state", get(handle_button_state))
        .route("/log", get(handle_get_log))
        .route("/yolo", get(handle_get_yolo))
        .route("/knob", post(handle_knob_action))
        .route("/config", get(handle_get_config))
        .route("/config", post(handle_set_config))
        .route("/frame", get(handle_get_frame))
        .route("/notify/test", post(handle_notify_test))
        .route("/setup/status", get(handle_setup_status))
        .route("/setup/install/{tool_id}", post(handle_setup_install))
        .route("/setup/uninstall/{tool_id}", post(handle_setup_uninstall))
        .route("/setup/brew-install/{package}", post(handle_brew_install))
        .route("/setup/brew-uninstall/{package}", post(handle_brew_uninstall))
        .route("/sounds", get(handle_list_sounds))
        .route("/sounds/upload", post(handle_upload_sound))
        .route("/sounds/play", post(handle_play_sound))
        .layer(CorsLayer::new()) // default denies all origins
        .with_state(state);

    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("hook server listening on {addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_health() -> StatusCode {
    StatusCode::OK
}

async fn handle_sessions(
    State(state): State<Arc<DaemonState>>,
) -> Json<Vec<vk_core::SessionInfo>> {
    let store = state.store.read().await;
    let sessions: Vec<vk_core::SessionInfo> = store.list().iter().map(|s| s.info.clone()).collect();
    Json(sessions)
}

async fn handle_hook_event(
    State(state): State<Arc<DaemonState>>,
    Json(hook): Json<HookEvent>,
) -> (StatusCode, Json<serde_json::Value>) {
    info!("hook event: type={} session_id={} tty={}", hook.event_type, hook.session_id, hook.session_tty);

    let is_permission = hook.event_type == "permission" || hook.event_type == "permission_request"
        || hook.event_type == "PreToolUse"; // Claude Code PreToolUse IS a permission check

    // Always update session's TTY/bundle_id from hook data (every hook carries these)
    if !hook.session_id.is_empty() && (!hook.session_tty.is_empty() || !hook.bundle_id.is_empty()) {
        let map = state.session_id_map.read().await;
        if let Some(&numeric_id) = map.get(&hook.session_id) {
            let mut store = state.store.write().await;
            if let Some(s) = store.get_mut(numeric_id) {
                if !hook.session_tty.is_empty() {
                    // Resolve tmux: if this TTY is a tmux pane, get the real client TTY
                    let real_tty = resolve_tmux_client_tty(&hook.session_tty);
                    if s.info.session_tty.is_empty() || s.info.session_tty != real_tty {
                        s.info.session_tty = real_tty.clone();
                        info!("hook updated session #{} tty={}", numeric_id, real_tty);
                    }
                }
                if !hook.bundle_id.is_empty() && s.info.bundle_id.is_empty() {
                    s.info.bundle_id = hook.bundle_id.clone();
                }
            }
        }
    }

    let event = match parse_hook_event(&hook) {
        Some(e) => e,
        None => {
            warn!("unrecognised hook event type: {}", hook.event_type);
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "unknown event type"})));
        }
    };

    if is_permission {
        // For permission requests, check YOLO/always-allow first, then block if needed.
        let numeric_id = {
            let map = state.session_id_map.read().await;
            map.get(&hook.session_id).copied()
        };

        // Pre-check: always-allow
        {
            let pq = state.perm_queue.read().await;
            if pq.is_always_allowed(&hook.tool_name, &hook.tool_input) {
                info!("always-allow (HTTP): {}({})", hook.tool_name, hook.tool_input);
                let body = serde_json::json!({"hookSpecificOutput":{"decision":{"behavior":"allow"}}});
                return (StatusCode::OK, Json(body));
            }
        }

        // Pre-check: YOLO (global — applies to ALL sessions regardless of numeric_id)
        {
            let yolo = state.yolo.read().await;
            let yolo_decision = evaluate_yolo(&yolo, &hook.tool_name, &hook.tool_input);
            drop(yolo);

            match yolo_decision {
                YoloDecision::AutoAllow => {
                    info!("YOLO auto-allow: {}({})", hook.tool_name, hook.tool_input);
                    let body = serde_json::json!({"hookSpecificOutput":{"decision":{"behavior":"allow"}}});
                    log_activity(&state, format!("YOLO ✓ allow {}({})", hook.tool_name, hook.tool_input)).await;
                    return (StatusCode::OK, Json(body));
                }
                YoloDecision::AutoDeny => {
                    info!("YOLO auto-deny: {}({})", hook.tool_name, hook.tool_input);
                    let body = serde_json::json!({"hookSpecificOutput":{"decision":{"behavior":"deny"}}});
                    log_activity(&state, format!("YOLO ✗ deny {}({})", hook.tool_name, hook.tool_input)).await;
                    return (StatusCode::OK, Json(body));
                }
                YoloDecision::AskUser => {
                    // Fall through to blocking permission flow
                }
            }
        }

        // Process the event (queues permission, sends to simulator)
        process_session_event(&state, event).await;

        if let Some(numeric_id) = numeric_id {
            // Permission should now be pending — wait for user decision
            let is_pending = {
                let pq = state.perm_queue.read().await;
                pq.pending_list().iter().any(|p| p.session_id == numeric_id)
            };

            if is_pending {
                // Create oneshot channel and wait
                let (tx, rx) = oneshot::channel();
                {
                    let mut channels = state.perm_response_channels.write().await;
                    channels.insert(numeric_id, tx);
                }
                info!("blocking HTTP response for permission #{numeric_id}...");

                // Wait for decision (with timeout)
                match tokio::time::timeout(std::time::Duration::from_secs(300), rx).await {
                    Ok(Ok(decision)) => {
                        let behavior = match decision.action {
                            PermissionAction::Allow | PermissionAction::Always => "allow",
                            PermissionAction::Deny => "deny",
                        };
                        info!("permission #{numeric_id} resolved: {behavior}");
                        let body = serde_json::json!({
                            "hookSpecificOutput": {
                                "decision": {
                                    "behavior": behavior
                                }
                            }
                        });
                        return (StatusCode::OK, Json(body));
                    }
                    Ok(Err(_)) => {
                        warn!("permission channel dropped for #{numeric_id}");
                    }
                    Err(_) => {
                        warn!("permission timeout for #{numeric_id}");
                    }
                }
                // Cleanup on timeout/error
                let mut channels = state.perm_response_channels.write().await;
                channels.remove(&numeric_id);
            }
            // Fail-closed: timeout/error defaults to deny (security M14-T14.5)
            let body = serde_json::json!({
                "hookSpecificOutput": {
                    "decision": {
                        "behavior": "deny"
                    }
                }
            });
            return (StatusCode::OK, Json(body));
        }
    } else {
        process_session_event(&state, event).await;
    }

    (StatusCode::OK, Json(serde_json::json!({})))
}

/// GET /frame — returns raw RGB565 framebuffer bytes.
async fn handle_get_frame(
    State(state): State<Arc<DaemonState>>,
) -> axum::response::Response {
    // T16.5: bytes::Bytes clone is Arc-level (no data copy)
    let frame = state.frame_buffer.read().await.clone();
    axum::response::Response::builder()
        .header("Content-Type", "application/octet-stream")
        .header("X-LCD-Width", state.lcd_width.to_string())
        .header("X-LCD-Height", state.lcd_height.to_string())
        .body(axum::body::Body::from(frame))
        .unwrap()
}

// ── GUI API endpoints ──

#[derive(serde::Deserialize)]
struct ButtonRequest {
    id: String,
    /// "down" = key press, "up" = key release, "toggle" = toggle, "click" = full press+release
    #[serde(default)]
    action: Option<String>,
}

/// Append to activity log (ring buffer, max 50).
pub(super) async fn log_activity(state: &DaemonState, msg: String) {
    let mut log = state.activity_log.write().await;
    let ts = chrono::Local::now().format("%H:%M:%S").to_string();
    log.push(format!("[{ts}] {msg}"));
    if log.len() > 50 { log.remove(0); }
}

/// GET /log — returns recent activity log.
async fn handle_get_log(
    State(state): State<Arc<DaemonState>>,
) -> Json<Vec<String>> {
    let log = state.activity_log.read().await;
    Json(log.clone())
}

/// GET /yolo — returns current YOLO mode state.
async fn handle_get_yolo(
    State(state): State<Arc<DaemonState>>,
) -> Json<serde_json::Value> {
    let yolo = state.yolo.read().await;
    Json(serde_json::json!({
        "active": yolo.active,
        "allow": yolo.allow,
        "deny": yolo.deny,
    }))
}

/// GET /button/state — returns which keys are currently held.
async fn handle_button_state(
    State(state): State<Arc<DaemonState>>,
) -> Json<serde_json::Value> {
    let held = state.held_keys.read().await;
    let keys: Vec<&String> = held.iter().collect();
    Json(serde_json::json!({ "held": keys }))
}

async fn handle_button_press(
    State(state): State<Arc<DaemonState>>,
    Json(req): Json<ButtonRequest>,
) -> StatusCode {
    let button = match req.id.as_str() {
        "send" => ButtonId::Send,
        "cancel" => ButtonId::Cancel,
        "mode" => ButtonId::Mode,
        "session" => ButtonId::Session,
        "delete" => ButtonId::Delete,
        "voice" => ButtonId::Voice,
        _ => return StatusCode::BAD_REQUEST,
    };
    let action = req.action.as_deref().unwrap_or("click");
    info!("GUI button {action}: {button:?}");
    log_activity(&state, format!("{:?} {} → macro={}", button, action,
        get_macro_for_button(&state, &button).await)).await;

    match action {
        "down" => {
            let macro_action = get_macro_for_button(&state, &button).await;
            if !macro_action.is_empty() {
                let action_clone = macro_action.clone();
                let result = tokio::task::spawn_blocking(move || {
                    crate::keystroke::execute_key_down(&action_clone)
                }).await;
                match result {
                    Ok(Ok(())) => log_activity(&state, format!("  ✓ key-down '{macro_action}' ok")).await,
                    Ok(Err(e)) => log_activity(&state, format!("  ✗ key-down '{macro_action}' FAILED: {e}")).await,
                    Err(e) => log_activity(&state, format!("  ✗ spawn error: {e}")).await,
                }
                state.held_keys.write().await.insert(format!("{}:{}", req.id, macro_action));
            }
        }
        "up" => {
            let macro_action = get_macro_for_button(&state, &button).await;
            if !macro_action.is_empty() {
                let action_clone = macro_action.clone();
                let result = tokio::task::spawn_blocking(move || {
                    crate::keystroke::execute_key_up(&action_clone)
                }).await;
                match result {
                    Ok(Ok(())) => log_activity(&state, format!("  ✓ key-up '{macro_action}' ok")).await,
                    Ok(Err(e)) => log_activity(&state, format!("  ✗ key-up '{macro_action}' FAILED: {e}")).await,
                    Err(e) => log_activity(&state, format!("  ✗ spawn error: {e}")).await,
                }
                state.held_keys.write().await.remove(&format!("{}:{}", req.id, macro_action));
            }
        }
        "toggle" => {
            // Toggle mode: click to toggle
            let macro_action = get_macro_for_button(&state, &button).await;
            if !macro_action.is_empty() {
                super::ipc_handler::handle_uplink(&state, UplinkMessage::ButtonPress(button)).await;
            }
        }
        _ => {
            // Default click: send full press+release
            let macro_action = get_macro_for_button(&state, &button).await;
            if !macro_action.is_empty() {
                let action_clone = macro_action.clone();
                let result = tokio::task::spawn_blocking(move || {
                    crate::keystroke::execute_button_action(&action_clone)
                }).await;
                match result {
                    Ok(Ok(())) => log_activity(&state, format!("  ✓ click '{macro_action}' ok")).await,
                    Ok(Err(e)) => log_activity(&state, format!("  ✗ click '{macro_action}' FAILED: {e}")).await,
                    Err(e) => log_activity(&state, format!("  ✗ spawn error: {e}")).await,
                }
            } else {
                // Fixed buttons (send/cancel/mode/session) go through uplink
                handle_uplink(&state, UplinkMessage::ButtonPress(button)).await;
            }
        }
    }
    // T16.1: bump render generation on button interaction
    state.bump_render_generation();
    StatusCode::OK
}

/// Get the macro action string for a button (from config).
async fn get_macro_for_button(state: &DaemonState, button: &ButtonId) -> String {
    let macros = state.macro_config.read().await;
    match button {
        ButtonId::Delete => macros.delete.clone(),
        ButtonId::Voice => macros.voice.clone(),
        _ => String::new(),
    }
}

/// Focus the active session's terminal (helper for buttons).
#[allow(dead_code)]
async fn focus_active_session(state: &DaemonState) {
    let ui = state.ui_state.read().await;
    let sessions = ui.sessions();
    let idx = ui.active_index();
    if let Some(session) = sessions.get(idx) {
        let session_id = session.id;
        drop(ui);
        let store = state.store.read().await;
        if let Some(s) = store.get(session_id) {
            let _ = crate::focus::macos::activate_window(s);
        }
    }
}

#[derive(serde::Deserialize)]
struct KnobRequest {
    action: String,
    #[serde(default = "default_steps")]
    steps: u8,
}

fn default_steps() -> u8 {
    1
}

async fn handle_knob_action(
    State(state): State<Arc<DaemonState>>,
    Json(req): Json<KnobRequest>,
) -> StatusCode {
    match req.action.as_str() {
        "cw" => {
            info!("GUI knob CW {}", req.steps);
            handle_uplink(
                &state,
                UplinkMessage::KnobRotate {
                    direction: vk_protocol::message::Direction::Clockwise,
                    steps: req.steps,
                },
            )
            .await;
        }
        "ccw" => {
            info!("GUI knob CCW {}", req.steps);
            handle_uplink(
                &state,
                UplinkMessage::KnobRotate {
                    direction: vk_protocol::message::Direction::CounterClockwise,
                    steps: req.steps,
                },
            )
            .await;
        }
        "press" => {
            info!("GUI knob press");
            handle_uplink(&state, UplinkMessage::KnobPress).await;
        }
        _ => return StatusCode::BAD_REQUEST,
    }
    StatusCode::OK
}

async fn handle_get_config(
    State(_state): State<Arc<DaemonState>>,
) -> Json<serde_json::Value> {
    let cfg = crate::config::load_config(&crate::config::default_config_path());
    Json(serde_json::json!({
        "general": {
            "hook_port": cfg.general.hook_port,
            "log_level": cfg.general.log_level,
        },
        "yolo": {
            "active": cfg.yolo.active,
            "allow": cfg.yolo.allow,
            "deny": cfg.yolo.deny,
            "notify_auto_allow": cfg.yolo.notify_auto_allow,
            "auto_allow_log": cfg.yolo.auto_allow_log,
        },
        "ipc": {
            "socket_path": cfg.ipc.socket_path,
        },
        "macros": {
            "delete": cfg.macros.delete,
            "voice": cfg.macros.voice,
        },
        "display": {
            "width": cfg.display.width,
            "height": cfg.display.height,
        },
        "sound": {
            "enabled": cfg.sound.enabled,
            "volume": cfg.sound.volume,
            "muted": cfg.sound.muted,
            "mapping": {
                "permission_alert": cfg.sound.mapping.permission_alert,
                "session_complete": cfg.sound.mapping.session_complete,
                "error": cfg.sound.mapping.error,
                "click": cfg.sound.mapping.click,
            }
        }
    }))
}

#[derive(serde::Deserialize)]
struct ConfigUpdate {
    key: String,
    value: String,
}

async fn handle_set_config(
    State(state): State<Arc<DaemonState>>,
    Json(req): Json<ConfigUpdate>,
) -> (StatusCode, String) {
    let path = crate::config::default_config_path();
    let mut cfg = crate::config::load_config(&path);

    match req.key.as_str() {
        "yolo.active" => match req.value.parse::<bool>() {
            Ok(v) => {
                cfg.yolo.active = v;
                // Also update runtime YOLO state
                let mut yolo = state.yolo.write().await;
                yolo.active = v;
            }
            Err(e) => return (StatusCode::BAD_REQUEST, format!("invalid bool: {e}")),
        },
        "yolo.allow" => {
            cfg.yolo.allow = req.value.split(',').map(|s| s.trim().to_string()).collect();
            let mut yolo = state.yolo.write().await;
            yolo.allow = cfg.yolo.allow.clone();
        }
        "yolo.deny" => {
            cfg.yolo.deny = req.value.split(',').map(|s| s.trim().to_string()).collect();
            let mut yolo = state.yolo.write().await;
            yolo.deny = cfg.yolo.deny.clone();
        }
        "general.hook_port" => match req.value.parse::<u16>() {
            Ok(v) => cfg.general.hook_port = v,
            Err(e) => return (StatusCode::BAD_REQUEST, format!("invalid port: {e}")),
        },
        "yolo.notify_auto_allow" => match req.value.parse::<bool>() {
            Ok(v) => cfg.yolo.notify_auto_allow = v,
            Err(e) => return (StatusCode::BAD_REQUEST, format!("invalid bool: {e}")),
        },
        "macros.delete" => {
            cfg.macros.delete = req.value.clone();
            let mut mc = state.macro_config.write().await;
            mc.delete = req.value.clone();
        }
        "macros.voice" => {
            cfg.macros.voice = req.value.clone();
            let mut mc = state.macro_config.write().await;
            mc.voice = req.value.clone();
        }
        "display.width" => match req.value.parse::<u16>() {
            Ok(v) => cfg.display.width = v,
            Err(e) => return (StatusCode::BAD_REQUEST, format!("invalid width: {e}")),
        },
        "display.height" => match req.value.parse::<u16>() {
            Ok(v) => cfg.display.height = v,
            Err(e) => return (StatusCode::BAD_REQUEST, format!("invalid height: {e}")),
        },
        "sound.enabled" => match req.value.parse::<bool>() {
            Ok(v) => cfg.sound.enabled = v,
            Err(e) => return (StatusCode::BAD_REQUEST, format!("invalid bool: {e}")),
        },
        "sound.volume" => match req.value.parse::<u8>() {
            Ok(v) => {
                let vol = v.min(100);
                cfg.sound.volume = vol;
                // Propagate to connected simulator
                send_downlink(&state, DownlinkMessage::SetVolume(vol)).await;
            }
            Err(e) => return (StatusCode::BAD_REQUEST, format!("invalid volume: {e}")),
        },
        "sound.muted" => match req.value.parse::<bool>() {
            Ok(v) => {
                cfg.sound.muted = v;
                // Propagate to connected simulator
                send_downlink(&state, DownlinkMessage::SetMuted(v)).await;
            }
            Err(e) => return (StatusCode::BAD_REQUEST, format!("invalid bool: {e}")),
        },
        "sound.mapping.permission_alert" => cfg.sound.mapping.permission_alert = req.value.clone(),
        "sound.mapping.session_complete" => cfg.sound.mapping.session_complete = req.value.clone(),
        "sound.mapping.error" => cfg.sound.mapping.error = req.value.clone(),
        "sound.mapping.click" => cfg.sound.mapping.click = req.value.clone(),
        _ => return (StatusCode::BAD_REQUEST, format!("unknown key: {}", req.key)),
    }

    match crate::config::save_config(&path, &cfg) {
        Ok(()) => {
            info!("config updated: {}={}", req.key, req.value);
            (StatusCode::OK, format!("{}={} saved", req.key, req.value))
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("save failed: {e}")),
    }
}

/// POST /notify/test — sequential test of ALL sound/LED/notification events.
///
/// Returns immediately (HTTP 200), plays 5 events in background with 1.5s gaps.
async fn handle_notify_test(
    State(state): State<Arc<DaemonState>>,
) -> StatusCode {
    // Spawn the sequence to background — don't block the HTTP response.
    tokio::spawn(async move {
        run_notify_test_sequence(state).await;
    });
    StatusCode::OK
}

async fn run_notify_test_sequence(state: Arc<DaemonState>) {
    use vk_protocol::message::{SessionStatus, SoundType};
    let s = &*state; // Deref Arc for function calls

    let delay = std::time::Duration::from_millis(1500);

    // 1. Click — button feedback sound
    info!("notify/test [1/5]: Click");
    send_downlink(s, DownlinkMessage::PlaySound(SoundType::Click)).await;
    tokio::time::sleep(delay).await;

    // 2. Permission Alert — permission request arrived
    info!("notify/test [2/5]: PermissionAlert");
    send_downlink(s, DownlinkMessage::PlaySound(SoundType::PermissionAlert)).await;
    send_downlink(s, DownlinkMessage::SetKnobRing(LedColor::GREEN)).await;
    cesp::route_status_change(
        &s.notification_queue,
        s,
        101,
        "PermTest",
        SessionStatus::Thinking,
        SessionStatus::PermissionNeeded,
        "Bash(rm -rf /tmp/test) needs approval",
    )
    .await;
    tokio::time::sleep(delay).await;

    // 3. Session Complete (Done) — ding + knob blue
    info!("notify/test [3/5]: SessionComplete (Done)");
    cesp::route_status_change(
        &s.notification_queue,
        s,
        102,
        "MyProject",
        SessionStatus::Thinking,
        SessionStatus::Done,
        "All tasks completed successfully",
    )
    .await;
    tokio::time::sleep(delay).await;

    // 4. Error — buzz + session LED red blink
    info!("notify/test [4/5]: Error");
    cesp::route_status_change(
        &s.notification_queue,
        s,
        103,
        "BuggyProject",
        SessionStatus::ToolUse,
        SessionStatus::Error,
        "cargo test failed: 3 assertions",
    )
    .await;
    tokio::time::sleep(delay).await;

    // 5. Context Limit — buzz + knob amber
    info!("notify/test [5/5]: ContextLimit >90%");
    cesp::route_context_limit(
        &s.notification_queue,
        s,
        104,
        "LargeSession",
        95,
    )
    .await;
    tokio::time::sleep(delay).await;

    // 6. Desktop notification summary
    let _ = s.notification_backend.notify(
        "Vibe Keyboard — Test Complete",
        "5 events tested: Click, PermissionAlert, Done, Error, ContextLimit",
        None,
    );

    let nq = s.notification_queue.read().await;
    info!(
        "notify/test: complete — 5 events played, {} notifications in queue",
        nq.all().len()
    );
}

// ── Setup API Handlers ──

async fn handle_setup_status(
    State(state): State<Arc<DaemonState>>,
) -> Json<crate::setup::SetupStatus> {
    let device_connected = state.ipc_downlink_tx.read().await.is_some();
    let status = SetupManager::detect_all(state.hook_port, device_connected).await;
    Json(status)
}

async fn handle_setup_install(
    State(state): State<Arc<DaemonState>>,
    axum::extract::Path(tool_id): axum::extract::Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    match SetupManager::install_hook(&tool_id, state.hook_port).await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "ok", "message": format!("Hook installed for {tool_id}")})),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"status": "error", "message": e})),
        ),
    }
}

async fn handle_setup_uninstall(
    axum::extract::Path(tool_id): axum::extract::Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    match SetupManager::uninstall_hook(&tool_id).await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "ok", "message": format!("Hook uninstalled for {tool_id}")})),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"status": "error", "message": e})),
        ),
    }
}

async fn handle_brew_install(
    axum::extract::Path(package): axum::extract::Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    match SetupManager::brew_install(&package).await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "ok", "message": format!("Installed {package}")})),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"status": "error", "message": e})),
        ),
    }
}

async fn handle_brew_uninstall(
    axum::extract::Path(package): axum::extract::Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    match SetupManager::brew_uninstall(&package).await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "ok", "message": format!("Uninstalled {package}")})),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"status": "error", "message": e})),
        ),
    }
}

// ── Sound endpoints (T11.6) ─────────────────────────────────────────

/// Directory for custom user sounds.
fn custom_sounds_dir() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("vk-daemon")
        .join("sounds")
        .join("custom")
}

/// POST /sounds/play — play a sound by sound_id ("builtin:alert") or event key ("error").
async fn handle_play_sound(
    State(state): State<Arc<DaemonState>>,
    Json(req): Json<serde_json::Value>,
) -> (StatusCode, String) {
    let sound_type_str = req.get("sound_type").and_then(|v| v.as_str()).unwrap_or("");

    // Try as sound_id first (e.g. "builtin:alert", "builtin:ding")
    if sound_type_str.starts_with("builtin:") || sound_type_str.starts_with("custom:") {
        state.local_speaker.play_by_id(sound_type_str);
        return (StatusCode::OK, "playing".into());
    }

    // Fallback: legacy event key → SoundType enum → send_downlink
    let sound_type = match sound_type_str {
        "permission_alert" => vk_protocol::message::SoundType::PermissionAlert,
        "session_complete" => vk_protocol::message::SoundType::SessionComplete,
        "error" => vk_protocol::message::SoundType::Error,
        "click" => vk_protocol::message::SoundType::Click,
        other => return (StatusCode::BAD_REQUEST, format!("unknown sound: {other}")),
    };

    send_downlink(&state, DownlinkMessage::PlaySound(sound_type)).await;
    (StatusCode::OK, "playing".into())
}

/// GET /sounds — list builtin + custom sounds.
async fn handle_list_sounds() -> Json<serde_json::Value> {
    let builtin = vec!["alert", "ding", "buzz", "click", "none"];

    let custom_dir = custom_sounds_dir();
    let custom: Vec<String> = tokio::task::spawn_blocking(move || {
        let mut names = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&custom_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("wav") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        names.push(stem.to_string());
                    }
                }
            }
        }
        names.sort();
        names
    })
    .await
    .unwrap_or_default();

    Json(serde_json::json!({
        "builtin": builtin,
        "custom": custom,
    }))
}

/// POST /sounds/upload — upload a custom WAV file (multipart form, field name "file").
async fn handle_upload_sound(
    mut multipart: axum::extract::Multipart,
) -> (StatusCode, Json<serde_json::Value>) {
    const MAX_SIZE: usize = 500 * 1024; // 500 KB
    const RIFF_HEADER: &[u8; 4] = b"RIFF";

    // Extract the "file" field from multipart
    let field = match multipart.next_field().await {
        Ok(Some(f)) => f,
        Ok(None) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"status": "error", "message": "no file field in multipart body"})),
            );
        }
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"status": "error", "message": format!("multipart error: {e}")})),
            );
        }
    };

    let filename = field
        .file_name()
        .unwrap_or("upload.wav")
        .to_string();

    // Validate extension
    if !filename.ends_with(".wav") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"status": "error", "message": "file must end with .wav"})),
        );
    }

    // Sanitize filename: only allow alphanumeric, dash, underscore, dot
    let safe_name: String = filename
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
        .collect();

    // Read bytes
    let data = match field.bytes().await {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"status": "error", "message": format!("failed to read upload: {e}")})),
            );
        }
    };

    // Validate size
    if data.len() > MAX_SIZE {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"status": "error", "message": format!("file too large: {} bytes (max {})", data.len(), MAX_SIZE)})),
        );
    }

    // Validate RIFF header
    if data.len() < 4 || &data[..4] != RIFF_HEADER {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"status": "error", "message": "invalid WAV file: missing RIFF header"})),
        );
    }

    let dest_dir = custom_sounds_dir();
    let dest_path = dest_dir.join(&safe_name);

    match tokio::task::spawn_blocking(move || -> Result<(), String> {
        std::fs::create_dir_all(&dest_dir).map_err(|e| format!("mkdir failed: {e}"))?;
        std::fs::write(&dest_path, &data).map_err(|e| format!("write failed: {e}"))?;
        Ok(())
    })
    .await
    {
        Ok(Ok(())) => {
            info!("uploaded custom sound: {safe_name}");
            (
                StatusCode::OK,
                Json(serde_json::json!({"status": "ok", "filename": safe_name})),
            )
        }
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"status": "error", "message": e})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"status": "error", "message": format!("task join error: {e}")})),
        ),
    }
}
