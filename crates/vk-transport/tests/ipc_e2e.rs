//! Real IPC E2E test — daemon server + simulator client over Unix socket.
//!
//! This test starts a real IPC listener, connects a client, and verifies
//! bidirectional message flow over the actual Unix socket transport.

use std::time::Duration;
use vk_transport::IpcTransport;
use vk_protocol::message::*;
use vk_transport::Transport;

const TEST_SOCKET: &str = "/tmp/vk-test-ipc-e2e.sock";

#[tokio::test]
async fn real_ipc_session_backfill_and_button_press() {
    // Clean up stale socket
    let _ = std::fs::remove_file(TEST_SOCKET);

    // Start IPC listener (daemon side) in background
    let listener_handle = tokio::spawn(async {
        let transport = IpcTransport::listen(TEST_SOCKET).await.unwrap();

        // Daemon sends session list backfill
        transport
            .send_downlink(&DownlinkMessage::SessionListUpdate {
                sessions: vec![
                    SessionInfo {
                        id: 1,
                        name: "E2EAgent".into(),
                        status: SessionStatus::Thinking,
                        has_permission_request: false,
                        ..Default::default()
                    },
                    SessionInfo {
                        id: 2,
                        name: "Worker".into(),
                        status: SessionStatus::Idle,
                        has_permission_request: false,
                        ..Default::default()
                    },
                ],
                active_index: 0,
            })
            .await
            .unwrap();

        // Wait for button press from simulator
        let msg = transport.recv_uplink().await.unwrap();
        assert_eq!(msg, UplinkMessage::ButtonPress(ButtonId::Send));

        // Send permission request
        transport
            .send_downlink(&DownlinkMessage::PermissionRequest {
                session_id: 1,
                action_desc: "Write main.rs".into(),
            })
            .await
            .unwrap();

        // Wait for permission response
        let response = transport.recv_uplink().await.unwrap();
        assert_eq!(
            response,
            UplinkMessage::PermissionResponse {
                session_id: 1,
                action: PermissionAction::Allow,
            }
        );

        // Send dismiss
        transport
            .send_downlink(&DownlinkMessage::DismissPermission { session_id: 1 })
            .await
            .unwrap();

        "daemon done"
    });

    // Small delay for listener to bind
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Start IPC client (simulator side)
    let client_handle = tokio::spawn(async {
        let transport = IpcTransport::connect(TEST_SOCKET).await.unwrap();

        // Receive session backfill
        let msg = transport.recv_downlink().await.unwrap();
        match msg {
            DownlinkMessage::SessionListUpdate { sessions, .. } => {
                assert_eq!(sessions.len(), 2);
                assert_eq!(sessions[0].name, "E2EAgent");
                assert_eq!(sessions[1].name, "Worker");
            }
            _ => panic!("expected SessionListUpdate, got {msg:?}"),
        }

        // Send button press
        transport
            .send_uplink(&UplinkMessage::ButtonPress(ButtonId::Send))
            .await
            .unwrap();

        // Receive permission request
        let perm = transport.recv_downlink().await.unwrap();
        match perm {
            DownlinkMessage::PermissionRequest {
                session_id,
                action_desc,
            } => {
                assert_eq!(session_id, 1);
                assert_eq!(action_desc, "Write main.rs");
            }
            _ => panic!("expected PermissionRequest"),
        }

        // Send permission response
        transport
            .send_uplink(&UplinkMessage::PermissionResponse {
                session_id: 1,
                action: PermissionAction::Allow,
            })
            .await
            .unwrap();

        // Receive dismiss
        let dismiss = transport.recv_downlink().await.unwrap();
        assert_eq!(dismiss, DownlinkMessage::DismissPermission { session_id: 1 });

        "simulator done"
    });

    let (daemon_result, sim_result) = tokio::join!(listener_handle, client_handle);
    assert_eq!(daemon_result.unwrap(), "daemon done");
    assert_eq!(sim_result.unwrap(), "simulator done");

    // Cleanup
    let _ = std::fs::remove_file(TEST_SOCKET);
}

#[tokio::test]
async fn real_ipc_concurrent_bidirectional() {
    let socket = "/tmp/vk-test-ipc-concurrent.sock";
    let _ = std::fs::remove_file(socket);

    let socket_owned = socket.to_string();
    let listener = tokio::spawn(async move {
        let transport = IpcTransport::listen(&socket_owned).await.unwrap();

        // Send 5 downlinks
        for i in 0..5u16 {
            transport
                .send_downlink(&DownlinkMessage::SessionStatusChange {
                    session_id: i,
                    status: SessionStatus::Thinking,
                })
                .await
                .unwrap();
        }

        // Receive 5 uplinks
        for i in 0..5u8 {
            let msg = transport.recv_uplink().await.unwrap();
            assert_eq!(
                msg,
                UplinkMessage::KnobRotate {
                    direction: Direction::Clockwise,
                    steps: i + 1,
                }
            );
        }
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let client = tokio::spawn(async move {
        let transport = IpcTransport::connect(socket).await.unwrap();

        // Send 5 uplinks
        for i in 0..5u8 {
            transport
                .send_uplink(&UplinkMessage::KnobRotate {
                    direction: Direction::Clockwise,
                    steps: i + 1,
                })
                .await
                .unwrap();
        }

        // Receive 5 downlinks
        for i in 0..5u16 {
            let msg = transport.recv_downlink().await.unwrap();
            assert_eq!(
                msg,
                DownlinkMessage::SessionStatusChange {
                    session_id: i,
                    status: SessionStatus::Thinking,
                }
            );
        }
    });

    let (l, c) = tokio::join!(listener, client);
    l.unwrap();
    c.unwrap();

    let _ = std::fs::remove_file(socket);
}
