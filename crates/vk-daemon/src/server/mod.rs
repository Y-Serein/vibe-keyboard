//! Daemon serve mode — hook server (axum) + IPC listener (Unix socket).
//!
//! Runs two concurrent loops:
//! 1. HTTP hook server on `config.general.hook_port` — receives HookEvent from AI tools
//! 2. IPC listener on `config.ipc.socket_path` — communicates with simulator via Transport

mod api;
mod ipc_handler;
mod render;
mod scanner;
mod state;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use tokio::sync::RwLock;
use tracing::{error, info};

use crate::config;
use crate::notification::NotificationQueue;
use crate::permission::{PermissionQueue, YoloConfig};
use crate::session::store::DaemonSession;
use crate::transcript;
use vk_protocol::message::{SessionInfo, SessionStatus};

use state::DaemonState;

// Re-export PermissionDecision so external tests can reference it if needed.
pub use state::PermissionDecision;

/// Main entry point for the daemon serve mode.
pub async fn run_serve(headless: bool) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = config::default_config_path();
    let cfg = config::load_config(&config_path);

    info!(
        "vk-daemon serve mode (headless={headless}) hook_port={} ipc={}",
        cfg.general.hook_port, cfg.ipc.socket_path
    );

    let yolo = YoloConfig {
        active: cfg.yolo.active,
        allow: cfg.yolo.allow.clone(),
        deny: cfg.yolo.deny.clone(),
        notify_auto_allow: cfg.yolo.notify_auto_allow,
        auto_allow_log: cfg.yolo.auto_allow_log,
    };

    // Load persisted always-allow patterns
    let mut perm_queue = PermissionQueue::new();
    for pattern in &cfg.always_allow.patterns {
        perm_queue.add_always_allow(pattern.clone());
    }

    let state = Arc::new(DaemonState {
        store: RwLock::new(crate::session::store::SessionStore::new()),
        perm_queue: RwLock::new(perm_queue),
        yolo: RwLock::new(yolo),
        macro_config: RwLock::new(cfg.macros.clone()),
        ipc_downlink_tx: RwLock::new(None),
        session_id_map: RwLock::new(HashMap::new()),
        perm_response_channels: RwLock::new(HashMap::new()),
        transcript_scanners: RwLock::new(HashMap::new()),
        ui_state: RwLock::new(vk_ui::screen::ScreenStateMachine::new()),
        frame_buffer: RwLock::new(bytes::Bytes::from(vec![0u8; cfg.display.width as usize * cfg.display.height as usize * 2])),
        render_generation: AtomicU64::new(1),
        lcd_width: cfg.display.width,
        lcd_height: cfg.display.height,
        hook_port: cfg.general.hook_port,
        held_keys: RwLock::new(std::collections::HashSet::new()),
        activity_log: RwLock::new(Vec::new()),
        notification_queue: RwLock::new(NotificationQueue::new()),
        focus_strategies: crate::focus::macos::default_strategies(),
        keystroke_injector: crate::keystroke::default_injector(),
        notification_backend: crate::notification::default_backend(),
        local_speaker: crate::local_speaker::LocalSpeaker::new(),
    });

    // Initial scan: discover active sessions from filesystem
    {
        let discovered = transcript::discover_active_sessions();
        if !discovered.is_empty() {
            let mut store = state.store.write().await;
            let mut scanners = state.transcript_scanners.write().await;
            let mut map = state.session_id_map.write().await;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            for ds in &discovered {
                let id = store.allocate_id();
                // Use last segment of CWD as project name (e.g. "vibe-keyboard")
                let cwd_path = std::path::Path::new(&ds.cwd);
                let dir_name = cwd_path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| ds.session_id[..8.min(ds.session_id.len())].to_string());

                // Detect terminal: default to iTerm2 for CLI Claude Code
                let bundle_id = std::env::var("TERM_PROGRAM")
                    .map(|tp| match tp.as_str() {
                        "iTerm.app" => "com.googlecode.iterm2".to_string(),
                        "ghostty" => "com.mitchellh.ghostty".to_string(),
                        "WarpTerminal" => "dev.warp.Warp-Stable".to_string(),
                        _ => "com.googlecode.iterm2".to_string(),
                    })
                    .unwrap_or_else(|_| "com.googlecode.iterm2".to_string());

                let session = DaemonSession {
                    info: SessionInfo {
                        id,
                        name: dir_name,
                        status: SessionStatus::Idle,
                        source: "claude-code".into(),
                        cwd: ds.cwd.clone(),
                        bundle_id,
                        started_at: now,
                        last_activity: now,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                store.update(session);
                map.insert(ds.session_id.clone(), id);
                scanners.insert(id, transcript::FileOffset::new(ds.transcript_path.clone()));
                info!("initial_scan: #{id} ({}) → {}", ds.cwd, ds.transcript_path.display());
            }
            info!("initial_scan: discovered {} active sessions", discovered.len());
        }
    }

    let hook_state = Arc::clone(&state);
    let ipc_state = Arc::clone(&state);
    let scan_state = Arc::clone(&state);
    let render_state = Arc::clone(&state);

    let hook_port = cfg.general.hook_port;
    let socket_path = cfg.ipc.socket_path.clone();

    tokio::select! {
        result = api::run_hook_server(hook_state, hook_port) => {
            error!("hook server exited: {result:?}");
            result
        }
        result = ipc_handler::run_ipc_listener(ipc_state, &socket_path) => {
            error!("IPC listener exited: {result:?}");
            result
        }
        _ = scanner::run_transcript_scanner(scan_state) => {
            error!("transcript scanner exited");
            Ok(())
        }
        _ = scanner::run_process_scanner(Arc::clone(&state)) => {
            error!("process scanner exited");
            Ok(())
        }
        _ = render::run_render_loop(render_state) => {
            error!("render loop exited");
            Ok(())
        }
        _ = tokio::signal::ctrl_c() => {
            info!("received Ctrl+C, shutting down...");
            // Clean up IPC socket
            let _ = std::fs::remove_file(&socket_path);
            Ok(())
        }
    }
}
