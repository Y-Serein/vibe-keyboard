//! Ghostty-specific focus strategy — NSRunningApplication activation.

use crate::focus::{FocusError, FocusStrategy};
use crate::session::store::DaemonSession;
use std::process::Command;

const BUNDLE_ID: &str = "com.mitchellh.ghostty";

/// Focus strategy for Ghostty terminal.
pub struct GhosttyFocus;

impl GhosttyFocus {
    /// Build JXA script to activate Ghostty via NSRunningApplication.
    pub fn build_activate_script() -> String {
        format!(
            "ObjC.import('Cocoa');\
             var target='{BUNDLE_ID}'.toLowerCase();\
             var apps=$.NSWorkspace.sharedWorkspace.runningApplications;\
             for(var i=0;i<apps.count;i++){{\
             var app=apps.objectAtIndex(i);\
             if(!app.bundleIdentifier.isNil()&&app.bundleIdentifier.js.toLowerCase()===target){{\
             app.activateWithOptions($.NSApplicationActivateIgnoringOtherApps);break}}}}"
        )
    }

    /// Build JXA script to check if Ghostty is the frontmost app.
    pub fn build_is_focused_script() -> String {
        format!(
            "ObjC.import('Cocoa');\
             var front=$.NSWorkspace.sharedWorkspace.frontmostApplication;\
             front.bundleIdentifier.js.toLowerCase()==='{}'.toLowerCase()?'true':'false'",
            BUNDLE_ID
        )
    }
}

impl FocusStrategy for GhosttyFocus {
    fn can_focus(&self, session: &DaemonSession) -> bool {
        session.info.bundle_id.eq_ignore_ascii_case(BUNDLE_ID)
    }

    fn activate(&self, session: &DaemonSession) -> Result<(), FocusError> {
        let _ = session; // Ghostty doesn't support tab-level switching
        let script = Self::build_activate_script();
        let output = Command::new("osascript")
            .args(["-l", "JavaScript", "-e", &script])
            .output()
            .map_err(|e| FocusError::CommandFailed(format!("osascript failed: {e}")))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(FocusError::ActivationFailed(format!(
                "Ghostty focus failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )))
        }
    }

    fn is_focused(&self, _session: &DaemonSession) -> bool {
        let script = Self::build_is_focused_script();
        let output = Command::new("osascript")
            .args(["-l", "JavaScript", "-e", &script])
            .output();
        matches!(output, Ok(o) if String::from_utf8_lossy(&o.stdout).trim() == "true")
    }

    fn name(&self) -> &str {
        "Ghostty"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vk_protocol::message::{SessionInfo, SessionStatus};

    fn make_session(bundle_id: &str) -> DaemonSession {
        DaemonSession {
            info: SessionInfo {
                id: 1,
                name: "test".into(),
                status: SessionStatus::Idle,
                bundle_id: bundle_id.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn can_focus_matches_ghostty() {
        let focus = GhosttyFocus;
        assert!(focus.can_focus(&make_session("com.mitchellh.ghostty")));
        assert!(focus.can_focus(&make_session("com.mitchellh.Ghostty")));
    }

    #[test]
    fn can_focus_rejects_others() {
        let focus = GhosttyFocus;
        assert!(!focus.can_focus(&make_session("com.googlecode.iterm2")));
        assert!(!focus.can_focus(&make_session("")));
    }

    #[test]
    fn activate_script_contains_bundle_id() {
        let script = GhosttyFocus::build_activate_script();
        assert!(script.contains(BUNDLE_ID));
        assert!(script.contains("activateWithOptions"));
    }

    #[test]
    fn is_focused_script_checks_frontmost() {
        let script = GhosttyFocus::build_is_focused_script();
        assert!(script.contains("frontmostApplication"));
        assert!(script.contains(BUNDLE_ID));
    }

    #[test]
    fn name_is_ghostty() {
        assert_eq!(GhosttyFocus.name(), "Ghostty");
    }
}
