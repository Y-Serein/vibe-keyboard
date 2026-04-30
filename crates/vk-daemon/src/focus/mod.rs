//! Focus management — FocusStrategy trait + platform implementations.

use crate::session::store::DaemonSession;

pub mod error;
pub mod generic;
pub mod ghostty;
pub mod iterm;
pub mod macos;
pub mod vscode;
pub mod warp;

pub use error::FocusError;

/// Validate that a TTY path matches expected format
pub(crate) fn validate_tty(tty: &str) -> bool {
    // /dev/ttys000, /dev/tty000, /dev/pts/0
    tty.starts_with("/dev/")
        && tty.len() < 30
        && tty
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '/' || c == '.')
}

/// Validate that a bundle_id matches expected format (reverse DNS)
pub(crate) fn validate_bundle_id(bid: &str) -> bool {
    !bid.is_empty()
        && bid.len() < 256
        && bid
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

/// Escape a string for use inside JXA single-quoted strings
pub(crate) fn escape_jxa_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

/// Escape a string for use inside AppleScript double-quoted strings
pub(crate) fn escape_applescript_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ")
        .replace('\r', " ")
        .replace('\t', " ")
}

/// Strategy for focusing a terminal window associated with a session.
///
/// Implementations are ordered by specificity — more specific strategies
/// (e.g., iTerm2 with TTY-level tab switching) are tried before generic ones.
pub trait FocusStrategy: Send + Sync {
    /// Whether this strategy can handle the given session (e.g., bundle_id match).
    fn can_focus(&self, session: &DaemonSession) -> bool;

    /// Activate (bring to front) the window for the given session.
    fn activate(&self, session: &DaemonSession) -> Result<(), FocusError>;

    /// Check if the session's window is currently focused.
    fn is_focused(&self, session: &DaemonSession) -> bool;

    /// Human-readable name for logging/debugging.
    fn name(&self) -> &str;
}

/// Check if any focus strategy reports the session as focused.
///
/// Iterates strategies in priority order and returns the result from the first
/// strategy that can handle the session. Returns `false` if no strategy matches
/// (unknown terminal = assume not focused).
pub fn is_session_focused(strategies: &[Box<dyn FocusStrategy>], session: &DaemonSession) -> bool {
    for strategy in strategies {
        if strategy.can_focus(session) {
            return strategy.is_focused(session);
        }
    }
    false // unknown terminal = assume not focused
}

/// Spawn a shell command fully detached with 200ms delay.
/// Prevents Tauri from stealing focus back (SC's sc-focus.sh approach).
pub fn spawn_detached_focus(shell_cmd: &str) -> Result<(), FocusError> {
    let script = format!("sleep 0.2; {shell_cmd}");
    std::process::Command::new("bash")
        .args(["-c", &script])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| FocusError::SpawnFailed(e.to_string()))?;
    Ok(())
}

