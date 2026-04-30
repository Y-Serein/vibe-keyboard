//! UI events — actions that trigger screen state transitions.

use vk_protocol::message::{ButtonId, PermissionAction, SessionInfo, SessionStatus};

/// Events that the UI engine processes.
#[derive(Debug, Clone, PartialEq)]
pub enum UiEvent {
    /// A session was added or updated.
    SessionUpdate {
        session_id: u16,
        name: String,
        status: SessionStatus,
    },

    /// Full session list replacement with rich data (clears old sessions not in the list).
    SessionListReplace {
        sessions: Vec<SessionInfo>,
    },

    /// A session was removed.
    SessionRemoved { session_id: u16 },

    /// Permission request arrived.
    PermissionRequest {
        session_id: u16,
        action_desc: String,
    },

    /// Permission was resolved (externally, e.g. YOLO auto-approve).
    PermissionResolved {
        session_id: u16,
        action: PermissionAction,
    },

    /// Knob rotated (positive = CW, negative = CCW).
    KnobRotate { steps: i8 },

    /// Knob pressed.
    KnobPress,

    /// Button pressed.
    ButtonPress(ButtonId),

    /// Notification list update from daemon.
    NotificationListUpdate {
        notifications: Vec<crate::screen::NotificationEntry>,
    },

    /// Idle timeout (return to Normal from Select).
    IdleTimeout,
}
