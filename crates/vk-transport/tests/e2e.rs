//! E2E integration tests — simulator + daemon logic over ChannelTransport.
//!
//! These tests wire the UI state machine, event handling, and protocol
//! together to verify complete button→action→LCD-update loops.

use vk_transport::ChannelTransport;
use vk_protocol::message::*;
use vk_transport::Transport;

use vk_ui::event::UiEvent;
use vk_ui::screen::{ScreenStateMachine, ScreenState, UiAction};

/// Helper: create a linked simulator/daemon transport pair.
fn make_pair() -> (ChannelTransport, ChannelTransport) {
    ChannelTransport::pair(32)
}

// ── E2E.1: Button → IPC → Daemon receives ──

#[tokio::test]
async fn e2e_button_press_round_trip() {
    let (sim_side, daemon_side) = make_pair();

    // Simulator sends button press
    sim_side
        .send_uplink(&UplinkMessage::ButtonPress(ButtonId::Send))
        .await
        .unwrap();

    // Daemon receives it
    let msg = daemon_side.recv_uplink().await.unwrap();
    assert_eq!(msg, UplinkMessage::ButtonPress(ButtonId::Send));
}

// ── E2E.2: Session update → Simulator receives ──

#[tokio::test]
async fn e2e_session_update_push() {
    let (sim_side, daemon_side) = make_pair();

    // Daemon sends session list
    daemon_side
        .send_downlink(&DownlinkMessage::SessionListUpdate {
            sessions: vec![
                SessionInfo {
                    id: 1,
                    name: "RustAgent".into(),
                    status: SessionStatus::Thinking,
                    has_permission_request: false,
                    ..Default::default()
                },
                SessionInfo {
                    id: 2,
                    name: "FrontEnd".into(),
                    status: SessionStatus::Idle,
                    has_permission_request: false,
                    ..Default::default()
                },
            ],
            active_index: 0,
        })
        .await
        .unwrap();

    // Simulator receives
    let msg = sim_side.recv_downlink().await.unwrap();
    match msg {
        DownlinkMessage::SessionListUpdate {
            sessions,
            active_index,
        } => {
            assert_eq!(sessions.len(), 2);
            assert_eq!(sessions[0].name, "RustAgent");
            assert_eq!(active_index, 0);
        }
        _ => panic!("expected SessionListUpdate"),
    }
}

// ── E2E.3: Permission complete flow ──

#[tokio::test]
async fn e2e_permission_flow() {
    let (sim_side, daemon_side) = make_pair();

    // 1. Daemon sends permission request
    daemon_side
        .send_downlink(&DownlinkMessage::PermissionRequest {
            session_id: 1,
            action_desc: "Write main.rs".into(),
        })
        .await
        .unwrap();

    // Daemon also sends visual cues
    daemon_side
        .send_downlink(&DownlinkMessage::SetKnobRing(LedColor::GREEN))
        .await
        .unwrap();
    daemon_side
        .send_downlink(&DownlinkMessage::PlaySound(SoundType::PermissionAlert))
        .await
        .unwrap();

    // 2. Simulator receives permission request
    let perm_msg = sim_side.recv_downlink().await.unwrap();
    assert!(matches!(perm_msg, DownlinkMessage::PermissionRequest { .. }));
    let _knob_msg = sim_side.recv_downlink().await.unwrap();
    let _sound_msg = sim_side.recv_downlink().await.unwrap();

    // 3. Simulator sends permission response (user pressed SEND = Allow)
    sim_side
        .send_uplink(&UplinkMessage::PermissionResponse {
            session_id: 1,
            action: PermissionAction::Allow,
        })
        .await
        .unwrap();

    // 4. Daemon receives response
    let response = daemon_side.recv_uplink().await.unwrap();
    assert_eq!(
        response,
        UplinkMessage::PermissionResponse {
            session_id: 1,
            action: PermissionAction::Allow,
        }
    );

    // 5. Daemon sends dismiss
    daemon_side
        .send_downlink(&DownlinkMessage::DismissPermission { session_id: 1 })
        .await
        .unwrap();

    let dismiss = sim_side.recv_downlink().await.unwrap();
    assert_eq!(dismiss, DownlinkMessage::DismissPermission { session_id: 1 });
}

// ── E2E.4: Knob switch session ──

#[tokio::test]
async fn e2e_knob_switch_session() {
    let (sim_side, daemon_side) = make_pair();

    // Simulator sends session switch (user selected session 3)
    sim_side
        .send_uplink(&UplinkMessage::SessionSwitch { session_id: 3 })
        .await
        .unwrap();

    let msg = daemon_side.recv_uplink().await.unwrap();
    assert_eq!(msg, UplinkMessage::SessionSwitch { session_id: 3 });
}

// ── E2E.5: Bidirectional concurrent flow ──

