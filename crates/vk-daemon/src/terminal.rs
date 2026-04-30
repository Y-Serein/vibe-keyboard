//! Terminal detection — identifies which terminal emulator is running.
//!
//! Uses environment variables (primarily `TERM_PROGRAM`) to detect the terminal,
//! with fallback logic for tmux/screen and vscode-family editors.

use std::collections::HashMap;

/// Terminal type enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalType {
    ITerm2,
    Ghostty,
    Warp,
    VsCode,
    Cursor,
    Windsurf,
    WezTerm,
    Zed,
    AppleTerminal,
    Unknown,
}

/// Detection result.
#[derive(Debug, Clone)]
pub struct TerminalInfo {
    pub terminal_type: TerminalType,
    pub bundle_id: String,
    pub session_tty: String,
}

/// Trait for terminal detection.
pub trait TerminalDetector: Send + Sync {
    fn detect(&self, env: &HashMap<String, String>) -> TerminalInfo;
    fn name(&self) -> &str;
}

/// macOS terminal detector — checks TERM_PROGRAM and fallback env vars.
pub struct MacTerminalDetector;

impl MacTerminalDetector {
    /// Resolve vscode-family IDE from `__CFBundleIdentifier`.
    fn detect_vscode_variant(env: &HashMap<String, String>) -> (TerminalType, &'static str) {
        if let Some(bundle) = env.get("__CFBundleIdentifier") {
            let lower = bundle.to_lowercase();
            if lower.contains("cursor") || lower.contains("todesktop") {
                return (TerminalType::Cursor, "com.todesktop.230313mzl4w4u92");
            }
            if lower.contains("windsurf") {
                return (TerminalType::Windsurf, "com.codeium.windsurf");
            }
        }
        (TerminalType::VsCode, "com.microsoft.VSCode")
    }

    /// Check fallback env vars for multiplexers (tmux, screen).
    fn detect_under_multiplexer(env: &HashMap<String, String>) -> Option<(TerminalType, &'static str)> {
        if env.contains_key("ITERM_SESSION_ID") {
            return Some((TerminalType::ITerm2, "com.googlecode.iterm2"));
        }
        if env.contains_key("GHOSTTY_RESOURCES_DIR") {
            return Some((TerminalType::Ghostty, "com.mitchellh.ghostty"));
        }
        None
    }

    /// Extract session TTY from env.
    fn extract_tty(env: &HashMap<String, String>) -> String {
        env.get("PEON_SESSION_TTY")
            .or_else(|| env.get("TTY"))
            .cloned()
            .unwrap_or_default()
    }
}

impl TerminalDetector for MacTerminalDetector {
    fn detect(&self, env: &HashMap<String, String>) -> TerminalInfo {
        let tty = Self::extract_tty(env);

        let (terminal_type, bundle_id) = match env.get("TERM_PROGRAM").map(|s| s.as_str()) {
            Some("iTerm.app") => (TerminalType::ITerm2, "com.googlecode.iterm2"),
            Some("ghostty") => (TerminalType::Ghostty, "com.mitchellh.ghostty"),
            Some("WarpTerminal") => (TerminalType::Warp, "dev.warp.Warp-Stable"),
            Some("Apple_Terminal") => (TerminalType::AppleTerminal, "com.apple.Terminal"),
            Some("WezTerm") => (TerminalType::WezTerm, "com.github.wez.wezterm"),
            Some("zed") => (TerminalType::Zed, "dev.zed.Zed"),
            Some("vscode") => {
                let (tt, bid) = Self::detect_vscode_variant(env);
                (tt, bid)
            }
            Some("tmux") | Some("screen") => {
                Self::detect_under_multiplexer(env)
                    .unwrap_or((TerminalType::Unknown, "com.googlecode.iterm2"))
            }
            Some(_) => (TerminalType::Unknown, "com.googlecode.iterm2"),
            None => (TerminalType::Unknown, "com.googlecode.iterm2"),
        };

        TerminalInfo {
            terminal_type,
            bundle_id: bundle_id.to_string(),
            session_tty: tty,
        }
    }

    fn name(&self) -> &str {
        "MacTerminalDetector"
    }
}

