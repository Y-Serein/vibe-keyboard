//! Integration tests for ChannelTransport with all message variants.

use vk_transport::ChannelTransport;
use vk_protocol::message::*;
use vk_transport::Transport;

fn all_uplink_messages() -> Vec<UplinkMessage> {
    vec![
        UplinkMessage::ButtonPress(ButtonId::Delete),
        UplinkMessage::ButtonPress(ButtonId::Cancel),
        UplinkMessage::ButtonPress(ButtonId::Mode),
        UplinkMessage::ButtonPress(ButtonId::Session),
        UplinkMessage::ButtonPress(ButtonId::Send),
        UplinkMessage::ButtonPress(ButtonId::Voice),
        UplinkMessage::ButtonRelease(ButtonId::Send),
        UplinkMessage::KnobRotate {
            direction: Direction::Clockwise,
            steps: 1,
        },
        UplinkMessage::KnobRotate {
            direction: Direction::CounterClockwise,
            steps: 255,
        },
        UplinkMessage::KnobPress,
        UplinkMessage::KnobRelease,
        UplinkMessage::PermissionResponse {
            session_id: 42,
            action: PermissionAction::Allow,
        },
        UplinkMessage::PermissionResponse {
            session_id: 0,
            action: PermissionAction::Deny,
        },
        UplinkMessage::PermissionResponse {
            session_id: 65535,
            action: PermissionAction::Always,
        },
        UplinkMessage::SessionSwitch { session_id: 7 },
    ]
}

fn all_downlink_messages() -> Vec<DownlinkMessage> {
    vec![
        DownlinkMessage::SessionListUpdate {
            sessions: vec![
                SessionInfo {
                    id: 1,
                    name: "Claude Code".to_string(),
                    status: SessionStatus::Thinking,
                    has_permission_request: false,
                    ..Default::default()
                },
                SessionInfo {
                    id: 2,
                    name: "Codex".to_string(),
                    status: SessionStatus::PermissionNeeded,
                    has_permission_request: true,
                    ..Default::default()
                },
            ],
            active_index: 0,
        },
        DownlinkMessage::SessionListUpdate {
            sessions: vec![],
            active_index: 0,
        },
        DownlinkMessage::SessionStatusChange {
            session_id: 1,
            status: SessionStatus::Thinking,
        },
        DownlinkMessage::SessionStatusChange {
            session_id: 2,
            status: SessionStatus::ToolUse,
        },
        DownlinkMessage::SessionStatusChange {
            session_id: 3,
            status: SessionStatus::Writing,
        },
        DownlinkMessage::SessionStatusChange {
            session_id: 4,
            status: SessionStatus::Done,
        },
        DownlinkMessage::SessionStatusChange {
            session_id: 5,
            status: SessionStatus::Error,
        },
        DownlinkMessage::SessionStatusChange {
            session_id: 6,
            status: SessionStatus::Idle,
        },
        DownlinkMessage::SessionStatusChange {
            session_id: 7,
            status: SessionStatus::PermissionNeeded,
        },
        DownlinkMessage::PermissionRequest {
            session_id: 1,
            action_desc: "Execute: rm -rf /tmp/test".to_string(),
        },
        DownlinkMessage::SetLed {
            button: ButtonId::Send,
            color: LedColor::GREEN,
            blink: true,
        },
        DownlinkMessage::SetLed {
            button: ButtonId::Delete,
            color: LedColor::RED,
            blink: false,
        },
        DownlinkMessage::SetKnobRing(LedColor::AMBER),
        DownlinkMessage::PlaySound(SoundType::PermissionAlert),
        DownlinkMessage::PlaySound(SoundType::SessionComplete),
        DownlinkMessage::PlaySound(SoundType::Error),
        DownlinkMessage::PlaySound(SoundType::Click),
        DownlinkMessage::DismissPermission { session_id: 1 },
        DownlinkMessage::NotificationListUpdate {
            notifications: vec![
                NotificationInfo {
                    id: 1,
                    session_id: 1,
                    session_name: "Claude".to_string(),
                    status: SessionStatus::Done,
                    description: "Task completed".to_string(),
                    timestamp: 1000,
                    read: false,
                },
            ],
        },
        DownlinkMessage::NotificationListUpdate {
            notifications: vec![],
        },
        DownlinkMessage::SetVolume(80),
        DownlinkMessage::SetVolume(0),
        DownlinkMessage::SetMuted(true),
        DownlinkMessage::SetMuted(false),
        DownlinkMessage::SetSoundMapping {
            sound_type: SoundType::PermissionAlert,
            sound_id: "builtin:alert".to_string(),
        },
        DownlinkMessage::SetSoundMapping {
            sound_type: SoundType::Click,
            sound_id: "custom:my_click".to_string(),
        },
    ]
}

