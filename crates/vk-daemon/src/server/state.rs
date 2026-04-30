//! Daemon shared state.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::sync::{oneshot, RwLock};

use vk_protocol::message::PermissionAction;
use vk_transport::Transport;

use crate::config;
use crate::notification::NotificationQueue;
use crate::permission::{PermissionQueue, YoloConfig};
use crate::transcript;

/// Permission decision sent back to the waiting HTTP handler.
#[derive(Debug, Clone)]
pub struct PermissionDecision {
    pub action: PermissionAction,
}

/// Shared daemon state accessible from both the hook server and IPC handler.
pub(crate) struct DaemonState {
    pub(super) store: RwLock<crate::session::store::SessionStore>,
    pub(super) perm_queue: RwLock<PermissionQueue>,
    pub(super) yolo: RwLock<YoloConfig>,
    pub(super) macro_config: RwLock<config::MacroConfig>,
    /// Sender half for downlink messages to the simulator.
    /// None until IPC is connected.
    pub(super) ipc_downlink_tx: RwLock<Option<Arc<dyn Transport>>>,
    /// Map from hook session_id (string) to internal numeric id.
    pub(super) session_id_map: RwLock<HashMap<String, u16>>,
    /// Oneshot channels waiting for permission decisions. Key = numeric session_id.
    pub(super) perm_response_channels: RwLock<HashMap<u16, oneshot::Sender<PermissionDecision>>>,
    /// Transcript scanners per session. Key = numeric session_id.
    pub(super) transcript_scanners: RwLock<HashMap<u16, transcript::FileOffset>>,
    /// Daemon-side UI state for LCD Canvas rendering.
    pub(super) ui_state: RwLock<vk_ui::screen::ScreenStateMachine>,
    /// Latest rendered framebuffer (RGB565 raw bytes, zero-copy Arc via bytes::Bytes).
    pub(super) frame_buffer: RwLock<bytes::Bytes>,
    /// Generation counter — bumped on any state change that requires re-render.
    pub(super) render_generation: AtomicU64,
    /// LCD dimensions from config.
    pub(super) lcd_width: u16,
    pub(super) lcd_height: u16,
    /// Hook server port (for setup status reporting).
    pub(super) hook_port: u16,
    /// Toggle state for held keys: action → is_held
    pub(super) held_keys: RwLock<std::collections::HashSet<String>>,
    /// Recent activity log (ring buffer, max 50 entries)
    pub(super) activity_log: RwLock<Vec<String>>,
    /// Notification queue for CESP event routing (Done/Error/PermissionNeeded).
    pub(super) notification_queue: RwLock<NotificationQueue>,
    // ── Platform trait objects (M10) ──
    pub(super) focus_strategies: Vec<Box<dyn crate::focus::FocusStrategy>>,
    pub(super) keystroke_injector: Box<dyn crate::keystroke::KeystrokeInjector>,
    pub(super) notification_backend: Box<dyn crate::notification::NotificationBackend>,
    /// Local speaker for desktop sound playback when no device connected.
    pub(super) local_speaker: crate::local_speaker::LocalSpeaker,
}

impl DaemonState {
    /// Bump the render generation counter to signal the render loop that a re-render is needed.
    pub(super) fn bump_render_generation(&self) {
        self.render_generation.fetch_add(1, Ordering::Relaxed);
    }
}
