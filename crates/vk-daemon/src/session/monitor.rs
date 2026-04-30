//! SessionMonitor trait — abstraction for AI tool session detection.
//!
//! Each AI tool (Claude Code, Cursor, Codex) has its own adapter.

use vk_protocol::message::SessionStatus;

/// Events emitted by a session monitor.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionEvent {
    /// New session detected.
    Started {
        session_id: String,
        name: String,
        source: String,
        cwd: String,
        bundle_id: String,
        session_tty: String,
        transcript_path: String,
    },
    /// Session ended.
    Ended {
        session_id: String,
    },
    /// Session status changed.
    StatusChanged {
        session_id: String,
        status: SessionStatus,
    },
    /// Permission request from session.
    PermissionRequest {
        session_id: String,
        tool_name: String,
        tool_input: String,
    },
}

/// Hook event payload (JSON from Claude Code hook POST).
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq, Default)]
pub struct HookEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub tool_name: String,
    #[serde(default)]
    pub tool_input: String,

    // ── Extended fields (from hook script env injection) ──
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub permission_mode: String,
    #[serde(default)]
    pub transcript_path: String,
    #[serde(default)]
    pub bundle_id: String,
    #[serde(default)]
    pub session_tty: String,
    #[serde(default)]
    pub error: String,
}

/// Parse a hook event into a SessionEvent.
/// Handles both Claude Code native events and SC-compatible events.
pub fn parse_hook_event(event: &HookEvent) -> Option<SessionEvent> {
    let ev = event.event_type.as_str();
    match ev {
        // Session lifecycle
        "SessionStart" | "session_start" | "init" => {
            let name = if event.name.is_empty() {
                if !event.cwd.is_empty() {
                    std::path::Path::new(&event.cwd)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| format!("S-{}", &event.session_id[..8.min(event.session_id.len())]))
                } else {
                    format!("S-{}", &event.session_id[..8.min(event.session_id.len())])
                }
            } else {
                event.name.clone()
            };
            Some(SessionEvent::Started {
                session_id: event.session_id.clone(),
                name,
                source: event.source.clone(),
                cwd: event.cwd.clone(),
                bundle_id: event.bundle_id.clone(),
                session_tty: event.session_tty.clone(),
                transcript_path: event.transcript_path.clone(),
            })
        }
        "SessionEnd" | "session_end" | "exit" => Some(SessionEvent::Ended {
            session_id: event.session_id.clone(),
        }),

        // Status updates — Claude Code hook event types (SC mapping)
        "PreToolUse" => Some(SessionEvent::StatusChanged {
            session_id: event.session_id.clone(),
            status: SessionStatus::ToolUse,
        }),
        "PostToolUse" => Some(SessionEvent::StatusChanged {
            session_id: event.session_id.clone(),
            status: SessionStatus::Writing,
        }),
        "Notification" => {
            // Check if this is a permission request
            if event.tool_name.contains("permission") || !event.tool_input.is_empty() {
                Some(SessionEvent::PermissionRequest {
                    session_id: event.session_id.clone(),
                    tool_name: event.tool_name.clone(),
                    tool_input: event.tool_input.clone(),
                })
            } else {
                Some(SessionEvent::StatusChanged {
                    session_id: event.session_id.clone(),
                    status: SessionStatus::Thinking,
                })
            }
        }
        "UserPromptSubmit" => Some(SessionEvent::StatusChanged {
            session_id: event.session_id.clone(),
            status: SessionStatus::Thinking,
        }),
        "Stop" => Some(SessionEvent::StatusChanged {
            session_id: event.session_id.clone(),
            status: SessionStatus::Done,
        }),
        "SubagentStart" => Some(SessionEvent::StatusChanged {
            session_id: event.session_id.clone(),
            status: SessionStatus::Thinking,
        }),
        "SubagentStop" => Some(SessionEvent::StatusChanged {
            session_id: event.session_id.clone(),
            status: SessionStatus::Writing,
        }),

        // Legacy/generic
        "status" | "tool_use" | "message" => {
            let status = if ev == "tool_use" {
                Some(SessionStatus::ToolUse)
            } else {
                parse_status(&event.status)
            };
            status.map(|s| SessionEvent::StatusChanged {
                session_id: event.session_id.clone(),
                status: s,
            })
        }
        "permission" | "permission_request" => Some(SessionEvent::PermissionRequest {
            session_id: event.session_id.clone(),
            tool_name: event.tool_name.clone(),
            tool_input: event.tool_input.clone(),
        }),
        _ => None,
    }
}

