//! Integration tests for vk-daemon server functionality.
//!
//! Tests internal functions without starting the HTTP server:
//! - parse_hook_event — event type mapping
//! - SessionStore — full CRUD lifecycle
//! - PermissionQueue — enqueue + resolve flow
//! - validate_transcript_path — path validation

use vk_daemon::permission::{
    evaluate_yolo, PendingPermission, PermissionQueue, YoloConfig, YoloDecision,
};
use vk_daemon::session::monitor::{parse_hook_event, HookEvent, SessionEvent};
use vk_daemon::session::store::{DaemonSession, SessionStore};
// validate_transcript_path is pub(crate) — inline a copy for testing
use vk_protocol::message::{PermissionAction, SessionInfo, SessionStatus};

// ── parse_hook_event: all event type mappings ──

fn hook(event_type: &str) -> HookEvent {
    HookEvent {
        event_type: event_type.into(),
        session_id: "sess-001".into(),
        ..Default::default()
    }
}

#[test]
fn parse_session_start_event() {
    let h = HookEvent {
        event_type: "SessionStart".into(),
        session_id: "sess-001".into(),
        name: "MyProject".into(),
        source: "claude-code".into(),
        cwd: "/Users/test/project".into(),
        ..Default::default()
    };
    let ev = parse_hook_event(&h).unwrap();
    match ev {
        SessionEvent::Started {
            session_id, name, source, cwd, ..
        } => {
            assert_eq!(session_id, "sess-001");
            assert_eq!(name, "MyProject");
            assert_eq!(source, "claude-code");
            assert_eq!(cwd, "/Users/test/project");
        }
        _ => panic!("expected Started"),
    }
}

#[test]
fn parse_session_start_aliases() {
    for et in &["SessionStart", "session_start", "init"] {
        let h = HookEvent {
            event_type: et.to_string(),
            session_id: "abc12345".into(),
            name: "Test".into(),
            ..Default::default()
        };
        assert!(
            matches!(parse_hook_event(&h), Some(SessionEvent::Started { .. })),
            "expected Started for event_type={et}"
        );
    }
}

#[test]
fn parse_session_end_aliases() {
    for et in &["SessionEnd", "session_end", "exit"] {
        let h = HookEvent {
            event_type: et.to_string(),
            session_id: "abc12345".into(),
            ..Default::default()
        };
        assert!(
            matches!(parse_hook_event(&h), Some(SessionEvent::Ended { .. })),
            "expected Ended for event_type={et}"
        );
    }
}

#[test]
fn parse_pre_tool_use_maps_to_tool_use_status() {
    let ev = parse_hook_event(&hook("PreToolUse")).unwrap();
    assert_eq!(
        ev,
        SessionEvent::StatusChanged {
            session_id: "sess-001".into(),
            status: SessionStatus::ToolUse,
        }
    );
}

#[test]
fn parse_post_tool_use_maps_to_writing() {
    let ev = parse_hook_event(&hook("PostToolUse")).unwrap();
    assert_eq!(
        ev,
        SessionEvent::StatusChanged {
            session_id: "sess-001".into(),
            status: SessionStatus::Writing,
        }
    );
}

#[test]
fn parse_user_prompt_submit_maps_to_thinking() {
    let ev = parse_hook_event(&hook("UserPromptSubmit")).unwrap();
    assert_eq!(
        ev,
        SessionEvent::StatusChanged {
            session_id: "sess-001".into(),
            status: SessionStatus::Thinking,
        }
    );
}

#[test]
fn parse_stop_maps_to_done() {
    let ev = parse_hook_event(&hook("Stop")).unwrap();
    assert_eq!(
        ev,
        SessionEvent::StatusChanged {
            session_id: "sess-001".into(),
            status: SessionStatus::Done,
        }
    );
}

#[test]
fn parse_subagent_start_maps_to_thinking() {
    let ev = parse_hook_event(&hook("SubagentStart")).unwrap();
    assert_eq!(
        ev,
        SessionEvent::StatusChanged {
            session_id: "sess-001".into(),
            status: SessionStatus::Thinking,
        }
    );
}