#[tokio::test]
async fn e2e_bidirectional_concurrent() {
    let (sim_side, daemon_side) = make_pair();

    // Spawn concurrent sends from both sides
    let sim_task = tokio::spawn(async move {
        for i in 0..5u8 {
            sim_side
                .send_uplink(&UplinkMessage::KnobRotate {
                    direction: Direction::Clockwise,
                    steps: i + 1,
                })
                .await
                .unwrap();
        }
        sim_side
    });

    let daemon_task = tokio::spawn(async move {
        for i in 0..5u16 {
            daemon_side
                .send_downlink(&DownlinkMessage::SessionStatusChange {
                    session_id: i,
                    status: SessionStatus::Thinking,
                })
                .await
                .unwrap();
        }
        daemon_side
    });

    let sim_side = sim_task.await.unwrap();
    let daemon_side = daemon_task.await.unwrap();

    // Verify all messages arrived
    for i in 0..5u8 {
        let msg = daemon_side.recv_uplink().await.unwrap();
        assert_eq!(
            msg,
            UplinkMessage::KnobRotate {
                direction: Direction::Clockwise,
                steps: i + 1,
            }
        );
    }

    for i in 0..5u16 {
        let msg = sim_side.recv_downlink().await.unwrap();
        assert_eq!(
            msg,
            DownlinkMessage::SessionStatusChange {
                session_id: i,
                status: SessionStatus::Thinking,
            }
        );
    }
}

// ── E2E.6: Invalid session_id tolerance ──

#[tokio::test]
async fn e2e_invalid_session_id_no_panic() {
    let (sim_side, daemon_side) = make_pair();

    // Simulator sends response for non-existent session
    sim_side
        .send_uplink(&UplinkMessage::PermissionResponse {
            session_id: 999,
            action: PermissionAction::Allow,
        })
        .await
        .unwrap();

    // Daemon receives — should not panic
    let msg = daemon_side.recv_uplink().await.unwrap();
    assert_eq!(
        msg,
        UplinkMessage::PermissionResponse {
            session_id: 999,
            action: PermissionAction::Allow,
        }
    );
    // In real daemon, this would be logged and ignored
}

// ── E2E.7: Full message type coverage ──

#[tokio::test]
async fn e2e_all_downlink_types() {
    let (sim_side, daemon_side) = make_pair();

    let messages = vec![
        DownlinkMessage::SessionListUpdate {
            sessions: vec![],
            active_index: 0,
        },
        DownlinkMessage::SessionStatusChange {
            session_id: 1,
            status: SessionStatus::Done,
        },
        DownlinkMessage::PermissionRequest {
            session_id: 1,
            action_desc: "Test".into(),
        },
        DownlinkMessage::SetLed {
            button: ButtonId::Mode,
            color: LedColor::AMBER,
            blink: true,
        },
        DownlinkMessage::SetKnobRing(LedColor::GREEN),
        DownlinkMessage::PlaySound(SoundType::Click),
        DownlinkMessage::DismissPermission { session_id: 1 },
    ];

    for msg in &messages {
        daemon_side.send_downlink(msg).await.unwrap();
    }

    for expected in &messages {
        let received = sim_side.recv_downlink().await.unwrap();
        assert_eq!(&received, expected);
    }
}

#[tokio::test]
async fn e2e_all_uplink_types() {
    let (sim_side, daemon_side) = make_pair();

    let messages = vec![
        UplinkMessage::ButtonPress(ButtonId::Send),
        UplinkMessage::ButtonRelease(ButtonId::Send),
        UplinkMessage::KnobRotate {
            direction: Direction::CounterClockwise,
            steps: 5,
        },
        UplinkMessage::KnobPress,
        UplinkMessage::KnobRelease,
        UplinkMessage::PermissionResponse {
            session_id: 1,
            action: PermissionAction::Always,
        },
        UplinkMessage::SessionSwitch { session_id: 2 },
    ];

    for msg in &messages {
        sim_side.send_uplink(msg).await.unwrap();
    }

    for expected in &messages {
        let received = daemon_side.recv_uplink().await.unwrap();
        assert_eq!(&received, expected);
    }
}

// ── True E2E: UI state machine wired through transport ──

