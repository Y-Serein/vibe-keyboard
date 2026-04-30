//! Setup manager — tool detection, hook install/uninstall, brew package management.
//!
//! Used by the `/setup/*` API endpoints to manage AI tool integrations.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::info;

// ── Response types ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupStatus {
    pub ai_tools: Vec<AiToolStatus>,
    pub recommended: Vec<RecommendedTool>,
    pub system: SystemStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiToolStatus {
    pub name: String,
    pub id: String,
    pub installed: bool,
    pub hook_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendedTool {
    pub name: String,
    pub id: String,
    pub installed: bool,
    pub brew: String,
    pub purpose: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    pub accessibility: bool,
    pub daemon_running: bool,
    pub daemon_port: u16,
    pub device_connected: bool,
    pub transport: String,
}

// ── SetupManager ──

pub struct SetupManager;

impl SetupManager {
    /// Detect all tool statuses, recommended tools, and system state.
    pub async fn detect_all(daemon_port: u16, device_connected: bool) -> SetupStatus {
        let (ai_tools, recommended, accessibility) = tokio::join!(
            Self::detect_ai_tools(),
            Self::detect_recommended_tools(),
            Self::check_accessibility(),
        );

        let transport = if device_connected { "IPC" } else { "None" };

        SetupStatus {
            ai_tools,
            recommended,
            system: SystemStatus {
                accessibility,
                daemon_running: true, // We're responding, so we're running
                daemon_port,
                device_connected,
                transport: transport.to_string(),
            },
        }
    }

    /// Install hook for a given AI tool.
    pub async fn install_hook(tool_id: &str, port: u16) -> Result<(), String> {
        match tool_id {
            "claude-code" => {
                tokio::task::spawn_blocking(move || install_claude_code_hook(port))
                    .await
                    .map_err(|e| format!("spawn_blocking failed: {e}"))?
            }
            "cursor" => Err("Cursor hook installation not yet implemented".into()),
            "codex" => Err("Codex hook installation not yet implemented".into()),
            other => Err(format!("Unknown tool: {other}")),
        }
    }

    /// Uninstall hook for a given AI tool.
    pub async fn uninstall_hook(tool_id: &str) -> Result<(), String> {
        match tool_id {
            "claude-code" => {
                tokio::task::spawn_blocking(uninstall_claude_code_hook)
                    .await
                    .map_err(|e| format!("spawn_blocking failed: {e}"))?
            }
            "cursor" => Err("Cursor hook uninstallation not yet implemented".into()),
            "codex" => Err("Codex hook uninstallation not yet implemented".into()),
            other => Err(format!("Unknown tool: {other}")),
        }
    }

    /// Install a brew package.
    pub async fn brew_install(package: &str) -> Result<(), String> {
        let allowed = ["iterm2", "terminal-notifier"];
        if !allowed.contains(&package) {
            return Err(format!("Package not in allowed list: {package}"));
        }
        let package = package.to_string();
        tokio::task::spawn_blocking(move || {
            let output = std::process::Command::new("brew")
                .args(["install", &package])
                .output()
                .map_err(|e| format!("Failed to run brew: {e}"))?;
            if output.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(format!("brew install failed: {stderr}"))
            }
        })
        .await
        .map_err(|e| format!("spawn_blocking failed: {e}"))?
    }

    /// Uninstall a brew package.
    pub async fn brew_uninstall(package: &str) -> Result<(), String> {
        let allowed = ["iterm2", "terminal-notifier"];
        if !allowed.contains(&package) {
            return Err(format!("Package not in allowed list: {package}"));
        }
        let package = package.to_string();
        tokio::task::spawn_blocking(move || {
            let output = std::process::Command::new("brew")
                .args(["uninstall", &package])
                .output()
                .map_err(|e| format!("Failed to run brew: {e}"))?;
            if output.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(format!("brew uninstall failed: {stderr}"))
            }
        })
        .await
        .map_err(|e| format!("spawn_blocking failed: {e}"))?
    }

    // ── Private detection helpers ──

    async fn detect_ai_tools() -> Vec<AiToolStatus> {
        tokio::task::spawn_blocking(|| {
            vec![
                detect_claude_code(),
                detect_cursor(),
                detect_codex(),
            ]
        })
        .await
        .unwrap_or_default()
    }

    async fn detect_recommended_tools() -> Vec<RecommendedTool> {
        tokio::task::spawn_blocking(|| {
            vec![
                detect_iterm2(),
                detect_terminal_notifier(),
            ]
        })
        .await
        .unwrap_or_default()
    }

    async fn check_accessibility() -> bool {
        tokio::task::spawn_blocking(check_accessibility_permission)
            .await
            .unwrap_or(false)
    }
}