/// Iterate strategies in priority order and activate using the first matching one.
pub fn activate_with_strategies(
    strategies: &[Box<dyn FocusStrategy>],
    session: &DaemonSession,
) -> Result<(), FocusError> {
    for strategy in strategies {
        if strategy.can_focus(session) {
            return strategy.activate(session);
        }
    }
    Err(FocusError::NoStrategyMatched)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vk_protocol::message::{SessionInfo, SessionStatus};

    struct AlwaysFail;
    impl FocusStrategy for AlwaysFail {
        fn can_focus(&self, _: &DaemonSession) -> bool {
            false
        }
        fn activate(&self, _: &DaemonSession) -> Result<(), FocusError> {
            Err(FocusError::ActivationFailed("should not be called".into()))
        }
        fn is_focused(&self, _: &DaemonSession) -> bool {
            false
        }
        fn name(&self) -> &str {
            "AlwaysFail"
        }
    }

    struct AlwaysMatch;
    impl FocusStrategy for AlwaysMatch {
        fn can_focus(&self, _: &DaemonSession) -> bool {
            true
        }
        fn activate(&self, _: &DaemonSession) -> Result<(), FocusError> {
            Ok(())
        }
        fn is_focused(&self, _: &DaemonSession) -> bool {
            true
        }
        fn name(&self) -> &str {
            "AlwaysMatch"
        }
    }

    #[test]
    fn activate_picks_first_matching_strategy() {
        let strategies: Vec<Box<dyn FocusStrategy>> = vec![
            Box::new(AlwaysFail),
            Box::new(AlwaysMatch),
        ];
        let session = DaemonSession {
            info: SessionInfo::new(1, "test", SessionStatus::Idle),
            ..Default::default()
        };
        let result = activate_with_strategies(&strategies, &session);
        assert!(result.is_ok());
    }

    #[test]
    fn activate_returns_error_when_no_strategy_matches() {
        let strategies: Vec<Box<dyn FocusStrategy>> = vec![Box::new(AlwaysFail)];
        let session = DaemonSession {
            info: SessionInfo::new(1, "test", SessionStatus::Idle),
            ..Default::default()
        };
        let result = activate_with_strategies(&strategies, &session);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no focus strategy matched"));
    }

    /// A strategy that matches but reports NOT focused.
    struct MatchButNotFocused;
    impl FocusStrategy for MatchButNotFocused {
        fn can_focus(&self, _: &DaemonSession) -> bool {
            true
        }
        fn activate(&self, _: &DaemonSession) -> Result<(), FocusError> {
            Ok(())
        }
        fn is_focused(&self, _: &DaemonSession) -> bool {
            false
        }
        fn name(&self) -> &str {
            "MatchButNotFocused"
        }
    }

    #[test]
    fn is_session_focused_with_matching_focused_strategy() {
        let strategies: Vec<Box<dyn FocusStrategy>> = vec![
            Box::new(AlwaysFail),
            Box::new(AlwaysMatch),
        ];
        let session = DaemonSession {
            info: SessionInfo::new(1, "test", SessionStatus::Idle),
            ..Default::default()
        };
        assert!(is_session_focused(&strategies, &session));
    }

    #[test]
    fn is_session_focused_with_matching_unfocused_strategy() {
        let strategies: Vec<Box<dyn FocusStrategy>> = vec![
            Box::new(AlwaysFail),
            Box::new(MatchButNotFocused),
        ];
        let session = DaemonSession {
            info: SessionInfo::new(1, "test", SessionStatus::Idle),
            ..Default::default()
        };
        assert!(!is_session_focused(&strategies, &session));
    }

    #[test]
    fn is_session_focused_no_matching_strategy_returns_false() {
        let strategies: Vec<Box<dyn FocusStrategy>> = vec![Box::new(AlwaysFail)];
        let session = DaemonSession {
            info: SessionInfo::new(1, "test", SessionStatus::Idle),
            ..Default::default()
        };
        assert!(!is_session_focused(&strategies, &session));
    }

    #[test]
    fn is_session_focused_empty_strategies_returns_false() {
        let strategies: Vec<Box<dyn FocusStrategy>> = vec![];
        let session = DaemonSession {
            info: SessionInfo::new(1, "test", SessionStatus::Idle),
            ..Default::default()
        };
        assert!(!is_session_focused(&strategies, &session));
    }

    #[test]
    fn priority_order_iterm_before_generic() {
        let strategies = macos::default_strategies();
        // iTerm should come before Generic
        let names: Vec<&str> = strategies.iter().map(|s| s.name()).collect();
        let iterm_pos = names.iter().position(|n| *n == "iTerm2");
        let generic_pos = names.iter().position(|n| *n == "GenericMac");
        assert!(iterm_pos.is_some());
        assert!(generic_pos.is_some());
        assert!(iterm_pos.unwrap() < generic_pos.unwrap());
    }

    // --- validate_tty tests ---

    #[test]
    fn validate_tty_accepts_valid_paths() {
        assert!(validate_tty("/dev/ttys042"));
        assert!(validate_tty("/dev/tty000"));
        assert!(validate_tty("/dev/pts/0"));
    }

    #[test]
    fn validate_tty_rejects_invalid_paths() {
        assert!(!validate_tty(""));
        assert!(!validate_tty("not_a_tty"));
        assert!(!validate_tty("/dev/ttys042'; malicious; '"));
        assert!(!validate_tty("/dev/tty\ninjected"));
        assert!(!validate_tty("/tmp/evil"));
    }

    // --- validate_bundle_id tests ---

    #[test]
    fn validate_bundle_id_accepts_valid_ids() {
        assert!(validate_bundle_id("com.googlecode.iterm2"));
        assert!(validate_bundle_id("com.microsoft.VSCode"));
        assert!(validate_bundle_id("io.codeium.windsurf"));
        assert!(validate_bundle_id("com.todesktop.230313mzl4w4u92"));
    }

    #[test]
    fn validate_bundle_id_rejects_invalid_ids() {
        assert!(!validate_bundle_id(""));
        assert!(!validate_bundle_id("com.evil';malicious;'"));
        assert!(!validate_bundle_id("bundle with spaces"));
        assert!(!validate_bundle_id("bundle\ninjection"));
    }

    // --- escape_jxa_string tests ---

    #[test]
    fn escape_jxa_string_escapes_quotes_and_backslashes() {
        assert_eq!(escape_jxa_string("hello"), "hello");
        assert_eq!(escape_jxa_string("it's"), "it\\'s");
        assert_eq!(escape_jxa_string("back\\slash"), "back\\\\slash");
        assert_eq!(escape_jxa_string("a'b\\c"), "a\\'b\\\\c");
    }

    // --- escape_applescript_string tests ---

    #[test]
    fn escape_applescript_string_escapes_special_chars() {
        assert_eq!(escape_applescript_string("hello"), "hello");
        assert_eq!(escape_applescript_string(r#"say "hi""#), r#"say \"hi\""#);
        assert_eq!(escape_applescript_string("line\nnew"), "line new");
        assert_eq!(escape_applescript_string("tab\there"), "tab here");
        assert_eq!(escape_applescript_string("cr\rhere"), "cr here");
    }
}
