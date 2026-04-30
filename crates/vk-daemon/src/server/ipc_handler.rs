//! IPC handler — uplink processing, downlink sending, permission resolution.

use std::sync::Arc;

use tracing::{error, info, warn};

use vk_protocol::message::{
    ButtonId, DownlinkMessage, LedColor, PermissionAction, SessionInfo, SessionStatus, UplinkMessage,
};
use vk_transport::{IpcTransport, Transport};

use crate::cesp;
use crate::config;
use crate::focus;
use crate::permission::{
    evaluate_yolo, PendingPermission, YoloDecision,
};
use crate::session::monitor::SessionEvent;
use crate::session::store::DaemonSession;
use crate::transcript;

use super::state::{DaemonState, PermissionDecision};

/// IPC listener — accepts simulator connections and handles uplink messages.
pub(super) async fn run_ipc_listener(
    state: Arc<DaemonState>,
    socket_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        info!("IPC: waiting for simulator connection on {socket_path}");
        let transport = match IpcTransport::listen(socket_path).await {
            Ok(t) => Arc::new(t),
            Err(e) => {
                error!("IPC listen error: {e:?}");
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                continue;
            }
        };
        info!("IPC: simulator connected");

        // Store the transport for downlink sending
        {
            let mut tx = state.ipc_downlink_tx.write().await;
            *tx = Some(Arc::clone(&transport) as Arc<dyn Transport>);
        }

        // Backfill: send current session list + pending permissions
        {
            let store = state.store.read().await;
            let sessions = store.to_protocol_list();
            let msg = DownlinkMessage::SessionListUpdate {
                sessions,
                active_index: 0,
            };
            if let Err(e) = transport.send_downlink(&msg).await {
                warn!("IPC backfill send failed: {e:?}");
            }
            drop(store);

            // Resend pending permissions
            let pq = state.perm_queue.read().await;
            for perm in pq.pending_list() {
                let desc = format!("{}({})", perm.tool_name, perm.tool_input);
                let msg = DownlinkMessage::PermissionRequest {
                    session_id: perm.session_id,
                    action_desc: desc,
                };
                if let Err(e) = transport.send_downlink(&msg).await {
                    warn!("IPC backfill permission send failed: {e:?}");
                    break;
                }
            }
        }

        // Read uplink messages until disconnected
        loop {
            match transport.recv_uplink().await {
                Ok(msg) => {
                    handle_uplink(&state, msg).await;
                }
                Err(e) => {
                    warn!("IPC recv error (simulator disconnected?): {e:?}");
                    break;
                }
            }
        }

        // Clear transport on disconnect
        {
            let mut tx = state.ipc_downlink_tx.write().await;
            *tx = None;
        }
        info!("IPC: simulator disconnected, waiting for reconnect...");
    }
}

/// Send a downlink message to the connected simulator (if any).
/// For PlaySound/SetVolume/SetMuted: also plays locally on desktop via LocalSpeaker.
pub(super) async fn send_downlink(state: &DaemonState, msg: DownlinkMessage) {
    // Local desktop sound playback — always play, regardless of device connection.
    match &msg {
        DownlinkMessage::PlaySound(sound) => state.local_speaker.play(*sound),
        DownlinkMessage::SetVolume(vol) => state.local_speaker.set_volume(*vol),
        DownlinkMessage::SetMuted(muted) => state.local_speaker.set_muted(*muted),
        _ => {}
    }

    // Forward to device via IPC (if connected).
    let tx = state.ipc_downlink_tx.read().await;
    if let Some(ref transport) = *tx {
        if let Err(e) = transport.send_downlink(&msg).await {
            warn!("IPC send_downlink failed: {e:?}");
        }
    }
}

/// Implement DownlinkSender for DaemonState so cesp module can send downlink messages.
impl cesp::DownlinkSender for DaemonState {
    async fn send_downlink_if_connected(&self, msg: &DownlinkMessage) {
        send_downlink(self, msg.clone()).await;
    }
}