#[tokio::test]
async fn channel_all_uplink_variants() {
    let (keyboard, daemon) = ChannelTransport::pair(32);
    let messages = all_uplink_messages();

    for msg in &messages {
        keyboard.send_uplink(msg).await.unwrap();
    }

    for expected in &messages {
        let received = daemon.recv_uplink().await.unwrap();
        assert_eq!(
            format!("{received:?}"),
            format!("{expected:?}"),
            "uplink message mismatch"
        );
    }
}

#[tokio::test]
async fn channel_all_downlink_variants() {
    let (keyboard, daemon) = ChannelTransport::pair(32);
    let messages = all_downlink_messages();

    for msg in &messages {
        daemon.send_downlink(msg).await.unwrap();
    }

    for expected in &messages {
        let received = keyboard.recv_downlink().await.unwrap();
        assert_eq!(
            format!("{received:?}"),
            format!("{expected:?}"),
            "downlink message mismatch"
        );
    }
}

#[tokio::test]
async fn channel_concurrent_sends() {
    let (keyboard, daemon) = ChannelTransport::pair(32);

    // Send 10 messages rapidly from a spawned task
    let send_handle = tokio::spawn(async move {
        for i in 0u16..10 {
            keyboard
                .send_uplink(&UplinkMessage::SessionSwitch { session_id: i })
                .await
                .unwrap();
        }
    });

    // Receive all 10 and verify order
    let mut received = Vec::new();
    for _ in 0..10 {
        let msg = daemon.recv_uplink().await.unwrap();
        received.push(msg);
    }

    send_handle.await.unwrap();

    for (i, msg) in received.iter().enumerate() {
        match msg {
            UplinkMessage::SessionSwitch { session_id } => {
                assert_eq!(*session_id, i as u16, "message {i} out of order");
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }
}

#[tokio::test]
async fn channel_concurrent_bidirectional() {
    let (keyboard, daemon) = ChannelTransport::pair(32);

    // Spawn uplink sender
    let kb = std::sync::Arc::new(keyboard);
    let kb2 = std::sync::Arc::clone(&kb);
    let up_handle = tokio::spawn(async move {
        for i in 0u16..10 {
            kb2.send_uplink(&UplinkMessage::SessionSwitch { session_id: i })
                .await
                .unwrap();
        }
    });

    // Spawn downlink sender
    let dm = std::sync::Arc::new(daemon);
    let dm2 = std::sync::Arc::clone(&dm);
    let down_handle = tokio::spawn(async move {
        for _ in 0..10 {
            dm2.send_downlink(&DownlinkMessage::DismissPermission { session_id: 1 })
                .await
                .unwrap();
        }
    });

    // Receive uplinks
    let dm3 = std::sync::Arc::clone(&dm);
    let recv_up = tokio::spawn(async move {
        let mut count = 0;
        for _ in 0..10 {
            dm3.recv_uplink().await.unwrap();
            count += 1;
        }
        count
    });

    // Receive downlinks
    let kb3 = std::sync::Arc::clone(&kb);
    let recv_down = tokio::spawn(async move {
        let mut count = 0;
        for _ in 0..10 {
            kb3.recv_downlink().await.unwrap();
            count += 1;
        }
        count
    });

    up_handle.await.unwrap();
    down_handle.await.unwrap();
    assert_eq!(recv_up.await.unwrap(), 10);
    assert_eq!(recv_down.await.unwrap(), 10);
}
