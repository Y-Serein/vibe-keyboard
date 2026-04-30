//! Protocol message definitions.
//!
//! All messages exchanged between the keyboard (simulator/firmware) and the daemon.
//! Core types (ButtonId, SessionStatus, etc.) are re-exported from vk-core.

// Re-export all core types so existing `use vk_protocol::message::*` still works.
pub use vk_core::{
    ButtonId, Direction, LedColor, SessionStatus, PermissionAction, SoundType,
    SessionInfo, NotificationInfo,
};

// ── Uplink: keyboard → daemon ──

/// Messages sent from keyboard to daemon.
///
/// The simulator resolves UI state locally and sends high-level semantic
/// messages to the daemon. For example, instead of sending raw ButtonPress(Send)
/// while in Allow mode, it sends PermissionResponse with the resolved action.
#[derive(Debug, Clone, PartialEq)]
pub enum UplinkMessage {
    /// Button pressed.
    ButtonPress(ButtonId),
    /// Button released.
    ButtonRelease(ButtonId),
    /// Rotary encoder rotated.
    KnobRotate {
        direction: Direction,
        steps: u8,
    },
    /// Rotary encoder pressed.
    KnobPress,
    /// Rotary encoder released.
    KnobRelease,
    /// Permission response from user (semantic: simulator resolves UI state).
    PermissionResponse {
        session_id: u16,
        action: PermissionAction,
    },
    /// Session switch request (semantic: simulator resolves selected session).
    SessionSwitch {
        session_id: u16,
    },
}

// ── Downlink: daemon → keyboard ──

/// Messages sent from daemon to keyboard.
#[derive(Debug, Clone, PartialEq)]
pub enum DownlinkMessage {
    /// Full session list update.
    SessionListUpdate {
        sessions: Vec<SessionInfo>,
        active_index: u8,
    },
    /// Single session status change.
    SessionStatusChange {
        session_id: u16,
        status: SessionStatus,
    },
    /// Permission request to display.
    PermissionRequest {
        session_id: u16,
        action_desc: String,
    },
    /// Set LED state.
    SetLed {
        button: ButtonId,
        color: LedColor,
        blink: bool,
    },
    /// Set knob ring color.
    SetKnobRing(LedColor),
    /// Play sound.
    PlaySound(SoundType),
    /// Dismiss permission dialog for a specific session.
    DismissPermission { session_id: u16 },
    /// Notification list update.
    NotificationListUpdate {
        notifications: Vec<NotificationInfo>,
    },
    /// Raw framebuffer pixel data for LCD Canvas rendering.
    FrameData {
        width: u16,
        height: u16,
        pixels: Vec<u8>, // RGB565 raw bytes, length = width * height * 2
    },
    /// Set speaker volume (0-100).
    SetVolume(u8),
    /// Set speaker muted state.
    SetMuted(bool),
    /// Set sound mapping for an event type.
    SetSoundMapping {
        sound_type: SoundType,
        sound_id: String, // "builtin:alert" or "custom:xxx"
    },
}