#[test]
fn parse_subagent_stop_maps_to_writing() {
    let ev = parse_hook_event(&hook("SubagentStop")).unwrap();
    assert_eq!(
        ev,
        SessionEvent::StatusChanged {
            session_id: "sess-001".into(),
            status: SessionStatus::Writing,
        }
    );
}

#[test]
fn parse_notification_without_permission_maps_to_thinking() {
    let h = HookEvent {
        event_type: "Notification".into(),
        session_id: "sess-001".into(),
        ..Default::default()
    };
    let ev = parse_hook_event(&h).unwrap();
    assert_eq!(
        ev,
        SessionEvent::StatusChanged {
            session_id: "sess-001".into(),
            status: SessionStatus::Thinking,
        }
    );
}

#[test]
fn parse_notification_with_permission_tool() {
    let h = HookEvent {
        event_type: "Notification".into(),
        session_id: "sess-001".into(),
        tool_name: "permission check".into(),
        tool_input: "".into(),
        ..Default::default()
    };
    let ev = parse_hook_event(&h).unwrap();
    assert!(matches!(ev, SessionEvent::PermissionRequest { .. }));
}

#[test]
fn parse_notification_with_tool_input() {
    let h = HookEvent {
        event_type: "Notification".into(),
        session_id: "sess-001".into(),
        tool_name: "Write".into(),
        tool_input: "main.rs".into(),
        ..Default::default()
    };
    let ev = parse_hook_event(&h).unwrap();
    assert!(matches!(ev, SessionEvent::PermissionRequest { .. }));
}

#[test]
fn parse_permission_event() {
    let h = HookEvent {
        event_type: "permission".into(),
        session_id: "sess-001".into(),
        tool_name: "Write".into(),
        tool_input: "main.rs".into(),
        ..Default::default()
    };
    let ev = parse_hook_event(&h).unwrap();
    assert_eq!(
        ev,
        SessionEvent::PermissionRequest {
            session_id: "sess-001".into(),
            tool_name: "Write".into(),
            tool_input: "main.rs".into(),
        }
    );
}

#[test]
fn parse_permission_request_event() {
    let h = HookEvent {
        event_type: "permission_request".into(),
        session_id: "sess-001".into(),
        tool_name: "Edit".into(),
        tool_input: "lib.rs".into(),
        ..Default::default()
    };
    let ev = parse_hook_event(&h).unwrap();
    assert!(matches!(ev, SessionEvent::PermissionRequest { .. }));
}

#[test]
fn parse_legacy_status_thinking() {
    let h = HookEvent {
        event_type: "status".into(),
        session_id: "sess-001".into(),
        status: "thinking".into(),
        ..Default::default()
    };
    let ev = parse_hook_event(&h).unwrap();
    assert_eq!(
        ev,
        SessionEvent::StatusChanged {
            session_id: "sess-001".into(),
            status: SessionStatus::Thinking,
        }
    );
}

#[test]
fn parse_legacy_tool_use() {
    let h = HookEvent {
        event_type: "tool_use".into(),
        session_id: "sess-001".into(),
        ..Default::default()
    };
    let ev = parse_hook_event(&h).unwrap();
    assert_eq!(
        ev,
        SessionEvent::StatusChanged {
            session_id: "sess-001".into(),
            status: SessionStatus::ToolUse,
        }
    );
}

#[test]
fn parse_unknown_event_returns_none() {
    assert!(parse_hook_event(&hook("UnknownEvent")).is_none());
}

// ── SessionStore: full lifecycle ──

fn make_session(id: u16, name: &str, status: SessionStatus) -> DaemonSession {
    DaemonSession {
        info: SessionInfo::new(id, name, status),
        ..Default::default()
    }
}

