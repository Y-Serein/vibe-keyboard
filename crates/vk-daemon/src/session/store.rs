//! SessionStore — in-memory session state aggregation.

use std::collections::HashMap;
use vk_protocol::message::{SessionInfo, SessionStatus};

/// Window location info for focus management.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowInfo {
    pub app_name: String,
    pub window_title: String,
    pub pid: Option<u32>,
}

/// Daemon-side session data — composes protocol `SessionInfo` with daemon-only fields.
#[derive(Debug, Clone, PartialEq)]
pub struct DaemonSession {
    /// Protocol-facing session fields (shared with simulator/UI).
    pub info: SessionInfo,

    // ── Daemon-only fields ──
    pub window_info: Option<WindowInfo>,
}

impl Default for DaemonSession {
    fn default() -> Self {
        Self {
            info: SessionInfo::default(),
            window_info: None,
        }
    }
}

impl DaemonSession {
    /// Convert to protocol SessionInfo for transport.
    pub fn to_protocol(&self) -> SessionInfo {
        self.info.clone()
    }

    // ── Convenience accessors for frequently used info fields ──

    pub fn id(&self) -> u16 {
        self.info.id
    }

    pub fn name(&self) -> &str {
        &self.info.name
    }
}

/// In-memory session store.
#[derive(Debug, Default)]
pub struct SessionStore {
    sessions: HashMap<u16, DaemonSession>,
    next_id: u16,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn allocate_id(&mut self) -> u16 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        // Skip ID 0 (used as default) and any existing IDs
        while self.next_id == 0 || self.sessions.contains_key(&self.next_id) {
            self.next_id = self.next_id.wrapping_add(1);
        }
        id
    }

    pub fn update(&mut self, session: DaemonSession) {
        if session.info.id >= self.next_id {
            self.next_id = session.info.id + 1;
        }
        self.sessions.insert(session.info.id, session);
    }

    pub fn remove(&mut self, id: u16) -> Option<DaemonSession> {
        self.sessions.remove(&id)
    }

    pub fn get(&self, id: u16) -> Option<&DaemonSession> {
        self.sessions.get(&id)
    }

    pub fn get_mut(&mut self, id: u16) -> Option<&mut DaemonSession> {
        self.sessions.get_mut(&id)
    }

    pub fn list(&self) -> Vec<&DaemonSession> {
        let mut sessions: Vec<_> = self.sessions.values().collect();
        sessions.sort_by_key(|s| s.info.id);
        sessions
    }

    pub fn list_mut(&mut self) -> Vec<&mut DaemonSession> {
        self.sessions.values_mut().collect()
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Convert all sessions to protocol format for transport.
    pub fn to_protocol_list(&self) -> Vec<SessionInfo> {
        self.list().iter().map(|s| s.to_protocol()).collect()
    }

    /// Find first session with pending permission.
    pub fn first_with_permission(&self) -> Option<&DaemonSession> {
        self.list().into_iter().find(|s| s.info.has_permission_request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_session(id: u16, name: &str, status: SessionStatus) -> DaemonSession {
        DaemonSession {
            info: SessionInfo::new(id, name, status),
            ..Default::default()
        }
    }

    #[test]
    fn new_store_is_empty() {
        let store = SessionStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn update_and_get() {
        let mut store = SessionStore::new();
        store.update(make_session(1, "A", SessionStatus::Idle));
        assert_eq!(store.len(), 1);
        assert_eq!(store.get(1).unwrap().info.name, "A");
    }

    #[test]
    fn update_existing() {
        let mut store = SessionStore::new();
        store.update(make_session(1, "A", SessionStatus::Idle));
        store.update(make_session(1, "A", SessionStatus::Thinking));
        assert_eq!(store.len(), 1);
        assert_eq!(store.get(1).unwrap().info.status, SessionStatus::Thinking);
    }

    #[test]
    fn remove_session() {
        let mut store = SessionStore::new();
        store.update(make_session(1, "A", SessionStatus::Idle));
        let removed = store.remove(1);
        assert!(removed.is_some());
        assert!(store.is_empty());
    }

    #[test]
    fn list_sorted_by_id() {
        let mut store = SessionStore::new();
        store.update(make_session(3, "C", SessionStatus::Idle));
        store.update(make_session(1, "A", SessionStatus::Idle));
        store.update(make_session(2, "B", SessionStatus::Idle));
        let list = store.list();
        assert_eq!(list[0].info.id, 1);
        assert_eq!(list[1].info.id, 2);
        assert_eq!(list[2].info.id, 3);
    }

    #[test]
    fn allocate_id_increments() {
        let mut store = SessionStore::new();
        assert_eq!(store.allocate_id(), 1);
        assert_eq!(store.allocate_id(), 2);
    }

    #[test]
    fn to_protocol_list() {
        let mut store = SessionStore::new();
        store.update(make_session(1, "A", SessionStatus::Thinking));
        let proto = store.to_protocol_list();
        assert_eq!(proto.len(), 1);
        assert_eq!(proto[0].name, "A");
        assert_eq!(proto[0].status, SessionStatus::Thinking);
        assert_eq!(proto[0].last_ai_output, "");
    }

    #[test]
    fn first_with_permission() {
        let mut store = SessionStore::new();
        store.update(make_session(1, "A", SessionStatus::Idle));
        store.update(DaemonSession {
            info: SessionInfo {
                id: 2,
                name: "B".into(),
                status: SessionStatus::PermissionNeeded,
                has_permission_request: true,
                ..Default::default()
            },
            ..Default::default()
        });
        let found = store.first_with_permission().unwrap();
        assert_eq!(found.info.id, 2);
    }

    #[test]
    fn get_mut_updates() {
        let mut store = SessionStore::new();
        store.update(make_session(1, "A", SessionStatus::Idle));
        store.get_mut(1).unwrap().info.status = SessionStatus::Done;
        assert_eq!(store.get(1).unwrap().info.status, SessionStatus::Done);
    }
}
