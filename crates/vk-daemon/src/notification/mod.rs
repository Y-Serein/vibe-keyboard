//! Desktop notification backend trait and notification queue.

pub mod mac_native;

use std::time::{SystemTime, UNIX_EPOCH};
use vk_protocol::message::SessionStatus;

/// Trait for desktop notification backends.
pub trait NotificationBackend: Send + Sync {
    /// Send a desktop notification.
    /// click_bundle_id: if Some, clicking notification activates this app.
    fn notify(&self, title: &str, body: &str, click_bundle_id: Option<&str>)
        -> Result<(), String>;
    fn name(&self) -> &str;
}

/// Null backend for testing or non-macOS platforms.
pub struct NullNotificationBackend;

impl NotificationBackend for NullNotificationBackend {
    fn notify(
        &self,
        _title: &str,
        _body: &str,
        _click_bundle_id: Option<&str>,
    ) -> Result<(), String> {
        Ok(())
    }

    fn name(&self) -> &str {
        "null"
    }
}

/// Return the platform-appropriate default notification backend.
pub fn default_backend() -> Box<dyn NotificationBackend> {
    #[cfg(target_os = "macos")]
    {
        Box::new(mac_native::MacNativeNotification)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Box::new(NullNotificationBackend)
    }
}

/// A notification entry in the queue.
#[derive(Debug, Clone)]
pub struct Notification {
    pub id: u32,
    pub session_id: u16,
    pub session_name: String,
    pub status: SessionStatus,
    pub description: String,
    pub timestamp: u64,
    pub read: bool,
}

impl From<&Notification> for vk_protocol::message::NotificationInfo {
    fn from(n: &Notification) -> Self {
        Self {
            id: n.id,
            session_id: n.session_id,
            session_name: n.session_name.clone(),
            status: n.status,
            description: n.description.clone(),
            timestamp: n.timestamp,
            read: n.read,
        }
    }
}

/// Maximum number of unread notifications before oldest are auto-marked as read.
const MAX_UNREAD: usize = 50;

/// Notification queue with unread + history.
pub struct NotificationQueue {
    items: Vec<Notification>,
    next_id: u32,
    max_history: usize,
}

impl NotificationQueue {
    /// Create a new queue with default max_history of 20.
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            next_id: 1,
            max_history: 20,
        }
    }

    /// Push a new notification into the queue.
    pub fn push(
        &mut self,
        session_id: u16,
        session_name: String,
        status: SessionStatus,
        description: String,
    ) -> u32 {
        let id = self.next_id;
        self.next_id += 1;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.items.push(Notification {
            id,
            session_id,
            session_name,
            status,
            description,
            timestamp,
            read: false,
        });

        // Cap unread: mark oldest unread as read when exceeding MAX_UNREAD
        self.cap_unread();

        // Auto-cleanup: remove oldest read items exceeding max_history
        self.cleanup_history();

        id
    }

    /// Mark a notification as read.
    pub fn mark_read(&mut self, id: u32) {
        if let Some(n) = self.items.iter_mut().find(|n| n.id == id) {
            n.read = true;
        }
    }

    /// Remove all notifications for a session (consumed after user jumps to it).
    pub fn remove_by_session(&mut self, session_id: u16) {
        self.items.retain(|n| n.session_id != session_id);
    }

    /// Remove a notification by id.
    pub fn remove(&mut self, id: u32) {
        self.items.retain(|n| n.id != id);
    }

    /// Unread items sorted by priority (PermissionNeeded > Error > Done > others).
    pub fn unread(&self) -> Vec<&Notification> {
        let mut items: Vec<&Notification> = self.items.iter().filter(|n| !n.read).collect();
        items.sort_by_key(|n| status_priority(n.status));
        items
    }

    /// Read items sorted by timestamp descending.
    pub fn history(&self) -> Vec<&Notification> {
        let mut items: Vec<&Notification> = self.items.iter().filter(|n| n.read).collect();
        items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        items
    }

    /// Count of unread notifications.
    pub fn unread_count(&self) -> usize {
        self.items.iter().filter(|n| !n.read).count()
    }

    /// All notifications: unread (priority sorted) then history (time sorted).
    pub fn all(&self) -> Vec<&Notification> {
        let mut result = self.unread();
        result.extend(self.history());
        result
    }

    /// Mark oldest unread notifications as read when unread count exceeds MAX_UNREAD.
    fn cap_unread(&mut self) {
        let unread_count = self.items.iter().filter(|n| !n.read).count();
        if unread_count > MAX_UNREAD {
            let to_mark = unread_count - MAX_UNREAD;
            // Collect indices of oldest unread items (lowest timestamp first)
            let mut unread_items: Vec<(usize, u64)> = self
                .items
                .iter()
                .enumerate()
                .filter(|(_, n)| !n.read)
                .map(|(i, n)| (i, n.timestamp))
                .collect();
            unread_items.sort_by_key(|&(_, ts)| ts);
            for &(idx, _) in unread_items.iter().take(to_mark) {
                self.items[idx].read = true;
            }
        }
    }

    /// Remove oldest read items when they exceed max_history.
    fn cleanup_history(&mut self) {
        let read_count = self.items.iter().filter(|n| n.read).count();
        if read_count > self.max_history {
            let to_remove = read_count - self.max_history;
            // Collect ids of oldest read items
            let mut read_items: Vec<(usize, u64)> = self
                .items
                .iter()
                .enumerate()
                .filter(|(_, n)| n.read)
                .map(|(i, n)| (i, n.timestamp))
                .collect();
            read_items.sort_by_key(|&(_, ts)| ts);
            let remove_indices: Vec<usize> =
                read_items.iter().take(to_remove).map(|&(i, _)| i).collect();
            // Remove in reverse order to preserve indices
            for &idx in remove_indices.iter().rev() {
                self.items.remove(idx);
            }
        }
    }
}

