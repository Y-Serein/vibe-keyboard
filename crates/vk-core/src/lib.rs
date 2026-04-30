//! Vibe Keyboard shared types — zero external dependencies.
//!
//! Core domain types used across all crates: button IDs, session status,
//! sound types, session info, and notification info.

pub mod sounds;

/// Button identifier matching the V2 physical layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ButtonId {
    Delete,
    Cancel,
    Mode,
    Session,
    Send,
    Voice,
}

/// Rotary encoder direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Direction {
    Clockwise,
    CounterClockwise,
}

/// LED color (RGB).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LedColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl LedColor {
    pub const OFF: Self = Self { r: 0, g: 0, b: 0 };
    pub const GREEN: Self = Self { r: 0, g: 200, b: 0 };
    pub const AMBER: Self = Self { r: 245, g: 158, b: 11 };
    pub const RED: Self = Self { r: 239, g: 68, b: 68 };
    pub const ORANGE: Self = Self { r: 255, g: 140, b: 0 };
}

/// Session status as displayed on the LCD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Thinking,
    ToolUse,
    Writing,
    Done,
    Error,
    Idle,
    PermissionNeeded,
}

impl SessionStatus {
    /// Priority value for sorting — lower = more urgent.
    /// Used by both UI (session list) and daemon (notification ordering).
    pub fn priority(self) -> u8 {
        match self {
            Self::PermissionNeeded => 0,
            Self::Error => 1,
            Self::Thinking | Self::ToolUse | Self::Writing => 2,
            Self::Done => 3,
            Self::Idle => 4,
        }
    }
}

/// Permission action options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum PermissionAction {
    Allow,
    Deny,
    Always,
}

/// Sound types the speaker can play.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum SoundType {
    PermissionAlert,
    SessionComplete,
    Error,
    Click,
}

/// Session info pushed to the keyboard for display.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SessionInfo {
    // -- Basic (from hook events) --
    pub id: u16,
    pub name: String,
    pub status: SessionStatus,
    pub has_permission_request: bool,

    // -- Hook event extensions --
    pub source: String,
    pub cwd: String,
    pub permission_mode: String,

    // -- JSONL transcript parsing --
    pub model: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cost_usd: f64,
    pub context_pct: u8,
    pub last_message: String,
    pub last_ai_output: String,

    // -- Window focus --
    pub bundle_id: String,
    pub session_tty: String,

    // -- Timing --
    pub started_at: u64,
    pub last_activity: u64,
}

impl Default for SessionInfo {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            status: SessionStatus::Idle,
            has_permission_request: false,
            source: String::new(),
            cwd: String::new(),
            permission_mode: String::new(),
            model: String::new(),
            tokens_in: 0,
            tokens_out: 0,
            cost_usd: 0.0,
            context_pct: 0,
            last_message: String::new(),
            last_ai_output: String::new(),
            bundle_id: String::new(),
            session_tty: String::new(),
            started_at: 0,
            last_activity: 0,
        }
    }
}

impl SessionInfo {
    /// Create a minimal SessionInfo with just the basic fields.
    pub fn new(id: u16, name: impl Into<String>, status: SessionStatus) -> Self {
        Self {
            id,
            name: name.into(),
            status,
            ..Default::default()
        }
    }
}

/// Notification info pushed to the keyboard for display.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NotificationInfo {
    pub id: u32,
    pub session_id: u16,
    pub session_name: String,
    pub status: SessionStatus,
    pub description: String,
    pub timestamp: u64,
    pub read: bool,
}