fn parse_status(s: &str) -> Option<SessionStatus> {
    match s {
        "thinking" => Some(SessionStatus::Thinking),
        "tool_use" => Some(SessionStatus::ToolUse),
        "writing" => Some(SessionStatus::Writing),
        "done" => Some(SessionStatus::Done),
        "error" => Some(SessionStatus::Error),
        "idle" => Some(SessionStatus::Idle),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_session_start() {
        let event = HookEvent {
            event_type: "session_start".into(),
            session_id: "abc12345".into(),
            name: "RustAgent".into(),
            ..default_hook()
        };
        let parsed = parse_hook_event(&event).unwrap();
        match parsed {
            SessionEvent::Started { session_id, name, .. } => {
                assert_eq!(session_id, "abc12345");
                assert_eq!(name, "RustAgent");
            }
            _ => panic!("expected Started"),
        }
    }

    #[test]
    fn parse_session_start_no_name() {
        let event = HookEvent {
            event_type: "session_start".into(),
            session_id: "abc12345".into(),
            name: "".into(),
            ..default_hook()
        };
        let parsed = parse_hook_event(&event).unwrap();
        if let SessionEvent::Started { name, .. } = parsed {
            assert!(name.starts_with("S-"));
        } else {
            panic!("expected Started");
        }
    }

    #[test]
    fn parse_session_end() {
        let event = HookEvent {
            event_type: "session_end".into(),
            session_id: "abc12345".into(),
            ..default_hook()
        };
        let parsed = parse_hook_event(&event).unwrap();
        assert_eq!(
            parsed,
            SessionEvent::Ended {
                session_id: "abc12345".into(),
            }
        );
    }

    #[test]
    fn parse_status_change() {
        let event = HookEvent {
            event_type: "status".into(),
            session_id: "abc".into(),
            status: "thinking".into(),
            ..default_hook()
        };
        let parsed = parse_hook_event(&event).unwrap();
        assert_eq!(
            parsed,
            SessionEvent::StatusChanged {
                session_id: "abc".into(),
                status: SessionStatus::Thinking,
            }
        );
    }

    #[test]
    fn parse_permission_request() {
        let event = HookEvent {
            event_type: "permission".into(),
            session_id: "abc".into(),
            tool_name: "Write".into(),
            tool_input: "main.rs".into(),
            ..default_hook()
        };
        let parsed = parse_hook_event(&event).unwrap();
        assert_eq!(
            parsed,
            SessionEvent::PermissionRequest {
                session_id: "abc".into(),
                tool_name: "Write".into(),
                tool_input: "main.rs".into(),
            }
        );
    }

    #[test]
    fn parse_unknown_type_returns_none() {
        let event = HookEvent {
            event_type: "unknown".into(),
            ..default_hook()
        };
        assert!(parse_hook_event(&event).is_none());
    }

    #[test]
    fn parse_invalid_status_returns_none() {
        let event = HookEvent {
            event_type: "status".into(),
            status: "invalid".into(),
            ..default_hook()
        };
        assert!(parse_hook_event(&event).is_none());
    }

    #[test]
    fn hook_event_serde_roundtrip() {
        let event = HookEvent {
            event_type: "session_start".into(),
            session_id: "test123".into(),
            name: "TestSession".into(),
            source: "claude-code".into(),
            cwd: "/home/user/project".into(),
            ..Default::default()
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: HookEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, event);
    }

    #[test]
    fn hook_event_deserializes_extended_fields() {
        let json = r#"{"type":"session_start","session_id":"abc","source":"claude-code","cwd":"/tmp/proj","bundle_id":"com.googlecode.iterm2","session_tty":"/dev/ttys001","transcript_path":"/home/.claude/sessions/abc/transcript.jsonl"}"#;
        let event: HookEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.source, "claude-code");
        assert_eq!(event.cwd, "/tmp/proj");
        assert_eq!(event.bundle_id, "com.googlecode.iterm2");
        assert_eq!(event.session_tty, "/dev/ttys001");
        assert_eq!(event.transcript_path, "/home/.claude/sessions/abc/transcript.jsonl");
    }

    #[test]
    fn parse_session_start_uses_cwd_dir_name_when_no_name() {
        let event = HookEvent {
            event_type: "session_start".into(),
            session_id: "abc12345".into(),
            cwd: "/home/user/my-project".into(),
            ..default_hook()
        };
        let parsed = parse_hook_event(&event).unwrap();
        if let SessionEvent::Started { name, cwd, .. } = parsed {
            assert_eq!(name, "my-project");
            assert_eq!(cwd, "/home/user/my-project");
        } else {
            panic!("expected Started");
        }
    }

    #[test]
    fn parse_tool_use_maps_to_status() {
        let event = HookEvent {
            event_type: "tool_use".into(),
            session_id: "abc".into(),
            ..default_hook()
        };
        let parsed = parse_hook_event(&event).unwrap();
        assert_eq!(
            parsed,
            SessionEvent::StatusChanged {
                session_id: "abc".into(),
                status: SessionStatus::ToolUse,
            }
        );
    }

    #[test]
    fn parse_permission_request_alias() {
        let event = HookEvent {
            event_type: "permission_request".into(),
            session_id: "abc".into(),
            tool_name: "Edit".into(),
            tool_input: "file.rs".into(),
            ..default_hook()
        };
        let parsed = parse_hook_event(&event).unwrap();
        assert!(matches!(parsed, SessionEvent::PermissionRequest { .. }));
    }

    #[test]
    fn parse_init_alias() {
        let event = HookEvent {
            event_type: "init".into(),
            session_id: "abc12345".into(),
            name: "Test".into(),
            ..default_hook()
        };
        assert!(matches!(parse_hook_event(&event), Some(SessionEvent::Started { .. })));
    }

    #[test]
    fn parse_exit_alias() {
        let event = HookEvent {
            event_type: "exit".into(),
            session_id: "abc".into(),
            ..default_hook()
        };
        assert!(matches!(parse_hook_event(&event), Some(SessionEvent::Ended { .. })));
    }

    fn default_hook() -> HookEvent {
        HookEvent::default()
    }
}
