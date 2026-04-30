//! In-process channel transport for testing.
//!
//! Uses `tokio::sync::mpsc` to create linked transport pairs.

use vk_protocol::message::{DownlinkMessage, UplinkMessage};
use crate::transport::{Transport, TransportError};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

/// A transport backed by `tokio::sync::mpsc` channels.
///
/// Created via [`ChannelTransport::pair`] which returns two linked instances:
/// one side's `send_uplink` feeds the other's `recv_uplink`, and vice versa
/// for downlink.
pub struct ChannelTransport {
    uplink_tx: mpsc::Sender<UplinkMessage>,
    uplink_rx: Mutex<mpsc::Receiver<UplinkMessage>>,
    downlink_tx: mpsc::Sender<DownlinkMessage>,
    downlink_rx: Mutex<mpsc::Receiver<DownlinkMessage>>,
    connected: Arc<AtomicBool>,
}

impl ChannelTransport {
    /// Create a linked pair of channel transports.
    ///
    /// `buffer` is the mpsc channel capacity for each direction.
    /// The first element acts as the "keyboard" side, the second as the "daemon" side.
    pub fn pair(buffer: usize) -> (Self, Self) {
        let (up_tx_a, up_rx_b) = mpsc::channel(buffer);
        let (up_tx_b, up_rx_a) = mpsc::channel(buffer);
        let (down_tx_a, down_rx_b) = mpsc::channel(buffer);
        let (down_tx_b, down_rx_a) = mpsc::channel(buffer);

        let connected = Arc::new(AtomicBool::new(true));

        let a = ChannelTransport {
            uplink_tx: up_tx_a,
            uplink_rx: Mutex::new(up_rx_a),
            downlink_tx: down_tx_a,
            downlink_rx: Mutex::new(down_rx_a),
            connected: Arc::clone(&connected),
        };

        let b = ChannelTransport {
            uplink_tx: up_tx_b,
            uplink_rx: Mutex::new(up_rx_b),
            downlink_tx: down_tx_b,
            downlink_rx: Mutex::new(down_rx_b),
            connected: Arc::clone(&connected),
        };

        (a, b)
    }

    /// Mark this transport as disconnected.
    pub fn disconnect(&self) {
        self.connected.store(false, Ordering::SeqCst);
    }
}

#[async_trait::async_trait]
impl Transport for ChannelTransport {
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
        rx.recv().await.ok_or(TransportError::Disconnected)
    }

    async fn recv_downlink(&self) -> Result<DownlinkMessage, TransportError> {
        let mut rx = self.downlink_rx.lock().await;
        rx.recv().await.ok_or(TransportError::Disconnected)
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vk_protocol::message::{ButtonId, Direction, LedColor, SoundType};

    #[tokio::test]
    async fn pair_uplink_roundtrip() {
        let (keyboard, daemon) = ChannelTransport::pair(16);

        let msg = UplinkMessage::ButtonPress(ButtonId::Send);
        keyboard.send_uplink(&msg).await.unwrap();

        let received = daemon.recv_uplink().await.unwrap();
        assert_eq!(format!("{received:?}"), format!("{msg:?}"));
    }

    #[tokio::test]
    async fn pair_downlink_roundtrip() {
        let (keyboard, daemon) = ChannelTransport::pair(16);

        let msg = DownlinkMessage::PlaySound(SoundType::Click);
        daemon.send_downlink(&msg).await.unwrap();

        let received = keyboard.recv_downlink().await.unwrap();
        assert_eq!(format!("{received:?}"), format!("{msg:?}"));
    }

    #[tokio::test]
    async fn pair_bidirectional() {
        let (keyboard, daemon) = ChannelTransport::pair(16);

        // keyboard -> daemon (uplink)
        keyboard
            .send_uplink(&UplinkMessage::KnobRotate {
                direction: Direction::Clockwise,
                steps: 2,
            })
            .await
            .unwrap();

        // daemon -> keyboard (downlink)
        daemon
            .send_downlink(&DownlinkMessage::SetKnobRing(LedColor::GREEN))
            .await
            .unwrap();

        let up = daemon.recv_uplink().await.unwrap();
        assert!(matches!(up, UplinkMessage::KnobRotate { steps: 2, .. }));

        let down = keyboard.recv_downlink().await.unwrap();
        assert!(matches!(down, DownlinkMessage::SetKnobRing(_)));
    }

    #[tokio::test]
    async fn disconnect_prevents_send() {
        let (keyboard, _daemon) = ChannelTransport::pair(16);
        keyboard.disconnect();

        let result = keyboard
            .send_uplink(&UplinkMessage::KnobPress)
            .await;
        assert!(matches!(result, Err(TransportError::Disconnected)));
    }

    #[tokio::test]
    async fn is_connected_shared() {
        let (a, b) = ChannelTransport::pair(16);
        assert!(a.is_connected());
        assert!(b.is_connected());
        a.disconnect();
        assert!(!a.is_connected());
        assert!(!b.is_connected());
    }

    #[tokio::test]
    async fn multiple_messages_in_order() {
        let (keyboard, daemon) = ChannelTransport::pair(16);

        for i in 0..5 {
            keyboard
                .send_uplink(&UplinkMessage::SessionSwitch { session_id: i })
                .await
                .unwrap();
        }

        for i in 0..5 {
            let msg = daemon.recv_uplink().await.unwrap();
            match msg {
                UplinkMessage::SessionSwitch { session_id } => assert_eq!(session_id, i),
                other => panic!("unexpected message: {other:?}"),
            }
        }
    }
}