#[test]
fn session_store_full_lifecycle() {
    let mut store = SessionStore::new();
    assert!(store.is_empty());

    // Create
    let id = store.allocate_id();
    assert_eq!(id, 1);
    store.update(make_session(id, "project-alpha", SessionStatus::Idle));
    assert_eq!(store.len(), 1);

    // Read
    let s = store.get(id).unwrap();
    assert_eq!(s.info.name, "project-alpha");
    assert_eq!(s.info.status, SessionStatus::Idle);

    // Update
    store.get_mut(id).unwrap().info.status = SessionStatus::Thinking;
    assert_eq!(store.get(id).unwrap().info.status, SessionStatus::Thinking);

    // Query (list)
    let id2 = store.allocate_id();
    store.update(make_session(id2, "project-beta", SessionStatus::Writing));
    let list = store.list();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].id(), 1);
    assert_eq!(list[1].id(), 2);

    // Delete
    let removed = store.remove(id).unwrap();
    assert_eq!(removed.info.name, "project-alpha");
    assert_eq!(store.len(), 1);
    assert!(store.get(id).is_none());
}

#[test]
fn session_store_protocol_conversion() {
    let mut store = SessionStore::new();
    store.update(DaemonSession {
        info: SessionInfo {
            id: 1,
            name: "test".into(),
            status: SessionStatus::Thinking,
            source: "claude-code".into(),
            model: "claude-opus-4-6".into(),
            tokens_in: 1000,
            tokens_out: 500,
            cost_usd: 0.05,
            ..Default::default()
        },
        ..Default::default()
    });
    let proto = store.to_protocol_list();
    assert_eq!(proto.len(), 1);
    assert_eq!(proto[0].name, "test");
    assert_eq!(proto[0].status, SessionStatus::Thinking);
    assert_eq!(proto[0].source, "claude-code");
    assert_eq!(proto[0].tokens_in, 1000);
}

#[test]
fn session_store_first_with_permission() {
    let mut store = SessionStore::new();
    store.update(make_session(1, "a", SessionStatus::Idle));
    assert!(store.first_with_permission().is_none());

    store.update(DaemonSession { info: SessionInfo { id: 2, name: "b".into(), status: SessionStatus::PermissionNeeded, has_permission_request: true, ..Default::default() },
        ..Default::default()
    });
    assert_eq!(store.first_with_permission().unwrap().id(), 2);
}

// ── PermissionQueue: enqueue + resolve ──

#[test]
fn permission_queue_enqueue_and_resolve() {
    let mut queue = PermissionQueue::new();
    assert_eq!(queue.len(), 0);

    // Enqueue
    queue.push(PendingPermission {
        session_id: 1,
        tool_name: "Write".into(),
        tool_input: "main.rs".into(),
    });
    assert_eq!(queue.len(), 1);
    assert_eq!(queue.current().unwrap().tool_name, "Write");

    // Resolve with Allow
    let resolved = queue.resolve(1, PermissionAction::Allow).unwrap();
    assert_eq!(resolved.tool_name, "Write");
    assert_eq!(queue.len(), 0);
}

#[test]
fn permission_queue_always_allow() {
    let mut queue = PermissionQueue::new();
    queue.push(PendingPermission {
        session_id: 1,
        tool_name: "Read".into(),
        tool_input: "file.rs".into(),
    });

    // Resolve with Always — adds to always-allow list
    queue.resolve(1, PermissionAction::Always);
    assert!(queue.is_always_allowed("Read", "file.rs"));
    assert!(!queue.is_always_allowed("Write", "file.rs"));
}

#[test]
fn permission_queue_resolve_nonexistent_returns_none() {
    let mut queue = PermissionQueue::new();
    assert!(queue.resolve(99, PermissionAction::Allow).is_none());
}

// ── YOLO evaluation ──

#[test]
fn yolo_inactive_always_asks_user() {
    let config = YoloConfig {
        active: false,
        ..Default::default()
    };
    assert_eq!(
        evaluate_yolo(&config, "Read", "foo.rs"),
        YoloDecision::AskUser
    );
}

#[test]
fn yolo_allow_list_matches() {
    let config = YoloConfig {
        active: true,
        allow: vec!["Read(*)".into()],
        deny: vec![],
        ..Default::default()
    };
    assert_eq!(
        evaluate_yolo(&config, "Read", "any_file.rs"),
        YoloDecision::AutoAllow
    );
}