// ── Detection functions (sync, run in spawn_blocking) ──

fn home_dir() -> Option<PathBuf> {
    dirs::home_dir()
}

fn detect_claude_code() -> AiToolStatus {
    let installed = home_dir()
        .map(|h| h.join(".claude").exists())
        .unwrap_or(false);

    let hook_active = if installed {
        check_claude_code_hook_active()
    } else {
        false
    };

    AiToolStatus {
        name: "Claude Code".into(),
        id: "claude-code".into(),
        installed,
        hook_active,
    }
}

fn check_claude_code_hook_active() -> bool {
    let settings_path = match home_dir() {
        Some(h) => h.join(".claude").join("settings.json"),
        None => return false,
    };

    let content = match std::fs::read_to_string(&settings_path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return false,
    };

    // Check if hooks object exists and has at least one event type with our hook
    json.get("hooks")
        .and_then(|h| h.as_object())
        .map(|hooks| {
            hooks.values().any(|entries| {
                entries.as_array().map(|arr| {
                    arr.iter().any(|entry| {
                        entry.get("hooks")
                            .and_then(|h| h.as_array())
                            .map(|hs| {
                                hs.iter().any(|h| {
                                    h.get("command")
                                        .and_then(|c| c.as_str())
                                        .is_some_and(|c| c.contains("localhost:") && c.contains("/event"))
                                })
                            })
                            .unwrap_or(false)
                    })
                }).unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn detect_cursor() -> AiToolStatus {
    let dir_exists = home_dir()
        .map(|h| h.join(".cursor").exists())
        .unwrap_or(false);

    let cmd_exists = which_exists("cursor");
    let installed = dir_exists || cmd_exists;

    AiToolStatus {
        name: "Cursor".into(),
        id: "cursor".into(),
        installed,
        hook_active: false, // TODO: implement cursor hook detection
    }
}

fn detect_codex() -> AiToolStatus {
    let installed = which_exists("codex");

    AiToolStatus {
        name: "Codex".into(),
        id: "codex".into(),
        installed,
        hook_active: false, // TODO: implement codex hook detection
    }
}

fn detect_iterm2() -> RecommendedTool {
    let installed = std::path::Path::new("/Applications/iTerm.app").exists();
    RecommendedTool {
        name: "iTerm2".into(),
        id: "iterm2".into(),
        installed,
        brew: "iterm2".into(),
        purpose: "Tab-level focus switching".into(),
    }
}

fn detect_terminal_notifier() -> RecommendedTool {
    let installed = which_exists("terminal-notifier");
    RecommendedTool {
        name: "terminal-notifier".into(),
        id: "terminal-notifier".into(),
        installed,
        brew: "terminal-notifier".into(),
        purpose: "Click-to-jump notifications".into(),
    }
}

fn which_exists(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn check_accessibility_permission() -> bool {
    std::process::Command::new("osascript")
        .args(["-e", r#"tell application "System Events" to keystroke """#])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Write file atomically: write to temp → fsync → rename.
/// Prevents corruption from concurrent writes or interrupted I/O.
fn atomic_write_settings(path: &std::path::Path, content: &str) -> Result<(), String> {
    use std::io::Write;
    let tmp = path.with_extension("tmp");
    let mut f = std::fs::File::create(&tmp).map_err(|e| format!("create tmp: {e}"))?;
    f.write_all(content.as_bytes()).map_err(|e| format!("write tmp: {e}"))?;
    f.sync_all().map_err(|e| format!("fsync: {e}"))?;
    drop(f);
    std::fs::rename(&tmp, path).map_err(|e| format!("rename: {e}"))
}

// ── Hook install/uninstall (sync) ──

fn install_claude_code_hook(port: u16) -> Result<(), String> {
    let settings_path = home_dir()
        .map(|h| h.join(".claude").join("settings.json"))
        .ok_or_else(|| "Cannot determine home directory".to_string())?;

    info!("Installing vk-daemon hook for Claude Code at {}", settings_path.display());

    let mut settings: serde_json::Value = if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path)
            .map_err(|e| format!("Failed to read settings: {e}"))?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    let hooks = settings
        .as_object_mut()
        .ok_or("Settings is not a JSON object")?
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));

    let event_types = [
        "PreToolUse", "PostToolUse", "Notification",
        "SessionStart", "SessionEnd", "Stop",
        "UserPromptSubmit", "SubagentStart", "SubagentStop",
    ];

    for event_type in event_types {
        let hook_array = hooks
            .as_object_mut()
            .ok_or("hooks is not a JSON object")?
            .entry(event_type)
            .or_insert_with(|| serde_json::json!([]));

        // Remove existing vk hooks
        if let Some(arr) = hook_array.as_array_mut() {
            arr.retain(|entry| {
                let has_our_hook = entry
                    .get("hooks")
                    .and_then(|h| h.as_array())
                    .map(|hs| {
                        hs.iter().any(|h| {
                            h.get("command")
                                .and_then(|c| c.as_str())
                                .is_some_and(|c| c.contains(&format!("localhost:{port}")))
                        })
                    })
                    .unwrap_or(false);
                !has_our_hook
            });
        }

        let hook_cmd = format!(
            r#"VK_TTY=""; if [ -n "${{TMUX:-}}" ]; then VK_TTY=$(tmux display-message -p '#{{client_tty}}' 2>/dev/null); else P=$PPID; while [ "$P" -gt 1 ] 2>/dev/null; do T=$(ps -p "$P" -o tty= 2>/dev/null | tr -d ' '); [ -n "$T" ] && [ "$T" != "??" ] && VK_TTY="/dev/$T"; P=$(ps -p "$P" -o ppid= 2>/dev/null | tr -d ' '); done; fi; export VK_TTY; jq -c '. + {{"type":"{event_type}","source":"claude-code","cwd":env.PWD,"bundle_id":(env.TERM_PROGRAM // ""),"session_tty":(env.VK_TTY // "")}}' | curl -s -X POST http://localhost:{port}/event -H 'Content-Type: application/json' -d @-"#,
        );

        let hook_entry = serde_json::json!({
            "matcher": "",
            "hooks": [{
                "type": "command",
                "command": hook_cmd,
                "timeout": 5000
            }]
        });
        hook_array
            .as_array_mut()
            .ok_or("hook array is not an array")?
            .push(hook_entry);
    }

    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {e}"))?;
    }
    let json = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize: {e}"))?;
    atomic_write_settings(&settings_path, &json)?;

    info!("Claude Code hook installed successfully");
    Ok(())
}

fn uninstall_claude_code_hook() -> Result<(), String> {
    let settings_path = home_dir()
        .map(|h| h.join(".claude").join("settings.json"))
        .ok_or_else(|| "Cannot determine home directory".to_string())?;

    if !settings_path.exists() {
        return Ok(()); // Nothing to uninstall
    }

    let content = std::fs::read_to_string(&settings_path)
        .map_err(|e| format!("Failed to read settings: {e}"))?;
    let mut settings: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse settings: {e}"))?;

    let hooks = match settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        Some(h) => h,
        None => return Ok(()), // No hooks section
    };

    // Remove vk-daemon hooks from all event types
    for (_event_type, entries) in hooks.iter_mut() {
        if let Some(arr) = entries.as_array_mut() {
            arr.retain(|entry| {
                let has_our_hook = entry
                    .get("hooks")
                    .and_then(|h| h.as_array())
                    .map(|hs| {
                        hs.iter().any(|h| {
                            h.get("command")
                                .and_then(|c| c.as_str())
                                .is_some_and(|c| c.contains("localhost:") && c.contains("/event"))
                        })
                    })
                    .unwrap_or(false);
                !has_our_hook
            });
        }
    }

    let json = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize: {e}"))?;
    atomic_write_settings(&settings_path, &json)?;

    info!("Claude Code hook uninstalled successfully");
    Ok(())
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_claude_code_returns_valid_status() {
        let status = detect_claude_code();
        assert_eq!(status.id, "claude-code");
        assert_eq!(status.name, "Claude Code");
        // installed depends on the test environment, just check it doesn't panic
    }

    #[test]
    fn detect_cursor_returns_valid_status() {
        let status = detect_cursor();
        assert_eq!(status.id, "cursor");
        assert_eq!(status.name, "Cursor");
    }

    #[test]
    fn detect_codex_returns_valid_status() {
        let status = detect_codex();
        assert_eq!(status.id, "codex");
        assert_eq!(status.name, "Codex");
    }

    #[test]
    fn detect_iterm2_returns_valid_status() {
        let tool = detect_iterm2();
        assert_eq!(tool.id, "iterm2");
        assert_eq!(tool.brew, "iterm2");
    }

    #[test]
    fn detect_terminal_notifier_returns_valid_status() {
        let tool = detect_terminal_notifier();
        assert_eq!(tool.id, "terminal-notifier");
        assert_eq!(tool.brew, "terminal-notifier");
    }

    #[test]
    fn which_exists_detects_known_command() {
        // `ls` should always exist
        assert!(which_exists("ls"));
        // random garbage should not
        assert!(!which_exists("this-command-should-not-exist-xyz123"));
    }

    #[test]
    fn hook_detection_with_valid_settings() {
        let dir = tempfile::tempdir().unwrap();
        let claude_dir = dir.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();

        let settings = serde_json::json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "",
                    "hooks": [{
                        "type": "command",
                        "command": "curl -s -X POST http://localhost:19280/event -d @-",
                        "timeout": 5000
                    }]
                }]
            }
        });
        std::fs::write(
            claude_dir.join("settings.json"),
            serde_json::to_string_pretty(&settings).unwrap(),
        )
        .unwrap();

        // We can't easily override home_dir, but we can test the hook parsing logic directly
        let content = std::fs::read_to_string(claude_dir.join("settings.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        let has_hook = json
            .get("hooks")
            .and_then(|h| h.as_object())
            .map(|hooks| {
                hooks.values().any(|entries| {
                    entries
                        .as_array()
                        .map(|arr| {
                            arr.iter().any(|entry| {
                                entry
                                    .get("hooks")
                                    .and_then(|h| h.as_array())
                                    .map(|hs| {
                                        hs.iter().any(|h| {
                                            h.get("command")
                                                .and_then(|c| c.as_str())
                                                .is_some_and(|c| {
                                                    c.contains("localhost:") && c.contains("/event")
                                                })
                                        })
                                    })
                                    .unwrap_or(false)
                            })
                        })
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);

        assert!(has_hook, "Should detect vk-daemon hook in settings.json");
    }

    #[test]
    fn hook_detection_without_hooks() {
        let settings = serde_json::json!({});
        let has_hook = settings
            .get("hooks")
            .and_then(|h| h.as_object())
            .map(|hooks| {
                hooks.values().any(|entries| {
                    entries
                        .as_array()
                        .map(|arr| !arr.is_empty())
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);

        assert!(!has_hook, "Empty settings should not have hooks");
    }

    #[tokio::test]
    async fn detect_all_returns_complete_status() {
        let status = SetupManager::detect_all(19280, false).await;

        assert_eq!(status.ai_tools.len(), 3);
        assert_eq!(status.recommended.len(), 2);
        assert!(status.system.daemon_running);
        assert_eq!(status.system.daemon_port, 19280);
        assert!(!status.system.device_connected);
        assert_eq!(status.system.transport, "None");

        // Verify tool IDs
        let tool_ids: Vec<&str> = status.ai_tools.iter().map(|t| t.id.as_str()).collect();
        assert!(tool_ids.contains(&"claude-code"));
        assert!(tool_ids.contains(&"cursor"));
        assert!(tool_ids.contains(&"codex"));

        let rec_ids: Vec<&str> = status.recommended.iter().map(|t| t.id.as_str()).collect();
        assert!(rec_ids.contains(&"iterm2"));
        assert!(rec_ids.contains(&"terminal-notifier"));
    }

    #[tokio::test]
    async fn install_hook_unknown_tool_returns_error() {
        let result = SetupManager::install_hook("unknown-tool", 19280).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown tool"));
    }

    #[tokio::test]
    async fn uninstall_hook_unknown_tool_returns_error() {
        let result = SetupManager::uninstall_hook("unknown-tool").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown tool"));
    }

    #[tokio::test]
    async fn brew_install_disallowed_package_returns_error() {
        let result = SetupManager::brew_install("malicious-package").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not in allowed list"));
    }

    #[tokio::test]
    async fn brew_uninstall_disallowed_package_returns_error() {
        let result = SetupManager::brew_uninstall("malicious-package").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not in allowed list"));
    }
}
