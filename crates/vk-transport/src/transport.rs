//! Transport trait — abstraction over IPC / USB HID / in-process channel.

use vk_protocol::message::{DownlinkMessage, UplinkMessage};

/// Error type for transport operations.
#[derive(Debug)]
pub enum TransportError {
    Disconnected,
    Timeout,
    EncodingError,
    IoError(std::io::Error),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disconnected => write!(f, "transport disconnected"),
            Self::Timeout => write!(f, "transport operation timed out"),
            Self::EncodingError => write!(f, "message encoding/decoding error"),
            Self::IoError(e) => write!(f, "transport I/O error: {e}"),
        }
    }
}

impl std::error::Error for TransportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IoError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for TransportError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}

/// Bidirectional transport between keyboard and daemon.
///
/// All methods are async (tokio). Implementations:
/// - `IpcTransport`     — Unix domain socket (simulator <-> daemon)
/// - `UsbHidTransport`  — USB HID device (real hardware <-> daemon)
/// - `ChannelTransport` — tokio mpsc (unit tests)
#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    /// Send an uplink message (keyboard -> daemon).
    async fn send_uplink(&self, msg: &UplinkMessage) -> Result<(), TransportError>;

    /// Send a downlink message (daemon -> keyboard).
    async fn send_downlink(&self, msg: &DownlinkMessage) -> Result<(), TransportError>;

    /// Receive an uplink message.
    async fn recv_uplink(&self) -> Result<UplinkMessage, TransportError>;

    /// Receive a downlink message.
    async fn recv_downlink(&self) -> Result<DownlinkMessage, TransportError>;

    /// Check if the transport is connected.
    fn is_connected(&self) -> bool;
}