/// Simulates the real simulator+daemon loop:
/// daemon pushes sessions → simulator UI transitions → user presses button → daemon receives action
#[tokio::test]
async fn e2e_ui_state_machine_with_transport() {
    let (sim_side, daemon_side) = make_pair();

    // 1. Daemon pushes session list
    daemon_side
        .send_downlink(&DownlinkMessage::SessionListUpdate {
            sessions: vec![SessionInfo {
                id: 1,
                name: "Agent".into(),
                status: SessionStatus::Thinking,
                has_permission_request: false,
                ..Default::default()
            }],
            active_index: 0,
        })
        .await
        .unwrap();

    // 2. Simulator receives and feeds to UI state machine
    let mut sm = ScreenStateMachine::new();
    assert_eq!(sm.state(), ScreenState::Standby);

    let msg = sim_side.recv_downlink().await.unwrap();
    if let DownlinkMessage::SessionListUpdate { sessions, .. } = msg {
        for s in &sessions {
            sm.handle_event(&UiEvent::SessionUpdate {
                session_id: s.id,
                name: s.name.clone(),
                status: s.status,
            });
        }
    }
    assert_eq!(sm.state(), ScreenState::Normal);

    // 3. Daemon pushes permission request
    daemon_side
        .send_downlink(&DownlinkMessage::PermissionRequest {
            session_id: 1,
            action_desc: "Write main.rs".into(),
        })
        .await
        .unwrap();

    let perm_msg = sim_side.recv_downlink().await.unwrap();
    if let DownlinkMessage::PermissionRequest { session_id, action_desc } = perm_msg {
        sm.handle_event(&UiEvent::PermissionRequest { session_id, action_desc });
    }
    assert_eq!(sm.state(), ScreenState::Allow);

    // 4. User presses SEND → state machine returns PermissionResponse action
    let action = sm.handle_event(&UiEvent::ButtonPress(ButtonId::Send));
    assert_eq!(sm.state(), ScreenState::Normal);

    // 5. Simulator sends the response through transport
    if let UiAction::PermissionResponse { session_id, action: perm_action } = action {
        sim_side
            .send_uplink(&UplinkMessage::PermissionResponse {
                session_id,
                action: perm_action,
            })
            .await
            .unwrap();
    } else {
        panic!("expected PermissionResponse action");
    }

    // 6. Daemon receives the permission response
    let response = daemon_side.recv_uplink().await.unwrap();
    assert_eq!(
        response,
        UplinkMessage::PermissionResponse {
            session_id: 1,
            action: PermissionAction::Allow,
        }
    );

    // 7. Daemon sends DismissPermission back
    daemon_side
        .send_downlink(&DownlinkMessage::DismissPermission { session_id: 1 })
        .await
        .unwrap();

    // 8. Simulator receives dismiss and feeds to UI
    let dismiss_msg = sim_side.recv_downlink().await.unwrap();
    assert_eq!(dismiss_msg, DownlinkMessage::DismissPermission { session_id: 1 });
    sm.handle_event(&UiEvent::PermissionResolved {
        session_id: 1,
        action: PermissionAction::Allow,
    });
    assert_eq!(sm.state(), ScreenState::Normal, "UI should be Normal after dismiss");
}

/// Simulates knob rotation → session switch flow through UI + transport.
#[tokio::test]
async fn e2e_knob_switch_with_ui() {
    let (sim_side, daemon_side) = make_pair();

    // Daemon pushes 3 sessions
    daemon_side
        .send_downlink(&DownlinkMessage::SessionListUpdate {
            sessions: vec![
                SessionInfo { id: 1, name: "A".into(), status: SessionStatus::Idle, has_permission_request: false, ..Default::default() },
                SessionInfo { id: 2, name: "B".into(), status: SessionStatus::Thinking, has_permission_request: false, ..Default::default() },
                SessionInfo { id: 3, name: "C".into(), status: SessionStatus::Done, has_permission_request: false, ..Default::default() },
            ],
            active_index: 0,
        })
        .await
        .unwrap();

    // Simulator receives and processes
    let mut sm = ScreenStateMachine::new();
    let msg = sim_side.recv_downlink().await.unwrap();
    if let DownlinkMessage::SessionListUpdate { sessions, .. } = msg {
        for s in &sessions {
            sm.handle_event(&UiEvent::SessionUpdate {
                session_id: s.id,
                name: s.name.clone(),
                status: s.status,
            });
        }
    }
    assert_eq!(sm.state(), ScreenState::Normal);

    // User rotates knob → direct switch (stays Normal)
    let action_r1 = sm.handle_event(&UiEvent::KnobRotate { steps: 1 });
    assert_eq!(sm.state(), ScreenState::Normal);
    assert!(matches!(action_r1, UiAction::SwitchSession { .. }));

    // Press knob → enter Select, rotate, press to confirm
    sm.handle_event(&UiEvent::KnobPress);
    assert_eq!(sm.state(), ScreenState::Select);
    sm.handle_event(&UiEvent::KnobRotate { steps: 1 });
    let action = sm.handle_event(&UiEvent::KnobPress);
    assert_eq!(sm.state(), ScreenState::Normal);

    // Send SessionSwitch through transport
    if let UiAction::SwitchSession { session_id } = action {
        sim_side
            .send_uplink(&UplinkMessage::SessionSwitch { session_id })
            .await
            .unwrap();

        let received = daemon_side.recv_uplink().await.unwrap();
        if let UplinkMessage::SessionSwitch { session_id: recv_id } = received {
            assert_eq!(recv_id, session_id, "daemon should receive the selected session_id");
        } else {
            panic!("expected SessionSwitch");
        }
    } else {
        panic!("expected SwitchSession");
    }
}
