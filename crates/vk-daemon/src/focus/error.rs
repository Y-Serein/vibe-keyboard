//! Focus error types.

use std::fmt;

/// Error type for focus operations.
#[derive(Debug)]
pub enum FocusError {
    /// No focus strategy matched the session.
    NoStrategyMatched,
    /// The underlying OS command (osascript, etc.) failed to execute.
    CommandFailed(String),
    /// The focus command ran but reported failure.
    ActivationFailed(String),
    /// No bundle_id or window info available for the session.
    NoTargetInfo,
    /// Focus spawn (detached) failed.
    SpawnFailed(String),
}

impl fmt::Display for FocusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FocusError::NoStrategyMatched => write!(f, "no focus strategy matched"),
            FocusError::CommandFailed(msg) => write!(f, "command failed: {msg}"),
            FocusError::ActivationFailed(msg) => write!(f, "activation failed: {msg}"),
            FocusError::NoTargetInfo => write!(f, "no bundle_id or window_info available"),
            FocusError::SpawnFailed(msg) => write!(f, "focus spawn failed: {msg}"),
        }
    }
}

impl std::error::Error for FocusError {}