impl Default for NotificationQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Priority value for sorting — delegates to canonical SessionStatus::priority().
fn status_priority(status: SessionStatus) -> u8 {
    status.priority()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_unread_count() {
        let mut q = NotificationQueue::new();
        assert_eq!(q.unread_count(), 0);

        q.push(1, "s1".into(), SessionStatus::Done, "task done".into());
        q.push(2, "s2".into(), SessionStatus::Error, "build failed".into());
        assert_eq!(q.unread_count(), 2);
    }

    #[test]
    fn id_auto_increment() {
        let mut q = NotificationQueue::new();
        let id1 = q.push(1, "s1".into(), SessionStatus::Done, "a".into());
        let id2 = q.push(2, "s2".into(), SessionStatus::Done, "b".into());
        let id3 = q.push(3, "s3".into(), SessionStatus::Error, "c".into());
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
    }

    #[test]
    fn mark_read_moves_to_history() {
        let mut q = NotificationQueue::new();
        let id = q.push(1, "s1".into(), SessionStatus::Done, "done".into());
        assert_eq!(q.unread_count(), 1);
        assert_eq!(q.history().len(), 0);

        q.mark_read(id);
        assert_eq!(q.unread_count(), 0);
        assert_eq!(q.history().len(), 1);
    }

    #[test]
    fn priority_ordering() {
        let mut q = NotificationQueue::new();
        q.push(1, "s1".into(), SessionStatus::Done, "done".into());
        q.push(2, "s2".into(), SessionStatus::Error, "error".into());
        q.push(
            3,
            "s3".into(),
            SessionStatus::PermissionNeeded,
            "perm".into(),
        );

        let unread = q.unread();
        assert_eq!(unread.len(), 3);
        assert_eq!(unread[0].status, SessionStatus::PermissionNeeded);
        assert_eq!(unread[1].status, SessionStatus::Error);
        assert_eq!(unread[2].status, SessionStatus::Done);
    }

    #[test]
    fn all_returns_unread_then_history() {
        let mut q = NotificationQueue::new();
        let id1 = q.push(1, "s1".into(), SessionStatus::Done, "done".into());
        q.push(2, "s2".into(), SessionStatus::Error, "error".into());

        q.mark_read(id1);

        let all = q.all();
        assert_eq!(all.len(), 2);
        // First is unread (Error), second is history (Done, read)
        assert!(!all[0].read);
        assert!(all[1].read);
    }

    #[test]
    fn max_history_cleanup() {
        let mut q = NotificationQueue::new();
        // Push and mark-read 25 items (exceeds max_history of 20)
        for i in 0..25u16 {
            let id = q.push(i, format!("s{i}"), SessionStatus::Done, "d".into());
            q.mark_read(id);
        }
        // After cleanup on next push, oldest read items should be removed
        q.push(99, "new".into(), SessionStatus::Error, "e".into());
        let hist = q.history();
        assert!(hist.len() <= 20);
        // Total items = read (<=20) + 1 unread
        assert!(q.items.len() <= 21);
    }

    #[test]
    fn null_backend_returns_ok() {
        let backend = NullNotificationBackend;
        assert_eq!(backend.name(), "null");
        assert!(backend.notify("title", "body", None).is_ok());
        assert!(backend
            .notify("title", "body", Some("com.example"))
            .is_ok());
    }

    #[test]
    fn default_backend_returns_valid() {
        let backend = default_backend();
        // On macOS it should be "mac-native", elsewhere "null"
        assert!(!backend.name().is_empty());
    }
}