/// Create the default terminal detector for the current platform.
pub fn default_detector() -> Box<dyn TerminalDetector> {
    Box::new(MacTerminalDetector)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_with(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn detect_iterm2() {
        let detector = MacTerminalDetector;
        let info = detector.detect(&env_with(&[("TERM_PROGRAM", "iTerm.app")]));
        assert_eq!(info.terminal_type, TerminalType::ITerm2);
        assert_eq!(info.bundle_id, "com.googlecode.iterm2");
    }

    #[test]
    fn detect_ghostty() {
        let detector = MacTerminalDetector;
        let info = detector.detect(&env_with(&[("TERM_PROGRAM", "ghostty")]));
        assert_eq!(info.terminal_type, TerminalType::Ghostty);
        assert_eq!(info.bundle_id, "com.mitchellh.ghostty");
    }

    #[test]
    fn detect_warp() {
        let detector = MacTerminalDetector;
        let info = detector.detect(&env_with(&[("TERM_PROGRAM", "WarpTerminal")]));
        assert_eq!(info.terminal_type, TerminalType::Warp);
        assert_eq!(info.bundle_id, "dev.warp.Warp-Stable");
    }

    #[test]
    fn detect_vscode() {
        let detector = MacTerminalDetector;
        let info = detector.detect(&env_with(&[("TERM_PROGRAM", "vscode")]));
        assert_eq!(info.terminal_type, TerminalType::VsCode);
        assert_eq!(info.bundle_id, "com.microsoft.VSCode");
    }

    #[test]
    fn detect_cursor_via_bundle() {
        let detector = MacTerminalDetector;
        let info = detector.detect(&env_with(&[
            ("TERM_PROGRAM", "vscode"),
            ("__CFBundleIdentifier", "com.todesktop.230313mzl4w4u92"),
        ]));
        assert_eq!(info.terminal_type, TerminalType::Cursor);
        assert!(info.bundle_id.contains("cursor") || info.bundle_id.contains("todesktop"));
    }

    #[test]
    fn detect_windsurf_via_bundle() {
        let detector = MacTerminalDetector;
        let info = detector.detect(&env_with(&[
            ("TERM_PROGRAM", "vscode"),
            ("__CFBundleIdentifier", "com.codeium.windsurf"),
        ]));
        assert_eq!(info.terminal_type, TerminalType::Windsurf);
        assert_eq!(info.bundle_id, "com.codeium.windsurf");
    }

    #[test]
    fn detect_wezterm() {
        let detector = MacTerminalDetector;
        let info = detector.detect(&env_with(&[("TERM_PROGRAM", "WezTerm")]));
        assert_eq!(info.terminal_type, TerminalType::WezTerm);
        assert_eq!(info.bundle_id, "com.github.wez.wezterm");
    }

    #[test]
    fn detect_zed() {
        let detector = MacTerminalDetector;
        let info = detector.detect(&env_with(&[("TERM_PROGRAM", "zed")]));
        assert_eq!(info.terminal_type, TerminalType::Zed);
        assert_eq!(info.bundle_id, "dev.zed.Zed");
    }

    #[test]
    fn detect_apple_terminal() {
        let detector = MacTerminalDetector;
        let info = detector.detect(&env_with(&[("TERM_PROGRAM", "Apple_Terminal")]));
        assert_eq!(info.terminal_type, TerminalType::AppleTerminal);
        assert_eq!(info.bundle_id, "com.apple.Terminal");
    }

    #[test]
    fn detect_tmux_with_iterm_fallback() {
        let detector = MacTerminalDetector;
        let info = detector.detect(&env_with(&[
            ("TERM_PROGRAM", "tmux"),
            ("ITERM_SESSION_ID", "w0t0p0:abc"),
        ]));
        assert_eq!(info.terminal_type, TerminalType::ITerm2);
        assert_eq!(info.bundle_id, "com.googlecode.iterm2");
    }

    #[test]
    fn detect_tmux_with_ghostty_fallback() {
        let detector = MacTerminalDetector;
        let info = detector.detect(&env_with(&[
            ("TERM_PROGRAM", "tmux"),
            ("GHOSTTY_RESOURCES_DIR", "/some/path"),
        ]));
        assert_eq!(info.terminal_type, TerminalType::Ghostty);
        assert_eq!(info.bundle_id, "com.mitchellh.ghostty");
    }

    #[test]
    fn detect_empty_env_returns_unknown() {
        let detector = MacTerminalDetector;
        let info = detector.detect(&HashMap::new());
        assert_eq!(info.terminal_type, TerminalType::Unknown);
        assert_eq!(info.bundle_id, "com.googlecode.iterm2");
    }

    #[test]
    fn detect_unknown_term_program() {
        let detector = MacTerminalDetector;
        let info = detector.detect(&env_with(&[("TERM_PROGRAM", "some-unknown-term")]));
        assert_eq!(info.terminal_type, TerminalType::Unknown);
        assert_eq!(info.bundle_id, "com.googlecode.iterm2");
    }

    #[test]
    fn session_tty_from_env() {
        let detector = MacTerminalDetector;
        let info = detector.detect(&env_with(&[
            ("TERM_PROGRAM", "ghostty"),
            ("PEON_SESSION_TTY", "/dev/ttys003"),
        ]));
        assert_eq!(info.session_tty, "/dev/ttys003");
    }

    #[test]
    fn session_tty_fallback_to_tty() {
        let detector = MacTerminalDetector;
        let info = detector.detect(&env_with(&[
            ("TERM_PROGRAM", "ghostty"),
            ("TTY", "/dev/ttys005"),
        ]));
        assert_eq!(info.session_tty, "/dev/ttys005");
    }

    #[test]
    fn default_detector_name() {
        let d = default_detector();
        assert_eq!(d.name(), "MacTerminalDetector");
    }

    #[test]
    fn bundle_id_correctness() {
        let detector = MacTerminalDetector;
        let cases: Vec<(&[(&str, &str)], &str)> = vec![
            (&[("TERM_PROGRAM", "iTerm.app")], "com.googlecode.iterm2"),
            (&[("TERM_PROGRAM", "ghostty")], "com.mitchellh.ghostty"),
            (&[("TERM_PROGRAM", "WarpTerminal")], "dev.warp.Warp-Stable"),
            (&[("TERM_PROGRAM", "Apple_Terminal")], "com.apple.Terminal"),
            (&[("TERM_PROGRAM", "WezTerm")], "com.github.wez.wezterm"),
            (&[("TERM_PROGRAM", "zed")], "dev.zed.Zed"),
            (&[("TERM_PROGRAM", "vscode")], "com.microsoft.VSCode"),
        ];
        for (env_pairs, expected_bundle) in cases {
            let info = detector.detect(&env_with(env_pairs));
            assert_eq!(info.bundle_id, expected_bundle, "failed for {:?}", env_pairs);
        }
    }
}
