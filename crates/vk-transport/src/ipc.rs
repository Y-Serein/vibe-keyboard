//! Unix domain socket IPC transport.
//!
//! Frame protocol: `[4B LE length][1B direction: 0=uplink, 1=downlink][encoded payload]`

use vk_protocol::codec::{decode_downlink, decode_uplink, encode_downlink, encode_uplink};
use vk_protocol::message::{DownlinkMessage, UplinkMessage};
use crate::transport::{Transport, TransportError};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

const DIR_UPLINK: u8 = 0;
const DIR_DOWNLINK: u8 = 1;
const MAX_FRAME_SIZE: u32 = 1024 * 1024; // 1 MiB safety limit

/// IPC transport over Unix domain sockets.
pub struct IpcTransport {
    uplink_rx: Mutex<mpsc::Receiver<Result<UplinkMessage, TransportError>>>,
    downlink_rx: Mutex<mpsc::Receiver<Result<DownlinkMessage, TransportError>>>,
    uplink_tx: mpsc::Sender<UplinkMessage>,
    downlink_tx: mpsc::Sender<DownlinkMessage>,
    /// Kept alive so the writer task does not terminate.
    _write_tx: mpsc::Sender<Vec<u8>>,
    connected: Arc<AtomicBool>,
}

impl IpcTransport {
    /// Connect to an IPC server at the given socket path.
    pub async fn connect(path: &str) -> Result<Self, TransportError> {
        let stream = UnixStream::connect(path)
            .await
            .map_err(TransportError::IoError)?;
        Ok(Self::from_stream(stream))
    }

    /// Listen on the given socket path and accept one connection.
    pub async fn listen(path: &str) -> Result<Self, TransportError> {
        // Remove stale socket file if it exists
        let _ = std::fs::remove_file(path);
        let listener = tokio::net::UnixListener::bind(path).map_err(TransportError::IoError)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
        let (stream, _) = listener.accept().await.map_err(TransportError::IoError)?;
        Ok(Self::from_stream(stream))
    }

    fn from_stream(stream: UnixStream) -> Self {
        let (read_half, write_half) = stream.into_split();

        let (uplink_in_tx, uplink_in_rx) =
            mpsc::channel::<Result<UplinkMessage, TransportError>>(64);
        let (downlink_in_tx, downlink_in_rx) =
            mpsc::channel::<Result<DownlinkMessage, TransportError>>(64);
        let (write_tx, write_rx) = mpsc::channel::<Vec<u8>>(64);

        // Need separate sender handles for user API
        let (uplink_out_tx, uplink_out_rx) = mpsc::channel::<UplinkMessage>(64);
        let (downlink_out_tx, downlink_out_rx) = mpsc::channel::<DownlinkMessage>(64);

        let connected = Arc::new(AtomicBool::new(true));

        // Spawn reader task: reads frames from socket, dispatches to uplink/downlink channels
        {
            let connected = Arc::clone(&connected);
            let uplink_tx = uplink_in_tx;
            let downlink_tx = downlink_in_tx;
            tokio::spawn(async move {
                let mut reader = read_half;
                loop {
                    // Read frame length
                    let mut len_buf = [0u8; 4];
                    if reader.read_exact(&mut len_buf).await.is_err() {
                        connected.store(false, Ordering::SeqCst);
                        break;
                    }
                    let frame_len = u32::from_le_bytes(len_buf);
                    if !(1..=MAX_FRAME_SIZE).contains(&frame_len) {
                        connected.store(false, Ordering::SeqCst);
                        break;
                    }

                    let mut frame = vec![0u8; frame_len as usize];
                    if reader.read_exact(&mut frame).await.is_err() {
                        connected.store(false, Ordering::SeqCst);
                        break;
                    }

                    let direction = frame[0];
                    let payload = &frame[1..];

                    match direction {
                        DIR_UPLINK => {
                            let result = decode_uplink(payload)
                                .map_err(|_| TransportError::EncodingError);
                            if uplink_tx.send(result).await.is_err() {
                                break;
                            }
                        }
                        DIR_DOWNLINK => {
                            let result = decode_downlink(payload)
                                .map_err(|_| TransportError::EncodingError);
                            if downlink_tx.send(result).await.is_err() {
                                break;
                            }
                        }
                        _ => {
                            // Unknown direction, skip
                        }
                    }
                }
            });
        }

        // Spawn writer task: takes raw frames and writes to socket
        {
            let connected = Arc::clone(&connected);
            tokio::spawn(async move {
                let mut writer = write_half;
                let mut rx = write_rx;
                while let Some(frame) = rx.recv().await {
                    let len = frame.len() as u32;
                    if writer.write_all(&len.to_le_bytes()).await.is_err() {
                        connected.store(false, Ordering::SeqCst);
                        break;
                    }
                    if writer.write_all(&frame).await.is_err() {
                        connected.store(false, Ordering::SeqCst);
                        break;
                    }
                    if writer.flush().await.is_err() {
                        connected.store(false, Ordering::SeqCst);
                        break;
                    }
                }
            });
        }

        // Merge the out channels: uplink_out_tx feeds write_tx with encoded uplink
        {
            let write_tx2 = write_tx.clone();
            tokio::spawn(async move {
                let mut rx = uplink_out_rx;
                while let Some(msg) = rx.recv().await {
                    let payload = match encode_uplink(&msg) {
                        Ok(p) => p,
                        Err(e) => { eprintln!("encode_uplink error: {e}"); continue; }
                    };
                    let mut frame = vec![DIR_UPLINK];
                    frame.extend_from_slice(&payload);
                    if write_tx2.send(frame).await.is_err() {
                        break;
                    }
                }
            });
        }
        {
            let write_tx2 = write_tx.clone();
            tokio::spawn(async move {
                let mut rx = downlink_out_rx;
                while let Some(msg) = rx.recv().await {
                    let payload = match encode_downlink(&msg) {
                        Ok(p) => p,
                        Err(e) => { eprintln!("encode_downlink error: {e}"); continue; }
                    };
                    let mut frame = vec![DIR_DOWNLINK];
                    frame.extend_from_slice(&payload);
                    if write_tx2.send(frame).await.is_err() {
                        break;
                    }
                }
            });
        }

        IpcTransport {
            uplink_rx: Mutex::new(uplink_in_rx),
            downlink_rx: Mutex::new(downlink_in_rx),
            uplink_tx: uplink_out_tx,
            downlink_tx: downlink_out_tx,
            _write_tx: write_tx,
            connected,
        }
    }
}

