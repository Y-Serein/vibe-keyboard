//! macOS native notification backend using terminal-notifier or osascript.

use std::process::Command;

use super::NotificationBackend;

/// macOS native notification backend.
///
/// Strategy 1: Try `terminal-notifier` (supports click-to-focus via `-activate`).
/// Strategy 2: Fallback to `osascript` with `display notification`.
pub struct MacNativeNotification;

impl NotificationBackend for MacNativeNotification {
    fn notify(
        &self,
        title: &str,
        body: &str,
        click_bundle_id: Option<&str>,
    ) -> Result<(), String> {
        // Strategy 1: terminal-notifier
        if try_terminal_notifier(title, body, click_bundle_id) {
            return Ok(());
        }

        // Strategy 2: osascript fallback
        try_osascript(title, body)
    }

    fn name(&self) -> &str {
        "mac-native"
    }
}

/// Try sending notification via terminal-notifier. Returns true on success.
fn try_terminal_notifier(title: &str, body: &str, click_bundle_id: Option<&str>) -> bool {
    let mut cmd = Command::new("terminal-notifier");
    cmd.args(["-title", title, "-message", body]);

    if let Some(bundle_id) = click_bundle_id {
        cmd.args(["-activate", bundle_id]);
    }

    match cmd.output() {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

fn escape_applescript(s: &str) -> String {
    s.replace('\\', "\\\\")
     .replace('"', "\\\"")
     .replace('\n', " ")
     .replace('\r', " ")
     .replace('\t', " ")
}

/// Fallback: send notification via osascript.
fn try_osascript(title: &str, body: &str) -> Result<(), String> {
    let escaped_title = escape_applescript(title);
    let escaped_body = escape_applescript(body);

    let script = format!(
        r#"display notification "{escaped_body}" with title "{escaped_title}""#
    );

    match Command::new("osascript").args(["-e", &script]).output() {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("osascript failed: {stderr}"))
        }
        Err(e) => Err(format!("failed to run osascript: {e}")),
    }
}
