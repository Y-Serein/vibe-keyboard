//! VS Code / Cursor / Windsurf focus strategy — dynamic process detection.

use crate::focus::{escape_jxa_string, validate_bundle_id, FocusError, FocusStrategy};
use crate::session::store::DaemonSession;
use std::process::Command;

/// Known bundle_id substrings for VS Code-family editors.
const VSCODE_PATTERNS: &[&str] = &["vscode", "cursor", "windsurf", "todesktop"];

/// Focus strategy for VS Code, Cursor, and Windsurf editors.
pub struct VsCodeFocus;

impl VsCodeFocus {
    /// Check if a bundle_id belongs to the VS Code family.
    pub fn is_vscode_family(bundle_id: &str) -> bool {
        let lower = bundle_id.to_lowercase();
        VSCODE_PATTERNS.iter().any(|pat| lower.contains(pat))
    }

    /// Build JXA script to activate a VS Code-family app by its exact bundle ID.
    pub fn build_activate_script(bundle_id: &str) -> String {
        if !validate_bundle_id(bundle_id) {
            return "'invalid_bundle_id'".to_string();
        }
        let bundle_id = escape_jxa_string(bundle_id);
        format!(
            "ObjC.import('Cocoa');\
             var target='{bundle_id}'.toLowerCase();\
             var apps=$.NSWorkspace.sharedWorkspace.runningApplications;\
             for(var i=0;i<apps.count;i++){{\
             var app=apps.objectAtIndex(i);\
             if(!app.bundleIdentifier.isNil()&&app.bundleIdentifier.js.toLowerCase()===target){{\
             app.activateWithOptions($.NSApplicationActivateIgnoringOtherApps);break}}}}"
        )
    }

    /// Build JXA script to check if a VS Code-family app is frontmost.
    pub fn build_is_focused_script(bundle_id: &str) -> String {
        if !validate_bundle_id(bundle_id) {
            return "'invalid_bundle_id'".to_string();
        }
        let bundle_id = escape_jxa_string(bundle_id);
        format!(
            "ObjC.import('Cocoa');\
             var front=$.NSWorkspace.sharedWorkspace.frontmostApplication;\
             front.bundleIdentifier.js.toLowerCase()==='{}'.toLowerCase()?'true':'false'",
            bundle_id
        )
    }
}

impl FocusStrategy for VsCodeFocus {
    fn can_focus(&self, session: &DaemonSession) -> bool {
        Self::is_vscode_family(&session.info.bundle_id)
    }

    fn activate(&self, session: &DaemonSession) -> Result<(), FocusError> {
        let script = Self::build_activate_script(&session.info.bundle_id);
        let output = Command::new("osascript")
            .args(["-l", "JavaScript", "-e", &script])
            .output()
            .map_err(|e| FocusError::CommandFailed(format!("osascript failed: {e}")))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(FocusError::ActivationFailed(format!(
                "VS Code focus failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )))
        }
    }

    fn is_focused(&self, session: &DaemonSession) -> bool {
        let script = Self::build_is_focused_script(&session.info.bundle_id);
        let output = Command::new("osascript")
            .args(["-l", "JavaScript", "-e", &script])
            .output();
        matches!(output, Ok(o) if String::from_utf8_lossy(&o.stdout).trim() == "true")
    }

    fn name(&self) -> &str {
        "VSCode"
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
    fn is_vscode_family_detects_variants() {
        assert!(VsCodeFocus::is_vscode_family("com.microsoft.VSCode"));
        // Cursor's actual bundle_id — we match on "cursor"
        assert!(VsCodeFocus::is_vscode_family("com.cursor.Cursor"));
        assert!(VsCodeFocus::is_vscode_family("io.codeium.windsurf"));
        // Cursor's real production bundle_id
        assert!(VsCodeFocus::is_vscode_family("com.todesktop.230313mzl4w4u92"));
    }

    #[test]
    fn is_vscode_family_rejects_non_vscode() {
        assert!(!VsCodeFocus::is_vscode_family("com.googlecode.iterm2"));
        assert!(!VsCodeFocus::is_vscode_family("com.mitchellh.ghostty"));
        assert!(!VsCodeFocus::is_vscode_family(""));
    }

    #[test]
    fn can_focus_matches_vscode_family() {
        let focus = VsCodeFocus;
        assert!(focus.can_focus(&make_session("com.microsoft.VSCode")));
        assert!(focus.can_focus(&make_session("com.cursor.Cursor")));
        assert!(focus.can_focus(&make_session("io.codeium.windsurf")));
    }

    #[test]
    fn can_focus_rejects_others() {
        let focus = VsCodeFocus;
        assert!(!focus.can_focus(&make_session("com.mitchellh.ghostty")));
        assert!(!focus.can_focus(&make_session("")));
    }

    #[test]
    fn activate_script_uses_session_bundle_id() {
        let script = VsCodeFocus::build_activate_script("com.microsoft.VSCode");
        assert!(script.contains("com.microsoft.VSCode"));
        assert!(script.contains("activateWithOptions"));
    }

    #[test]
    fn is_focused_script_uses_session_bundle_id() {
        let script = VsCodeFocus::build_is_focused_script("com.cursor.Cursor");
        assert!(script.contains("com.cursor.Cursor"));
        assert!(script.contains("frontmostApplication"));
    }

    #[test]
    fn name_is_vscode() {
        assert_eq!(VsCodeFocus.name(), "VSCode");
    }

    #[test]
    fn activate_script_rejects_malicious_bundle_id() {
        let script = VsCodeFocus::build_activate_script("com.evil';hack();'");
        assert_eq!(script, "'invalid_bundle_id'");
        assert!(!script.contains("hack"));
    }

    #[test]
    fn is_focused_script_rejects_malicious_bundle_id() {
        let script = VsCodeFocus::build_is_focused_script("';require('child_process');'");
        assert_eq!(script, "'invalid_bundle_id'");
        assert!(!script.contains("child_process"));
    }
}