#[async_trait::async_trait]
impl Transport for IpcTransport {
    async fn send_uplink(&self, msg: &UplinkMessage) -> Result<(), TransportError> {
        if !self.is_connected() {
            return Err(TransportError::Disconnected);
        }
        self.uplink_tx
            .send(msg.clone())
            .await
            .map_err(|_| TransportError::Disconnected)
    }

    async fn send_downlink(&self, msg: &DownlinkMessage) -> Result<(), TransportError> {
        if !self.is_connected() {
            return Err(TransportError::Disconnected);
        }
        self.downlink_tx
            .send(msg.clone())
            .await
            .map_err(|_| TransportError::Disconnected)
    }

    async fn recv_uplink(&self) -> Result<UplinkMessage, TransportError> {
        let mut rx = self.uplink_rx.lock().await;
        rx.recv()
            .await
            .ok_or(TransportError::Disconnected)
            .and_then(|r| r)
    }

    async fn recv_downlink(&self) -> Result<DownlinkMessage, TransportError> {
        let mut rx = self.downlink_rx.lock().await;
        rx.recv()
            .await
            .ok_or(TransportError::Disconnected)
            .and_then(|r| r)
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vk_protocol::message::{ButtonId, LedColor, SessionInfo, SessionStatus, SoundType};

    fn socket_path(name: &str) -> String {
        let dir = std::env::temp_dir();
        dir.join(format!("vk-ipc-test-{}-{}", name, std::process::id()))
            .to_string_lossy()
            .to_string()
    }

    #[tokio::test]
    async fn ipc_uplink_roundtrip() {
        let path = socket_path("up");
        let server_path = path.clone();

        let server_handle = tokio::spawn(async move {
            IpcTransport::listen(&server_path).await.unwrap()
        });

        // Give the server a moment to bind
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let client = IpcTransport::connect(&path).await.unwrap();
        let server = server_handle.await.unwrap();

        // Client sends uplink, server receives
        client
            .send_uplink(&UplinkMessage::ButtonPress(ButtonId::Send))
            .await
            .unwrap();

        let msg = server.recv_uplink().await.unwrap();
        assert!(matches!(msg, UplinkMessage::ButtonPress(ButtonId::Send)));

        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn ipc_downlink_roundtrip() {
        let path = socket_path("down");
        let server_path = path.clone();

        let server_handle = tokio::spawn(async move {
            IpcTransport::listen(&server_path).await.unwrap()
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let client = IpcTransport::connect(&path).await.unwrap();
        let server = server_handle.await.unwrap();

        // Server sends downlink, client receives
        server
            .send_downlink(&DownlinkMessage::PlaySound(SoundType::Click))
            .await
            .unwrap();

        let msg = client.recv_downlink().await.unwrap();
        assert!(matches!(msg, DownlinkMessage::PlaySound(SoundType::Click)));

        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn ipc_bidirectional_complex() {
        let path = socket_path("bidir");
        let server_path = path.clone();

        let server_handle = tokio::spawn(async move {
            IpcTransport::listen(&server_path).await.unwrap()
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let client = IpcTransport::connect(&path).await.unwrap();
        let server = server_handle.await.unwrap();

        // Send a complex downlink message
        let sessions = vec![SessionInfo {
            id: 1,
            name: "test-session".to_string(),
            status: SessionStatus::Thinking,
            has_permission_request: false,
            ..Default::default()
        }];
        server
            .send_downlink(&DownlinkMessage::SessionListUpdate {
                sessions,
                active_index: 0,
            })
            .await
            .unwrap();

        // Send an uplink from client
        client
            .send_uplink(&UplinkMessage::SessionSwitch { session_id: 1 })
            .await
            .unwrap();

        let down = client.recv_downlink().await.unwrap();
        assert!(matches!(down, DownlinkMessage::SessionListUpdate { .. }));

        let up = server.recv_uplink().await.unwrap();
        assert!(matches!(
            up,
            UplinkMessage::SessionSwitch { session_id: 1 }
        ));

        // Send SetLed
        server
            .send_downlink(&DownlinkMessage::SetLed {
                button: ButtonId::Send,
                color: LedColor::GREEN,
                blink: true,
            })
            .await
            .unwrap();

        let led_msg = client.recv_downlink().await.unwrap();
        assert!(matches!(led_msg, DownlinkMessage::SetLed { .. }));

        let _ = std::fs::remove_file(&path);
    }
}