/// Map a SessionEvent into store updates + downlink messages.
pub(super) async fn process_session_event(state: &DaemonState, event: SessionEvent) {
    match event {
        SessionEvent::Started { session_id, name, source, cwd, bundle_id, session_tty, transcript_path } => {
            let mut store = state.store.write().await;
            let mut map = state.session_id_map.write().await;

            let numeric_id = store.allocate_id();
            map.insert(session_id.clone(), numeric_id);

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let session = DaemonSession {
                info: SessionInfo {
                    id: numeric_id,
                    name: name.clone(),
                    status: SessionStatus::Idle,
                    source,
                    cwd,
                    bundle_id,
                    session_tty,
                    started_at: now,
                    last_activity: now,
                    ..Default::default()
                },
                ..Default::default()
            };
            store.update(session);
            drop(store);
            drop(map);
            info!("session started: #{numeric_id} ({name}) hook_id={session_id}");

            // Register transcript scanner if path provided or discoverable
            let scan_path = if !transcript_path.is_empty() {
                if transcript::validate_transcript_path(&transcript_path) {
                    Some(std::path::PathBuf::from(&transcript_path))
                } else {
                    tracing::warn!("rejected unsafe transcript path: {}", transcript_path);
                    None
                }
            } else {
                transcript::find_transcript_path(&session_id)
            };
            if let Some(path) = scan_path {
                info!("transcript scanner registered: #{numeric_id} → {}", path.display());
                let mut scanners = state.transcript_scanners.write().await;
                scanners.insert(numeric_id, transcript::FileOffset::new(path));
            }

            // Send full session list update to simulator
            let sessions = {
                let store = state.store.read().await;
                store.to_protocol_list()
            };
            send_downlink(
                state,
                DownlinkMessage::SessionListUpdate {
                    sessions,
                    active_index: 0,
                },
            )
            .await;
        }

        SessionEvent::Ended { session_id } => {
            let mut store = state.store.write().await;
            let mut map = state.session_id_map.write().await;

            if let Some(&numeric_id) = map.get(&session_id) {
                store.remove(numeric_id);
                map.remove(&session_id);
                info!("session ended: #{numeric_id} hook_id={session_id}");

                let sessions = store.to_protocol_list();
                drop(store);
                drop(map);
                send_downlink(
                    state,
                    DownlinkMessage::SessionListUpdate {
                        sessions,
                        active_index: 0,
                    },
                )
                .await;
            } else {
                warn!("session end for unknown hook_id={session_id}");
            }
        }

        SessionEvent::StatusChanged { session_id, status } => {
            let mut store = state.store.write().await;
            let map = state.session_id_map.read().await;

            if let Some(&numeric_id) = map.get(&session_id) {
                let (old_status, session_name) = if let Some(s) = store.get_mut(numeric_id) {
                    let old = s.info.status;
                    let name = s.info.name.clone();
                    s.info.status = status;
                    (old, name)
                } else {
                    (SessionStatus::Idle, String::new())
                };
                info!("session #{numeric_id} status -> {status:?}");
                drop(store);
                drop(map);
                send_downlink(
                    state,
                    DownlinkMessage::SessionStatusChange {
                        session_id: numeric_id,
                        status,
                    },
                )
                .await;
                // CESP event routing: trigger Sound + LED + Notification
                if old_status != status {
                    cesp::route_status_change(
                        &state.notification_queue,
                        state,
                        numeric_id,
                        &session_name,
                        old_status,
                        status,
                        &format!("{status:?}"),
                    )
                    .await;
                }
            } else {
                warn!("status change for unknown hook_id={session_id}");
            }
        }

        SessionEvent::PermissionRequest {
            session_id,
            tool_name,
            tool_input,
        } => {
            let map = state.session_id_map.read().await;
            let numeric_id = match map.get(&session_id) {
                Some(&id) => id,
                None => {
                    warn!("permission request for unknown hook_id={session_id}");
                    return;
                }
            };
            drop(map);

            // Check always-allow list first (runtime "Always" from user)
            {
                let pq = state.perm_queue.read().await;
                if pq.is_always_allowed(&tool_name, &tool_input) {
                    info!("always-allow: {tool_name}({tool_input}) for #{numeric_id}");
                    return;
                }
            }

            // Check YOLO rules
            let yolo = state.yolo.read().await;
            let decision = evaluate_yolo(&yolo, &tool_name, &tool_input);
            drop(yolo);

            match decision {
                YoloDecision::AutoAllow => {
                    info!("YOLO auto-allow: {tool_name}({tool_input}) for #{numeric_id}");
                    // No need to bother the user — just dismiss
                }
                YoloDecision::AutoDeny => {
                    info!("YOLO auto-deny: {tool_name}({tool_input}) for #{numeric_id}");
                    // TODO: send deny response back to hook caller if protocol supports it
                }
                YoloDecision::AskUser => {
                    // Queue permission and notify simulator
                    let mut store = state.store.write().await;
                    if let Some(s) = store.get_mut(numeric_id) {
                        s.info.has_permission_request = true;
                        s.info.status = SessionStatus::PermissionNeeded;
                    }
                    drop(store);

                    let mut pq = state.perm_queue.write().await;
                    pq.push(PendingPermission {
                        session_id: numeric_id,
                        tool_name: tool_name.clone(),
                        tool_input: tool_input.clone(),
                    });
                    drop(pq);

                    let desc = format!("{tool_name}({tool_input})");
                    info!("permission queued: {desc} for #{numeric_id}");
                    send_downlink(
                        state,
                        DownlinkMessage::PermissionRequest {
                            session_id: numeric_id,
                            action_desc: desc,
                        },
                    )
                    .await;
                    // LED + Sound cues for permission arrival
                    send_downlink(state, DownlinkMessage::SetKnobRing(LedColor::GREEN)).await;
                    send_downlink(state, DownlinkMessage::PlaySound(vk_protocol::message::SoundType::PermissionAlert)).await;
                }
            }
        }
    }
}

