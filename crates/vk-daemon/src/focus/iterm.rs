//! iTerm2-specific focus strategy — JXA tab/session switching by TTY.

use crate::focus::{escape_jxa_string, validate_tty, FocusError, FocusStrategy};
use crate::session::store::DaemonSession;
use std::process::Command;

const BUNDLE_ID: &str = "com.googlecode.iterm2";

/// Focus strategy for iTerm2: can switch to specific tab/session by TTY.
pub struct ITermFocus;

impl ITermFocus {
    /// Build the JXA script that finds and activates the iTerm2 tab matching the given TTY.
    pub fn build_tty_script(tty: &str) -> String {
        if !validate_tty(tty) {
            return "'invalid_tty'".to_string();
        }
        let tty = escape_jxa_string(tty);
        format!(
            "var iTerm=Application('iTerm2');var ws=iTerm.windows();var f=0;\
             for(var w=0;w<ws.length&&!f;w++){{var ts=ws[w].tabs();\
             for(var t=0;t<ts.length&&!f;t++){{var ss=ts[t].sessions();\
             for(var s=0;s<ss.length&&!f;s++){{try{{if(ss[s].tty()==='{tty}')\
             {{ts[t].select();ss[s].select();ws[w].index=1;iTerm.activate();f=1}}}}catch(e){{}}}}}}}}",
        )
    }

    /// Build the JXA script that activates iTerm2 via NSRunningApplication.
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

    /// Build the JXA script to check if iTerm2's current tab matches the given TTY.
    pub fn build_is_focused_script(tty: &str) -> String {
        if !validate_tty(tty) {
            return "'invalid_tty'".to_string();
        }
        let tty = escape_jxa_string(tty);
        format!(
            "ObjC.import('Cocoa');\
             var front=$.NSWorkspace.sharedWorkspace.frontmostApplication;\
             var bid=front.bundleIdentifier.js.toLowerCase();\
             if(bid!=='{bundle}'.toLowerCase()){{'false'}}\
             else{{var iTerm=Application('iTerm2');\
             try{{var s=iTerm.currentWindow().currentTab().currentSession();\
             s.tty()==='{tty}'?'true':'false'}}catch(e){{'false'}}}}",
            bundle = BUNDLE_ID,
            tty = tty,
        )
    }
}

impl FocusStrategy for ITermFocus {
    fn can_focus(&self, session: &DaemonSession) -> bool {
        session.info.bundle_id.eq_ignore_ascii_case(BUNDLE_ID)
    }

    fn activate(&self, session: &DaemonSession) -> Result<(), FocusError> {
        // Build shell command for detached execution (prevents Tauri focus steal-back)
        let mut cmd = String::new();

        if !session.info.session_tty.is_empty() {
            let tty_jxa = Self::build_tty_script(&session.info.session_tty);
            cmd.push_str(&format!("osascript -l JavaScript -e '{}'; ", tty_jxa.replace('\'', "'\\''")));
        }

        let activate_jxa = Self::build_activate_script();
        cmd.push_str(&format!("osascript -l JavaScript -e '{}'", activate_jxa.replace('\'', "'\\''")));

        super::spawn_detached_focus(&cmd)
    }

    fn is_focused(&self, session: &DaemonSession) -> bool {
        if session.info.session_tty.is_empty() {
            // No TTY — just check if iTerm2 is frontmost
            let script = format!(
                "ObjC.import('Cocoa');\
                 var front=$.NSWorkspace.sharedWorkspace.frontmostApplication;\
                 front.bundleIdentifier.js.toLowerCase()==='{}'.toLowerCase()?'true':'false'",
                BUNDLE_ID
            );
            let output = Command::new("osascript")
                .args(["-l", "JavaScript", "-e", &script])
                .output();
            matches!(output, Ok(o) if String::from_utf8_lossy(&o.stdout).trim() == "true")
        } else {
            let script = Self::build_is_focused_script(&session.info.session_tty);
            let output = Command::new("osascript")
                .args(["-l", "JavaScript", "-e", &script])
                .output();
            matches!(output, Ok(o) if String::from_utf8_lossy(&o.stdout).trim() == "true")
        }
    }

    fn name(&self) -> &str {
        "iTerm2"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vk_protocol::message::{SessionInfo, SessionStatus};

    fn make_session(bundle_id: &str, tty: &str) -> DaemonSession {
        DaemonSession {
            info: SessionInfo {
                id: 1,
                name: "test".into(),
                status: SessionStatus::Idle,
                bundle_id: bundle_id.into(),
                session_tty: tty.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn can_focus_matches_iterm2_bundle_id() {
        let focus = ITermFocus;
        assert!(focus.can_focus(&make_session("com.googlecode.iterm2", "")));
        assert!(focus.can_focus(&make_session("com.googlecode.iTerm2", "")));
        assert!(focus.can_focus(&make_session("COM.GOOGLECODE.ITERM2", "")));
    }

    #[test]
    fn can_focus_rejects_other_bundle_ids() {
        let focus = ITermFocus;
        assert!(!focus.can_focus(&make_session("com.mitchellh.ghostty", "")));
        assert!(!focus.can_focus(&make_session("", "")));
    }

    #[test]
    fn tty_script_contains_tty_path() {
        let script = ITermFocus::build_tty_script("/dev/ttys042");
        assert!(script.contains("/dev/ttys042"));
        assert!(script.contains("iTerm2"));
        assert!(script.contains("select"));
    }

    #[test]
    fn activate_script_contains_bundle_id() {
        let script = ITermFocus::build_activate_script();
        assert!(script.contains(BUNDLE_ID));
        assert!(script.contains("activateWithOptions"));
    }

    #[test]
    fn is_focused_script_checks_tty() {
        let script = ITermFocus::build_is_focused_script("/dev/ttys042");
        assert!(script.contains("/dev/ttys042"));
        assert!(script.contains(BUNDLE_ID));
        assert!(script.contains("frontmostApplication"));
    }

    #[test]
    fn name_is_iterm2() {
        assert_eq!(ITermFocus.name(), "iTerm2");
    }

    #[test]
    fn tty_script_rejects_malicious_input() {
        let script = ITermFocus::build_tty_script("'; malicious; '");
        assert_eq!(script, "'invalid_tty'");
        assert!(!script.contains("malicious"));
    }

    #[test]
    fn is_focused_script_rejects_malicious_tty() {
        let script = ITermFocus::build_is_focused_script("/dev/ttys042';hack();'");
        assert_eq!(script, "'invalid_tty'");
        assert!(!script.contains("hack"));
    }
}
