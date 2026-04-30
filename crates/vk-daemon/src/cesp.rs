//! CESP event routing — routes SessionStatus changes to Sound + LED + Notification actions.
//!
//! Based on the CESP (Coding Event Sound Pack) specification from M10-daemon-traits.md.
//! This module is a routing layer, not a trait — it dispatches to NotificationBackend,
//! speaker (via DownlinkMessage::PlaySound), and LEDs (via DownlinkMessage::SetLed/SetKnobRing).

use tracing::info;

use vk_protocol::message::{
    ButtonId, DownlinkMessage, LedColor, SessionStatus, SoundType,
};

use crate::notification::NotificationQueue;

/// Blue color for knob ring on task completion.
const BLUE: LedColor = LedColor { r: 0, g: 100, b: 255 };

/// Trait for sending downlink messages, abstractable for testing.
#[allow(async_fn_in_trait)]
pub trait DownlinkSender: Send + Sync {
    async fn send_downlink_if_connected(&self, msg: &DownlinkMessage);
}

/// Route a session status change to appropriate Sound/LED/Notification actions.
///
/// Only routes transitions that produce notifications (Done, Error, PermissionNeeded).
/// Thinking/ToolUse/Writing/Idle are silent — no notification generated.
pub async fn route_status_change<D: DownlinkSender>(
    notification_queue: &tokio::sync::RwLock<NotificationQueue>,
    downlink_sender: &D,
    session_id: u16,
    session_name: &str,
    old_status: SessionStatus,
    new_status: SessionStatus,
    description: &str,
) {
    let _ = old_status; // available for future debounce logic
    match new_status {
        SessionStatus::Done => {
            // Sound: session complete chime
            downlink_sender
                .send_downlink_if_connected(&DownlinkMessage::PlaySound(
                    SoundType::SessionComplete,
                ))
                .await;
            // LED: knob ring blue
            downlink_sender
                .send_downlink_if_connected(&DownlinkMessage::SetKnobRing(BLUE))
                .await;
            // Notification queue
            notification_queue.write().await.push(
                session_id,
                session_name.to_string(),
                new_status,
                description.to_string(),
            );
            info!(
                "CESP: session #{} ({}) Done — sound+LED+notification",
                session_id, session_name
            );
        }
        SessionStatus::Error => {
            // Sound: error alert
            downlink_sender
                .send_downlink_if_connected(&DownlinkMessage::PlaySound(SoundType::Error))
                .await;
            // LED: SESSION button blink red
            downlink_sender
                .send_downlink_if_connected(&DownlinkMessage::SetLed {
                    button: ButtonId::Session,
                    color: LedColor::RED,
                    blink: true,
                })
                .await;
            // Notification queue
            notification_queue.write().await.push(
                session_id,
                session_name.to_string(),
                new_status,
                description.to_string(),
            );
            info!(
                "CESP: session #{} ({}) Error — sound+LED+notification",
                session_id, session_name
            );
        }
        SessionStatus::PermissionNeeded => {
            // Permission LED/Sound is already handled by the existing permission flow
            // in process_session_event. Just add to notification queue.
            notification_queue.write().await.push(
                session_id,
                session_name.to_string(),
                new_status,
                description.to_string(),
            );
            info!(
                "CESP: session #{} ({}) PermissionNeeded — notification only",
                session_id, session_name
            );
        }
        // Thinking, ToolUse, Writing, Idle transitions — silent (too frequent to notify)
        _ => {}
    }
}