/// Handle uplink messages from the simulator.
pub(super) async fn handle_uplink(state: &DaemonState, msg: UplinkMessage) {
    match msg {
        UplinkMessage::ButtonPress(ButtonId::Send) => {
            // Resolve pending permission as Allow
            let mut pq = state.perm_queue.write().await;
            if let Some(current) = pq.current().cloned() {
                let resolved = pq.resolve(current.session_id, PermissionAction::Allow);
                drop(pq);
                if let Some(perm) = resolved {
                    info!(
                        "permission ALLOW for #{}: {}({})",
                        perm.session_id, perm.tool_name, perm.tool_input
                    );
                    // Clear permission flag on session
                    let mut store = state.store.write().await;
                    if let Some(s) = store.get_mut(perm.session_id) {
                        s.info.has_permission_request = false;
                        s.info.status = SessionStatus::Idle;
                    }
                    drop(store);
                    send_downlink(state, DownlinkMessage::DismissPermission { session_id: perm.session_id }).await;
                    notify_permission_resolved(state, perm.session_id, PermissionAction::Allow).await;
                }
            } else {
                drop(pq);
                // Normal mode: SEND → focus terminal + Enter key
                info!("ButtonPress(Send) — sending Enter key");
                focus_active_then_keystroke(state, "enter").await;
            }
        }

        UplinkMessage::ButtonPress(ButtonId::Cancel) => {
            // If in Notify mode: CANCEL = delete current notification
            let in_notify = state.ui_state.read().await.state() == vk_ui::screen::ScreenState::Notify;
            if in_notify {
                let mut ui = state.ui_state.write().await;
                let notify_idx = ui.notify_index();
                let session_id = ui.notifications().get(notify_idx).map(|n| n.session_id);
                ui.handle_event(&vk_ui::event::UiEvent::ButtonPress(ButtonId::Cancel));
                drop(ui);
                if let Some(sid) = session_id {
                    state.notification_queue.write().await.remove_by_session(sid);
                    info!("Notify CANCEL → deleted notification for session #{sid}");
                }
            } else {
            // Resolve pending permission as Deny
            let mut pq = state.perm_queue.write().await;
            if let Some(current) = pq.current().cloned() {
                let resolved = pq.resolve(current.session_id, PermissionAction::Deny);
                drop(pq);
                if let Some(perm) = resolved {
                    info!(
                        "permission DENY for #{}: {}({})",
                        perm.session_id, perm.tool_name, perm.tool_input
                    );
                    let mut store = state.store.write().await;
                    if let Some(s) = store.get_mut(perm.session_id) {
                        s.info.has_permission_request = false;
                        s.info.status = SessionStatus::Idle;
                    }
                    drop(store);
                    send_downlink(state, DownlinkMessage::DismissPermission { session_id: perm.session_id }).await;
                    notify_permission_resolved(state, perm.session_id, PermissionAction::Deny).await;
                }
            } else {
                drop(pq);
                // Normal mode: CANCEL → focus terminal + Escape key
                info!("ButtonPress(Cancel) — sending Escape key");
                focus_active_then_keystroke(state, "escape").await;
            }
            } // close else (not in_notify)
        }

        UplinkMessage::ButtonPress(ButtonId::Mode) => {
            let mut yolo = state.yolo.write().await;
            yolo.active = !yolo.active;
            let mode = if yolo.active { "YOLO" } else { "Default" };
            info!("Mode toggled: {mode}");
            drop(yolo);
            super::api::log_activity(state, format!("MODE → {mode}")).await;
        }

        UplinkMessage::ButtonPress(ButtonId::Session) => {
            // Forward to UI state machine — opens Notify screen if has unread notifications
            info!("ButtonPress(Session) — checking notifications...");
            {
                let nq = state.notification_queue.read().await;
                let unread = nq.unread_count();
                let total = nq.all().len();
                info!("  notification_queue: {unread} unread, {total} total");
            }
            {
                let mut ui = state.ui_state.write().await;
                let unread = ui.unread_count();
                info!("  ui_state unread_count: {unread}, state: {:?}", ui.state());
                let action = ui.handle_event(&vk_ui::event::UiEvent::ButtonPress(ButtonId::Session));
                info!("  after handle_event → state: {:?}, action: {:?}", ui.state(), action);
                match action {
                    vk_ui::screen::UiAction::SwitchSession { session_id } => {
                        drop(ui);
                        // Remove consumed notification from daemon queue
                        state.notification_queue.write().await.remove_by_session(session_id);
                        // Focus the session's terminal window
                        let store = state.store.read().await;
                        if let Some(s) = store.get(session_id) {
                            info!("Notify → focus session #{session_id} ({}), notification consumed", s.info.name);
                            if let Err(e) = crate::focus::activate_with_strategies(&state.focus_strategies, s) {
                                warn!("focus failed for #{session_id}: {e}");
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        UplinkMessage::ButtonPress(ButtonId::Delete) => {
            let macros = state.macro_config.read().await;
            let action = macros.delete.clone();
            drop(macros);
            if !action.is_empty() {
                info!("ButtonPress(Delete) — executing: {action}");
                focus_active_then_keystroke(state, &action).await;
            }
        }

        UplinkMessage::ButtonPress(ButtonId::Voice) => {
            let macros = state.macro_config.read().await;
            let action = macros.voice.clone();
            drop(macros);
            if !action.is_empty() {
                info!("ButtonPress(Voice) — executing: {action}");
                focus_active_then_keystroke(state, &action).await;
            }
        }

        UplinkMessage::ButtonRelease(button) => {
            info!("ButtonRelease({button:?}) — ignored");
        }

        UplinkMessage::KnobRotate { direction, steps } => {
            info!("KnobRotate({direction:?}, {steps})");
            let mut ui = state.ui_state.write().await;
            let s = if direction == vk_protocol::message::Direction::Clockwise {
                steps as i8
            } else {
                -(steps as i8)
            };
            let action = ui.handle_event(&vk_ui::event::UiEvent::KnobRotate { steps: s });
            drop(ui);
            // Normal rotate now returns SwitchSession — trigger focus
            if let vk_ui::screen::UiAction::SwitchSession { session_id } = action {
                let store = state.store.read().await;
                if let Some(s) = store.get(session_id) {
                    if let Err(e) = crate::focus::activate_with_strategies(&state.focus_strategies, s) {
                        warn!("focus failed: {e}");
                    }
                }
            }
        }

        UplinkMessage::KnobPress => {
            info!("KnobPress");
            let mut ui = state.ui_state.write().await;
            let action = ui.handle_event(&vk_ui::event::UiEvent::KnobPress);
            drop(ui);
            // If switch session, trigger focus + consume notification
            if let vk_ui::screen::UiAction::SwitchSession { session_id } = action {
                info!("KnobPress → SwitchSession #{session_id}");
                // Consume notification from daemon queue
                state.notification_queue.write().await.remove_by_session(session_id);
                // Focus terminal window
                let store = state.store.read().await;
                if let Some(s) = store.get(session_id) {
                    if let Err(e) = crate::focus::activate_with_strategies(&state.focus_strategies, s) {
                        warn!("focus failed: {e}");
                    }
                }
            }
        }

        UplinkMessage::KnobRelease => {
            info!("KnobRelease — UI-only, handled by simulator");
        }

        UplinkMessage::PermissionResponse {
            session_id,
            action,
        } => {
            let mut pq = state.perm_queue.write().await;
            let resolved = pq.resolve(session_id, action);
            drop(pq);
            if let Some(perm) = resolved {
                info!(
                    "PermissionResponse({action:?}) for #{session_id}: {}({})",
                    perm.tool_name, perm.tool_input
                );
                let mut store = state.store.write().await;
                if let Some(s) = store.get_mut(session_id) {
                    s.info.has_permission_request = false;
                    s.info.status = SessionStatus::Idle;
                }
                drop(store);
                send_downlink(state, DownlinkMessage::DismissPermission { session_id: perm.session_id }).await;
                notify_permission_resolved(state, perm.session_id, action).await;
            } else {
                warn!("PermissionResponse for #{session_id} — no pending permission");
            }
        }

        UplinkMessage::SessionSwitch { session_id } => {
            info!("SessionSwitch to #{session_id}");
            let store = state.store.read().await;
            if let Some(s) = store.get(session_id) {
                if let Err(e) = crate::focus::activate_with_strategies(&state.focus_strategies, s) {
                    warn!("focus failed for #{session_id}: {e}");
                }
            } else {
                warn!("SessionSwitch to #{session_id} — session not found");
            }
        }
    }
    // T16.1: bump render generation on any UI interaction
    state.bump_render_generation();
}

/// Notify the waiting HTTP handler that a permission was resolved.
/// Also resets LED cues if no more pending permissions.
async fn notify_permission_resolved(state: &DaemonState, session_id: u16, action: PermissionAction) {
    let mut channels = state.perm_response_channels.write().await;
    if let Some(tx) = channels.remove(&session_id) {
        let _ = tx.send(PermissionDecision { action });
    }
    drop(channels);

    // Reset knob ring LED if no more pending permissions
    let pq = state.perm_queue.read().await;
    if pq.is_empty() {
        drop(pq);
        send_downlink(state, DownlinkMessage::SetKnobRing(LedColor::OFF)).await;
    }

    // T9.5: Persist always-allow to config.toml
    if action == PermissionAction::Always {
        let config_path = config::default_config_path();
        let mut cfg = config::load_config(&config_path);
        let pq = state.perm_queue.read().await;
        // Sync entire always_allow list from runtime to config
        cfg.always_allow.patterns = pq.always_allow_list().to_vec();
        drop(pq);
        if let Err(e) = config::save_config(&config_path, &cfg) {
            warn!("failed to persist always-allow: {e}");
        } else {
            info!("persisted always-allow to config.toml");
        }
    }
}

/// Focus the active session's terminal window, then inject a keystroke.
async fn focus_active_then_keystroke(state: &DaemonState, action: &str) {
    let ui = state.ui_state.read().await;
    let sessions = ui.sessions();
    let idx = ui.active_index();
    if let Some(session) = sessions.get(idx) {
        let session_id = session.id;
        drop(ui);
        let store = state.store.read().await;
        if let Some(s) = store.get(session_id) {
            // Focus terminal window first
            let _ = focus::macos::activate_window(s);
            drop(store);
            // Brief delay to let window activate
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    } else {
        drop(ui);
    }
    // Inject keystroke (goes to now-focused terminal)
    if let Err(e) = crate::keystroke::execute_button_action(action) {
        warn!("keystroke {action} failed: {e}");
    }
}