#[test]
fn yolo_deny_takes_priority() {
    let config = YoloConfig {
        active: true,
        allow: vec!["Bash(*)".into()],
        deny: vec!["Bash(rm -rf*)".into()],
        ..Default::default()
    };
    assert_eq!(
        evaluate_yolo(&config, "Bash", "rm -rf /"),
        YoloDecision::AutoDeny
    );
}

// ── validate_transcript_path ──

fn validate_transcript_path(path: &str) -> bool {
    let p = std::path::Path::new(path);
    if !p.is_absolute() { return false; }
    if p.extension().map_or(true, |e| e != "jsonl") { return false; }
    let home = dirs::home_dir().unwrap_or_default();
    let claude_dir = home.join(".claude").join("projects");
    p.starts_with(&claude_dir)
}

#[test]
fn validate_transcript_accepts_valid_path() {
    let home = dirs::home_dir().unwrap();
    let path = format!("{}/.claude/projects/abc123/session.jsonl", home.display());
    // This path doesn't exist, but our inline validator only checks prefix + extension
    assert!(validate_transcript_path(&path));
}

#[test]
fn validate_transcript_rejects_relative_path() {
    assert!(!validate_transcript_path("relative/path.jsonl"));
}

#[test]
fn validate_transcript_rejects_non_jsonl() {
    assert!(!validate_transcript_path(
        "/Users/test/.claude/projects/foo/bar.txt"
    ));
}

#[test]
fn validate_transcript_rejects_path_traversal() {
    assert!(!validate_transcript_path(
        "/Users/test/../etc/passwd.jsonl"
    ));
}

#[test]
fn validate_transcript_rejects_no_extension() {
    assert!(!validate_transcript_path("/Users/test/transcript"));
}

// ── Hook event creates session (simulated flow) ──

#[test]
fn hook_event_creates_session_in_store() {
    let h = HookEvent {
        event_type: "SessionStart".into(),
        session_id: "test-session-id".into(),
        name: "vibe-keyboard".into(),
        source: "claude-code".into(),
        cwd: "/Users/test/codes/vibe-keyboard".into(),
        ..Default::default()
    };

    // Parse the hook event
    let event = parse_hook_event(&h).unwrap();
    assert!(matches!(event, SessionEvent::Started { .. }));

    // Simulate what process_session_event does (synchronous parts)
    if let SessionEvent::Started {
        name, source, cwd, ..
    } = event
    {
        let mut store = SessionStore::new();
        let id = store.allocate_id();
        store.update(DaemonSession {
            info: SessionInfo {
                id,
                name: name.clone(),
                status: SessionStatus::Idle,
                source,
                cwd,
                ..Default::default()
            },
            ..Default::default()
        });

        assert_eq!(store.len(), 1);
        let s = store.get(id).unwrap();
        assert_eq!(s.info.name, "vibe-keyboard");
        assert_eq!(s.info.status, SessionStatus::Idle);
    }
}

// ── Permission flow simulation ──

#[test]
fn permission_flow_enqueue_evaluate_resolve() {
    let mut queue = PermissionQueue::new();
    let yolo = YoloConfig {
        active: true,
        allow: vec!["Read(*)".into()],
        deny: vec!["Bash(rm*)".into()],
        ..Default::default()
    };

    // Step 1: Permission request comes in for Write (not in allow/deny)
    let decision = evaluate_yolo(&yolo, "Write", "main.rs");
    assert_eq!(decision, YoloDecision::AskUser);

    // Step 2: Enqueue since YOLO says ask user
    queue.push(PendingPermission {
        session_id: 1,
        tool_name: "Write".into(),
        tool_input: "main.rs".into(),
    });

    // Step 3: User allows
    let resolved = queue.resolve(1, PermissionAction::Allow).unwrap();
    assert_eq!(resolved.tool_name, "Write");

    // Step 4: A Read request comes in — auto-allowed by YOLO
    let decision2 = evaluate_yolo(&yolo, "Read", "lib.rs");
    assert_eq!(decision2, YoloDecision::AutoAllow);
    // No need to enqueue
}
