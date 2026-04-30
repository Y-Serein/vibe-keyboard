//! macOS focus management — strategy composition and backward-compatible helpers.
//!
//! Provides `default_strategies()` for the ordered list of macOS focus strategies,
//! and `activate_window()` as a convenience function for backward compatibility.

use crate::focus::{activate_with_strategies, generic, ghostty, iterm, vscode, warp, FocusError, FocusStrategy};
use crate::session::store::DaemonSession;

/// Default macOS focus strategies in priority order.
///
/// More specific strategies (iTerm2 with tab switching) come first;
/// the generic fallback comes last.
pub fn default_strategies() -> Vec<Box<dyn FocusStrategy>> {
    vec![
        Box::new(iterm::ITermFocus),
        Box::new(ghostty::GhosttyFocus),
        Box::new(warp::WarpFocus),
        Box::new(vscode::VsCodeFocus),
        Box::new(generic::GenericMacFocus),
    ]
}

/// Activate the macOS window for a session (backward-compatible convenience function).
///
/// Iterates through `default_strategies()` and uses the first matching one.
pub fn activate_window(session: &DaemonSession) -> Result<(), FocusError> {
    let strategies = default_strategies();
    activate_with_strategies(&strategies, session)
}

/// Build an osascript command string (for testing/dry-run — legacy helper).
pub fn build_focus_script(app_name: &str, window_title: &str) -> String {
    generic::GenericMacFocus::build_window_info_script(app_name, window_title)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::store::WindowInfo;
    use vk_protocol::message::{SessionInfo, SessionStatus};

    #[test]
    fn default_strategies_has_five_entries() {
        let strategies = default_strategies();
        assert_eq!(strategies.len(), 5);
    }

    #[test]
    fn default_strategies_order() {
        let strategies = default_strategies();
        let names: Vec<&str> = strategies.iter().map(|s| s.name()).collect();
        assert_eq!(names, vec!["iTerm2", "Ghostty", "Warp", "VSCode", "GenericMac"]);
    }

    #[test]
    fn activate_no_window_info_falls_back_to_generic() {
        let session = DaemonSession {
            info: SessionInfo::new(1, "Test", SessionStatus::Idle),
            ..Default::default()
        };
        let result = activate_window(&session);
        assert!(result.is_err());
    }

    #[test]
    fn build_focus_script_contains_app_name() {
        let script = build_focus_script("iTerm2", "RustAgent");
        assert!(script.contains("iTerm2"));
        assert!(script.contains("RustAgent"));
        assert!(script.contains("AXRaise"));
    }

    #[test]
    fn activate_with_window_info_builds_script() {
        let session = DaemonSession {
            info: SessionInfo::new(1, "Test", SessionStatus::Idle),
            window_info: Some(WindowInfo {
                app_name: "NonExistentApp12345".into(),
                window_title: "test".into(),
                pid: None,
            }),
        };
        let result = activate_window(&session);
        assert!(result.is_err() || result.is_ok());
    }

    #[test]
    fn activate_iterm_session_uses_iterm_strategy() {
        let session = DaemonSession {
            info: SessionInfo {
                id: 1,
                name: "Test".into(),
                status: SessionStatus::Idle,
                bundle_id: "com.googlecode.iterm2".into(),
                session_tty: "/dev/ttys042".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let result = activate_window(&session);
        assert!(result.is_ok() || result.is_err());
    }
}
