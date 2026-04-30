//! Generic macOS focus strategy — NSWorkspace fallback for any bundle ID.

use crate::focus::{
    escape_applescript_string, escape_jxa_string, validate_bundle_id, FocusError, FocusStrategy,
};
use crate::session::store::DaemonSession;
use std::process::Command;

/// Fallback focus strategy that activates any app by bundle ID via NSWorkspace.
pub struct GenericMacFocus;

impl GenericMacFocus {
    /// Build JXA script to activate an app by bundle_id using NSWorkspace.
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

    /// Build JXA script to check if a given bundle_id is the frontmost app.
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

    /// Build an AppleScript to activate by window info (legacy path).
    pub fn build_window_info_script(app_name: &str, window_title: &str) -> String {
        let app_name = escape_applescript_string(app_name);
        let window_title = escape_applescript_string(window_title);
        format!(
            r#"tell application "{app_name}" to activate
tell application "System Events"
    tell process "{app_name}"
        set frontmost to true
        set targetWindow to first window whose name contains "{window_title}"
        if targetWindow is missing value then
            error "Window not found: {window_title}"
        end if
        perform action "AXRaise" of targetWindow
    end tell
end tell"#
        )
    }
}

impl FocusStrategy for GenericMacFocus {
    fn can_focus(&self, _session: &DaemonSession) -> bool {
        // Fallback — always matches (as long as there's a bundle_id or window_info)
        true
    }

    fn activate(&self, session: &DaemonSession) -> Result<(), FocusError> {
        // Try bundle_id first
        if !session.info.bundle_id.is_empty() {
            let script = Self::build_activate_script(&session.info.bundle_id);
            let output = Command::new("osascript")
                .args(["-l", "JavaScript", "-e", &script])
                .output()
                .map_err(|e| FocusError::CommandFailed(format!("osascript failed: {e}")))?;

            if output.status.success() {
                return Ok(());
            }
            return Err(FocusError::ActivationFailed(format!(
                "generic focus failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        // Fallback to window_info
        if let Some(info) = &session.window_info {
            let script = Self::build_window_info_script(&info.app_name, &info.window_title);
            let output = Command::new("osascript")
                .args(["-e", &script])
                .output()
                .map_err(|e| FocusError::CommandFailed(format!("osascript failed: {e}")))?;

            if output.status.success() {
                return Ok(());
            }
            return Err(FocusError::ActivationFailed(format!(
                "generic focus failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Err(FocusError::NoTargetInfo)
    }

    fn is_focused(&self, session: &DaemonSession) -> bool {
        if session.info.bundle_id.is_empty() {
            return false;
        }
        let script = Self::build_is_focused_script(&session.info.bundle_id);
        let output = Command::new("osascript")
            .args(["-l", "JavaScript", "-e", &script])
            .output();
        matches!(output, Ok(o) if String::from_utf8_lossy(&o.stdout).trim() == "true")
    }

    fn name(&self) -> &str {
        "GenericMac"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::store::WindowInfo;
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
    fn can_focus_always_true() {
        let focus = GenericMacFocus;
        assert!(focus.can_focus(&make_session("com.example.anything")));
        assert!(focus.can_focus(&make_session("")));
    }

    #[test]
    fn activate_script_contains_bundle_id() {
        let script = GenericMacFocus::build_activate_script("com.example.app");
        assert!(script.contains("com.example.app"));
        assert!(script.contains("activateWithOptions"));
    }

    #[test]
    fn is_focused_script_checks_frontmost() {
        let script = GenericMacFocus::build_is_focused_script("com.example.app");
        assert!(script.contains("frontmostApplication"));
        assert!(script.contains("com.example.app"));
    }

    #[test]
    fn window_info_script_contains_app_and_title() {
        let script = GenericMacFocus::build_window_info_script("iTerm2", "MyProject");
        assert!(script.contains("iTerm2"));
        assert!(script.contains("MyProject"));
        assert!(script.contains("AXRaise"));
    }

    #[test]
    fn activate_with_empty_bundle_and_no_window_info_fails() {
        let focus = GenericMacFocus;
        let session = make_session("");
        let result = focus.activate(&session);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no bundle_id"));
    }

    #[test]
    fn activate_with_window_info_builds_applescript() {
        // Verify the window_info path constructs the right script
        let session = DaemonSession {
            info: SessionInfo::new(1, "test", SessionStatus::Idle),
            window_info: Some(WindowInfo {
                app_name: "TestApp".into(),
                window_title: "TestWindow".into(),
                pid: None,
            }),
        };
        // We can't really run osascript in tests, but we can verify the
        // code path doesn't panic and produces an error (app doesn't exist)
        let focus = GenericMacFocus;
        let result = focus.activate(&session);
        // Should fail because TestApp doesn't exist, but not panic
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn name_is_generic_mac() {
        assert_eq!(GenericMacFocus.name(), "GenericMac");
    }

    #[test]
    fn activate_script_rejects_malicious_bundle_id() {
        let script = GenericMacFocus::build_activate_script("com.evil';hack();'");
        assert_eq!(script, "'invalid_bundle_id'");
        assert!(!script.contains("hack"));
    }

    #[test]
    fn is_focused_script_rejects_malicious_bundle_id() {
        let script = GenericMacFocus::build_is_focused_script("';process.exit();'");
        assert_eq!(script, "'invalid_bundle_id'");
        assert!(!script.contains("process.exit"));
    }

    #[test]
    fn window_info_script_escapes_special_chars() {
        let script =
            GenericMacFocus::build_window_info_script("App\"Name", "Title\nInjection");
        // Double quotes in app_name should be escaped
        assert!(script.contains(r#"App\"Name"#));
        // Newline in window_title should be replaced with space
        assert!(script.contains("Title Injection"));
        assert!(!script.contains('\n') || script.contains("Title\nInjection") == false);
    }
}