/// Route a context_pct threshold crossing (>90%) to Sound + Notification.
pub async fn route_context_limit<D: DownlinkSender>(
    notification_queue: &tokio::sync::RwLock<NotificationQueue>,
    downlink_sender: &D,
    session_id: u16,
    session_name: &str,
    new_context_pct: u8,
) {
    downlink_sender
        .send_downlink_if_connected(&DownlinkMessage::PlaySound(SoundType::Error))
        .await;
    // LED: knob ring blink amber
    downlink_sender
        .send_downlink_if_connected(&DownlinkMessage::SetKnobRing(LedColor::AMBER))
        .await;
    notification_queue.write().await.push(
        session_id,
        session_name.to_string(),
        SessionStatus::Thinking, // reuse status for resource limit
        format!("Context {}%", new_context_pct),
    );
    info!(
        "CESP: session #{} ({}) context limit {}% — sound+LED+notification",
        session_id, session_name, new_context_pct
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    /// Test double that records sent messages.
    struct MockDownlink {
        sent: Arc<Mutex<Vec<DownlinkMessage>>>,
    }

    impl MockDownlink {
        fn new() -> (Self, Arc<Mutex<Vec<DownlinkMessage>>>) {
            let sent = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    sent: Arc::clone(&sent),
                },
                sent,
            )
        }
    }

    impl DownlinkSender for MockDownlink {
        async fn send_downlink_if_connected(&self, msg: &DownlinkMessage) {
            self.sent.lock().await.push(msg.clone());
        }
    }

    #[tokio::test]
    async fn route_done_adds_to_notification_queue() {
        let queue = tokio::sync::RwLock::new(NotificationQueue::new());
        let (mock, sent) = MockDownlink::new();

        route_status_change(
            &queue,
            &mock,
            1,
            "test-session",
            SessionStatus::Idle,
            SessionStatus::Done,
            "task complete",
        )
        .await;

        assert_eq!(queue.read().await.unread_count(), 1);
        let q = queue.read().await;
        let unread = q.unread();
        assert_eq!(unread[0].status, SessionStatus::Done);
        drop(q);

        // Should have sent PlaySound + SetKnobRing
        let messages = sent.lock().await;
        assert_eq!(messages.len(), 2);
        assert!(matches!(
            messages[0],
            DownlinkMessage::PlaySound(SoundType::SessionComplete)
        ));
        assert!(matches!(messages[1], DownlinkMessage::SetKnobRing(_)));
    }

    #[tokio::test]
    async fn route_error_adds_to_notification_queue() {
        let queue = tokio::sync::RwLock::new(NotificationQueue::new());
        let (mock, sent) = MockDownlink::new();

        route_status_change(
            &queue,
            &mock,
            2,
            "error-session",
            SessionStatus::Thinking,
            SessionStatus::Error,
            "build failed",
        )
        .await;

        assert_eq!(queue.read().await.unread_count(), 1);
        let q = queue.read().await;
        let unread = q.unread();
        assert_eq!(unread[0].status, SessionStatus::Error);
        drop(q);

        // Should have sent PlaySound + SetLed
        let messages = sent.lock().await;
        assert_eq!(messages.len(), 2);
        assert!(matches!(
            messages[0],
            DownlinkMessage::PlaySound(SoundType::Error)
        ));
        assert!(matches!(
            messages[1],
            DownlinkMessage::SetLed {
                button: ButtonId::Session,
                blink: true,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn route_idle_to_thinking_silent() {
        let queue = tokio::sync::RwLock::new(NotificationQueue::new());
        let (mock, sent) = MockDownlink::new();

        route_status_change(
            &queue,
            &mock,
            3,
            "thinking-session",
            SessionStatus::Idle,
            SessionStatus::Thinking,
            "working",
        )
        .await;

        // Idle → Thinking is now silent (too frequent, was spamming)
        assert_eq!(queue.read().await.unread_count(), 0);
        let messages = sent.lock().await;
        assert_eq!(messages.len(), 0);
    }

    #[tokio::test]
    async fn route_thinking_to_tooluse_silent() {
        let queue = tokio::sync::RwLock::new(NotificationQueue::new());
        let (mock, sent) = MockDownlink::new();

        route_status_change(
            &queue,
            &mock,
            3,
            "session",
            SessionStatus::Thinking,
            SessionStatus::ToolUse,
            "tool",
        )
        .await;

        // Thinking → ToolUse is silent (not Idle → Thinking)
        assert_eq!(queue.read().await.unread_count(), 0);
        let messages = sent.lock().await;
        assert_eq!(messages.len(), 0);
    }

    #[tokio::test]
    async fn notification_queue_count_increases() {
        let queue = tokio::sync::RwLock::new(NotificationQueue::new());
        let (mock, _sent) = MockDownlink::new();

        assert_eq!(queue.read().await.unread_count(), 0);

        route_status_change(
            &queue,
            &mock,
            1,
            "s1",
            SessionStatus::Idle,
            SessionStatus::Done,
            "done1",
        )
        .await;
        assert_eq!(queue.read().await.unread_count(), 1);

        route_status_change(
            &queue,
            &mock,
            2,
            "s2",
            SessionStatus::Idle,
            SessionStatus::Error,
            "error1",
        )
        .await;
        assert_eq!(queue.read().await.unread_count(), 2);

        route_status_change(
            &queue,
            &mock,
            3,
            "s3",
            SessionStatus::Idle,
            SessionStatus::PermissionNeeded,
            "perm needed",
        )
        .await;
        assert_eq!(queue.read().await.unread_count(), 3);
    }

    #[tokio::test]
    async fn route_context_limit_adds_notification() {
        let queue = tokio::sync::RwLock::new(NotificationQueue::new());
        let (mock, sent) = MockDownlink::new();

        route_context_limit(&queue, &mock, 1, "ctx-session", 95).await;

        assert_eq!(queue.read().await.unread_count(), 1);
        let q = queue.read().await;
        let unread = q.unread();
        assert_eq!(unread[0].description, "Context 95%");
        drop(q);

        let messages = sent.lock().await;
        assert_eq!(messages.len(), 2); // PlaySound + SetKnobRing
    }

    #[tokio::test]
    async fn route_permission_needed_only_queues() {
        let queue = tokio::sync::RwLock::new(NotificationQueue::new());
        let (mock, sent) = MockDownlink::new();

        route_status_change(
            &queue,
            &mock,
            4,
            "perm-session",
            SessionStatus::Thinking,
            SessionStatus::PermissionNeeded,
            "Write main.rs",
        )
        .await;

        assert_eq!(queue.read().await.unread_count(), 1);
        // PermissionNeeded does NOT send sound/LED (handled elsewhere)
        let messages = sent.lock().await;
        assert_eq!(messages.len(), 0);
    }
}
