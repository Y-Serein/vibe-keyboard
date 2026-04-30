//! Screen state machine — Standby / Normal / Select / Allow.
//!
//! The state machine processes UiEvents and drives transitions between four
//! screen states. It also maintains the session list, active index, and
//! permission queue needed to render each screen.

use vk_protocol::message::{ButtonId, PermissionAction, SessionStatus};

use crate::event::UiEvent;

/// LCD screen state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScreenState {
    #[default]
    Standby,
    Normal,
    Select,
    Allow,
    Notify,
}

/// Notification entry displayed in the Notify screen.
#[derive(Debug, Clone, PartialEq)]
pub struct NotificationEntry {
    pub id: u32,
    pub session_id: u16,
    pub session_name: String,
    pub status: SessionStatus,
    pub description: String,
    pub timestamp: u64,
    pub read: bool,
}

impl From<vk_protocol::message::NotificationInfo> for NotificationEntry {
    fn from(n: vk_protocol::message::NotificationInfo) -> Self {
        Self {
            id: n.id,
            session_id: n.session_id,
            session_name: n.session_name,
            status: n.status,
            description: n.description,
            timestamp: n.timestamp,
            read: n.read,
        }
    }
}

/// Toast overlay state (auto-dismiss after countdown).
#[derive(Debug, Clone)]
pub struct ToastState {
    pub session_id: u16,
    pub session_name: String,
    pub description: String,
    pub status: SessionStatus,
    pub remaining_frames: u32,
}

/// Permission entry in the pending queue.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingPermission {
    pub session_id: u16,
    pub action_desc: String,
}

/// Session data tracked by the UI.
#[derive(Debug, Clone, PartialEq)]
pub struct UiSession {
    pub id: u16,
    pub name: String,
    pub status: SessionStatus,
    // Rich fields (optional, displayed when available)
    pub source: String,
    pub model: String,
    pub cwd: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cost_usd: f64,
    pub context_pct: u8,
    pub last_message: String,
    pub last_ai_output: String,
}

/// Output action produced by the state machine after processing an event.
#[derive(Debug, Clone, PartialEq)]
pub enum UiAction {
    /// Request daemon to switch focus to a session.
    SwitchSession { session_id: u16 },
    /// Reply to a permission request.
    PermissionResponse {
        session_id: u16,
        action: PermissionAction,
    },
    /// No action needed.
    None,
}

/// Allow screen selection option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllowOption {
    Allow,
    Deny,
    Always,
}

impl AllowOption {
    pub const ALL: [AllowOption; 3] = [AllowOption::Allow, AllowOption::Deny, AllowOption::Always];

    pub fn as_str(self) -> &'static str {
        match self {
            AllowOption::Allow => "ALLOW",
            AllowOption::Deny => "DENY",
            AllowOption::Always => "ALWAYS",
        }
    }

    pub fn to_permission_action(self) -> PermissionAction {
        match self {
            AllowOption::Allow => PermissionAction::Allow,
            AllowOption::Deny => PermissionAction::Deny,
            AllowOption::Always => PermissionAction::Always,
        }
    }
}

/// The UI state machine.
#[derive(Debug)]
pub struct ScreenStateMachine {
    state: ScreenState,
    sessions: Vec<UiSession>,
    active_index: usize,
    /// Index in the session list highlighted during Select mode.
    select_index: usize,
    /// Permission queue.
    permissions: Vec<PendingPermission>,
    /// Index of the current permission being viewed.
    permission_view_index: usize,
    /// Currently highlighted Allow option (Allow/Deny/Always).
    allow_option_index: usize,
    /// Frame counter for animation.
    frame: u32,
    /// Frame at which Select mode was entered (for idle timeout).
    select_entered_frame: u32,
    /// Idle timeout in frames (e.g. 90 frames at 30fps = 3 seconds).
    idle_timeout_frames: u32,
    /// Notification entries (unread + read history).
    notifications: Vec<NotificationEntry>,
    /// Currently highlighted notification index.
    notify_index: usize,
    /// Active toasts (auto-dismiss overlays).
    toasts: Vec<ToastState>,
    /// Frame at which Notify mode was entered (for idle timeout).
    notify_entered_frame: u32,
}

impl Default for ScreenStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl ScreenStateMachine {
    /// Default idle timeout: 90 frames (3 seconds at 30fps).
    pub const DEFAULT_IDLE_TIMEOUT: u32 = 90;

    pub fn new() -> Self {
        Self {
            state: ScreenState::Standby,
            sessions: Vec::new(),
            active_index: 0,
            select_index: 0,
            permissions: Vec::new(),
            permission_view_index: 0,
            allow_option_index: 0,
            frame: 0,
            select_entered_frame: 0,
            idle_timeout_frames: Self::DEFAULT_IDLE_TIMEOUT,
            notifications: Vec::new(),
            notify_index: 0,
            toasts: Vec::new(),
            notify_entered_frame: 0,
        }
    }

    pub fn state(&self) -> ScreenState {
        self.state
    }

    pub fn sessions(&self) -> &[UiSession] {
        &self.sessions
    }

    pub fn active_index(&self) -> usize {
        self.active_index
    }

    pub fn select_index(&self) -> usize {
        self.select_index
    }

    pub fn permissions(&self) -> &[PendingPermission] {
        &self.permissions
    }

    pub fn permission_view_index(&self) -> usize {
        self.permission_view_index
    }

    pub fn allow_option_index(&self) -> usize {
        self.allow_option_index
    }

    pub fn current_allow_option(&self) -> AllowOption {
        AllowOption::ALL[self.allow_option_index]
    }

    pub fn frame(&self) -> u32 {
        self.frame
    }

    pub fn notifications(&self) -> &[NotificationEntry] {
        &self.notifications
    }

    pub fn notify_index(&self) -> usize {
        self.notify_index
    }

    pub fn toasts(&self) -> &[ToastState] {
        &self.toasts
    }

    /// Returns true if there are active animations (toasts, idle timeouts) that
    /// require continuous rendering even without state changes.
    pub fn has_active_animation(&self) -> bool {
        if !self.toasts.is_empty() {
            return true;
        }
        // Select/Notify screens have idle timeouts that need tick updates
        matches!(self.state, ScreenState::Select | ScreenState::Notify)
    }

    /// Count of unique sessions with unread notifications.
    pub fn unread_count(&self) -> usize {
        let mut seen = std::collections::HashSet::new();
        self.notifications.iter().filter(|n| !n.read && seen.insert(n.session_id)).count()
    }

    /// Aggregated notifications grouped by session_id.
    /// Returns: (session_id, session_name, worst_status, count, summary).
    pub fn aggregated_notifications(&self) -> Vec<(u16, String, SessionStatus, usize, String)> {
        let mut map: std::collections::HashMap<u16, (String, SessionStatus, usize, Vec<String>)> =
            std::collections::HashMap::new();
        for n in &self.notifications {
            let entry = map.entry(n.session_id).or_insert_with(|| {
                (n.session_name.clone(), n.status, 0, Vec::new())
            });
            entry.2 += 1;
            // Keep worst status (PermissionNeeded > Error > Done > others)
            if status_priority(n.status) < status_priority(entry.1) {
                entry.1 = n.status;
            }
            if entry.3.len() < 3 {
                entry.3.push(n.description.clone());
            }
        }
        let mut result: Vec<_> = map.into_iter()
            .map(|(sid, (name, status, count, descs))| {
                // Summary = first description (always show what happened)
                let summary = descs.first().cloned().unwrap_or_else(|| format!("{:?}", status));
                (sid, name, status, count, summary)
            })
            .collect();
        // Sort by worst status priority, then by session_id for stable ordering
        result.sort_by_key(|(sid, _, s, _, _)| (status_priority(*s), *sid));
        result
    }

    /// Replace the full notification list.
    pub fn set_notifications(&mut self, notifications: Vec<NotificationEntry>) {
        self.notifications = notifications;
        if self.notify_index >= self.notifications.len() {
            self.notify_index = 0;
        }
    }

    /// Push a toast overlay (auto-dismiss after ~5s = 150 frames at 30fps).
    pub fn show_toast(&mut self, session_id: u16, session_name: String, description: String, status: SessionStatus) {
        self.toasts.push(ToastState {
            session_id,
            session_name,
            description,
            status,
            remaining_frames: 150,
        });
        // Max 2 stacked toasts
        while self.toasts.len() > 2 {
            self.toasts.remove(0);
        }
    }

    /// Advance frame counter. Returns true if an idle timeout occurred
    /// (Select → Normal or Notify → Normal auto-transition).
    pub fn tick(&mut self) -> bool {
        self.frame = self.frame.wrapping_add(1);
        // Count down toasts
        for toast in &mut self.toasts {
            toast.remaining_frames = toast.remaining_frames.saturating_sub(1);
        }
        self.toasts.retain(|t| t.remaining_frames > 0);
        // Select idle timeout
        if self.state == ScreenState::Select
            && self.frame.wrapping_sub(self.select_entered_frame) >= self.idle_timeout_frames
        {
            self.state = ScreenState::Normal;
            return true;
        }
        // Notify idle timeout
        if self.state == ScreenState::Notify
            && self.frame.wrapping_sub(self.notify_entered_frame) >= self.idle_timeout_frames
        {
            self.state = ScreenState::Normal;
            return true;
        }
        false
    }

    /// Process a UI event, returning any action that should be sent to the daemon.
    pub fn handle_event(&mut self, event: &UiEvent) -> UiAction {
        // Toast interaction: if toast visible + SEND/KnobPress in Normal → jump to toast's session
        if !self.toasts.is_empty() && self.state == ScreenState::Normal {
            match event {
                UiEvent::ButtonPress(ButtonId::Send) | UiEvent::KnobPress => {
                    let session_id = self.toasts.last().unwrap().session_id;
                    self.toasts.clear();
                    return UiAction::SwitchSession { session_id };
                }
                _ => {}
            }
        }

        // Any user-input event clears toasts
        match event {
            UiEvent::KnobRotate { .. }
            | UiEvent::KnobPress
            | UiEvent::ButtonPress(_) => {
                self.toasts.clear();
            }
            _ => {}
        }

        match event {
            UiEvent::SessionUpdate {
                session_id,
                name,
                status,
            } => {
                self.update_session(*session_id, name.clone(), *status);
                if self.state == ScreenState::Standby && !self.sessions.is_empty() {
                    self.state = ScreenState::Normal;
                }
                UiAction::None
            }

            UiEvent::SessionListReplace { sessions } => {
                // Full replacement with rich data
                let new_ids: std::collections::HashSet<u16> = sessions.iter().map(|s| s.id).collect();
                self.sessions.retain(|s| new_ids.contains(&s.id));
                for info in sessions {
                    self.update_session_rich(info);
                }
                if self.state == ScreenState::Standby && !self.sessions.is_empty() {
                    self.state = ScreenState::Normal;
                }
                if self.sessions.is_empty() && self.permissions.is_empty() {
                    self.state = ScreenState::Standby;
                }
                UiAction::None
            }

            UiEvent::SessionRemoved { session_id } => {
                self.remove_session(*session_id);
                if self.sessions.is_empty() && self.permissions.is_empty() {
                    self.state = ScreenState::Standby;
                }
                UiAction::None
            }

            UiEvent::PermissionRequest {
                session_id,
                action_desc,
            } => {
                self.permissions.push(PendingPermission {
                    session_id: *session_id,
                    action_desc: action_desc.clone(),
                });
                if self.state != ScreenState::Allow {
                    self.state = ScreenState::Allow;
                    self.permission_view_index = self.permissions.len() - 1;
                    self.allow_option_index = 0;
                }
                UiAction::None
            }

            UiEvent::PermissionResolved {
                session_id,
                action: _,
            } => {
                self.dismiss_permission(*session_id);
                UiAction::None
            }

            UiEvent::KnobRotate { steps } => self.handle_knob_rotate(*steps),

            UiEvent::KnobPress => self.handle_knob_press(),

            UiEvent::ButtonPress(button) => self.handle_button(*button),

            UiEvent::NotificationListUpdate { notifications } => {
                self.set_notifications(notifications.clone());
                UiAction::None
            }

            UiEvent::IdleTimeout => {
                if self.state == ScreenState::Select || self.state == ScreenState::Notify {
                    self.state = ScreenState::Normal;
                }
                UiAction::None
            }
        }
    }

    fn handle_knob_rotate(&mut self, steps: i8) -> UiAction {
        match self.state {
            ScreenState::Normal => {
                // Direct switch: rotate knob = change active session immediately
                if self.sessions.len() > 1 {
                    let len = self.sessions.len() as isize;
                    let new_idx = (self.active_index as isize + steps as isize).rem_euclid(len) as usize;
                    self.active_index = new_idx;
                    let session_id = self.sessions[new_idx].id;
                    UiAction::SwitchSession { session_id }
                } else {
                    UiAction::None
                }
            }
            ScreenState::Select => {
                self.select_entered_frame = self.frame; // reset idle timer
                self.rotate_select_index(steps);
                UiAction::None
            }
            ScreenState::Allow => {
                let len = AllowOption::ALL.len() as i8;
                let new_idx =
                    (self.allow_option_index as i8 + steps).rem_euclid(len) as usize;
                self.allow_option_index = new_idx;
                UiAction::None
            }
            ScreenState::Notify => {
                self.notify_entered_frame = self.frame; // reset idle timer
                let agg_len = self.aggregated_notifications().len();
                if agg_len > 0 {
                    let new_idx = (self.notify_index as isize + steps as isize).rem_euclid(agg_len as isize) as usize;
                    self.notify_index = new_idx;
                }
                UiAction::None
            }
            ScreenState::Standby => UiAction::None,
        }
    }

    fn handle_knob_press(&mut self) -> UiAction {
        match self.state {
            ScreenState::Normal => {
                // Press knob in Normal = enter Select list
                if !self.sessions.is_empty() {
                    self.state = ScreenState::Select;
                    self.select_entered_frame = self.frame;
                    self.select_index = self.active_index;
                }
                UiAction::None
            }
            ScreenState::Select => {
                // Press knob in Select = confirm selection and switch
                if let Some(session) = self.sessions.get(self.select_index) {
                    let session_id = session.id;
                    self.active_index = self.select_index;
                    self.state = ScreenState::Normal;
                    UiAction::SwitchSession { session_id }
                } else {
                    self.state = ScreenState::Normal;
                    UiAction::None
                }
            }
            ScreenState::Allow => {
                self.confirm_allow_selection()
            }
            ScreenState::Notify => {
                self.confirm_notify_selection()
            }
            _ => UiAction::None,
        }
    }

    fn handle_button(&mut self, button: ButtonId) -> UiAction {
        match self.state {
            ScreenState::Allow => match button {
                ButtonId::Send => {
                    // Confirm current selection (default is Allow = Quick Allow)
                    self.confirm_allow_selection()
                }
                ButtonId::Cancel => {
                    // Quick Deny
                    self.allow_option_index = 1; // force Deny
                    self.confirm_allow_selection()
                }
                ButtonId::Session => {
                    // Cycle through pending permissions
                    if self.permissions.len() > 1 {
                        self.permission_view_index =
                            (self.permission_view_index + 1) % self.permissions.len();
                        self.allow_option_index = 0;
                    }
                    UiAction::None
                }
                _ => UiAction::None,
            },
            ScreenState::Normal | ScreenState::Select => match button {
                ButtonId::Session => {
                    // Always open Notify (even if empty — show "No notifications")
                    self.state = ScreenState::Notify;
                    self.notify_entered_frame = self.frame;
                    self.notify_index = 0;
                    UiAction::None
                }
                _ => UiAction::None,
            },
            ScreenState::Notify => match button {
                ButtonId::Session => {
                    // Toggle: SESSION again closes Notify
                    self.state = ScreenState::Normal;
                    UiAction::None
                }
                ButtonId::Cancel => {
                    // CANCEL = delete/dismiss current aggregated session's notifications
                    let agg = self.aggregated_notifications();
                    if let Some((session_id, _, _, _, _)) = agg.get(self.notify_index) {
                        let sid = *session_id;
                        self.notifications.retain(|n| n.session_id != sid);
                        self.notify_index = 0;
                    }
                    if self.notifications.is_empty() {
                        self.state = ScreenState::Normal;
                    }
                    UiAction::None
                }
                ButtonId::Send => {
                    self.confirm_notify_selection()
                }
                _ => UiAction::None,
            },
            _ => UiAction::None,
        }
    }

    fn confirm_allow_selection(&mut self) -> UiAction {
        if let Some(perm) = self.permissions.get(self.permission_view_index) {
            let session_id = perm.session_id;
            let action = self.current_allow_option().to_permission_action();

            self.permissions.remove(self.permission_view_index);
            if self.permission_view_index >= self.permissions.len() && !self.permissions.is_empty() {
                self.permission_view_index = self.permissions.len() - 1;
            }
            self.allow_option_index = 0;

            if self.permissions.is_empty() {
                self.state = if self.sessions.is_empty() {
                    ScreenState::Standby
                } else {
                    ScreenState::Normal
                };
            }

            UiAction::PermissionResponse { session_id, action }
        } else {
            UiAction::None
        }
    }

    fn confirm_notify_selection(&mut self) -> UiAction {
        let agg = self.aggregated_notifications();
        if let Some((session_id, _, _, _, _)) = agg.get(self.notify_index) {
            let session_id = *session_id;
            // Remove ALL notifications for this session (aggregated = consume all)
            self.notifications.retain(|n| n.session_id != session_id);
            self.notify_index = 0;
            // Switch LCD to show the target session
            self.set_active_by_id(session_id);
            self.state = ScreenState::Normal;
            UiAction::SwitchSession { session_id }
        } else {
            self.state = ScreenState::Normal;
            UiAction::None
        }
    }

    /// Set active session index by session_id (for Notify jump → LCD shows target session).
    fn set_active_by_id(&mut self, session_id: u16) {
        if let Some(idx) = self.sessions.iter().position(|s| s.id == session_id) {
            self.active_index = idx;
        }
    }

    fn dismiss_permission(&mut self, session_id: u16) {
        self.permissions.retain(|p| p.session_id != session_id);
        if self.permission_view_index >= self.permissions.len() && !self.permissions.is_empty() {
            self.permission_view_index = self.permissions.len() - 1;
        }
        if self.permissions.is_empty() && self.state == ScreenState::Allow {
            self.state = if self.sessions.is_empty() {
                ScreenState::Standby
            } else {
                ScreenState::Normal
            };
        }
    }

    fn update_session(&mut self, id: u16, name: String, status: SessionStatus) {
        if let Some(session) = self.sessions.iter_mut().find(|s| s.id == id) {
            session.name = name;
            session.status = status;
        } else {
            self.sessions.push(UiSession {
                id, name, status,
                source: String::new(), model: String::new(), cwd: String::new(),
                tokens_in: 0, tokens_out: 0, cost_usd: 0.0, context_pct: 0,
                last_message: String::new(), last_ai_output: String::new(),
            });
        }
    }

    /// Update or add a session with full rich data from protocol SessionInfo.
    fn update_session_rich(&mut self, info: &vk_protocol::message::SessionInfo) {
        if let Some(session) = self.sessions.iter_mut().find(|s| s.id == info.id) {
            if !info.name.is_empty() {
                session.name = info.name.clone();
            }
            session.status = info.status;
            if !info.source.is_empty() { session.source = info.source.clone(); }
            if !info.model.is_empty() { session.model = info.model.clone(); }
            if !info.cwd.is_empty() { session.cwd = info.cwd.clone(); }
            session.tokens_in = info.tokens_in;
            session.tokens_out = info.tokens_out;
            session.cost_usd = info.cost_usd;
            session.context_pct = info.context_pct;
            if !info.last_message.is_empty() {
                session.last_message = info.last_message.clone();
            }
            if !info.last_ai_output.is_empty() {
                session.last_ai_output = info.last_ai_output.clone();
            }
        } else {
            self.sessions.push(UiSession {
                id: info.id,
                name: info.name.clone(),
                status: info.status,
                source: info.source.clone(),
                model: info.model.clone(),
                cwd: info.cwd.clone(),
                tokens_in: info.tokens_in,
                tokens_out: info.tokens_out,
                cost_usd: info.cost_usd,
                context_pct: info.context_pct,
                last_message: info.last_message.clone(),
                last_ai_output: info.last_ai_output.clone(),
            });
        }
    }

    fn remove_session(&mut self, id: u16) {
        self.sessions.retain(|s| s.id != id);
        if self.active_index >= self.sessions.len() && !self.sessions.is_empty() {
            self.active_index = self.sessions.len() - 1;
        }
        if self.select_index >= self.sessions.len() && !self.sessions.is_empty() {
            self.select_index = self.sessions.len() - 1;
        }
    }

    fn rotate_select_index(&mut self, steps: i8) {
        if self.sessions.is_empty() {
            return;
        }
        let len = self.sessions.len() as isize;
        let new_idx = (self.select_index as isize + steps as isize).rem_euclid(len) as usize;
        self.select_index = new_idx;
    }
}

/// Priority for status sorting — delegates to canonical SessionStatus::priority().
fn status_priority(status: SessionStatus) -> u8 {
    status.priority()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_update(id: u16, name: &str, status: SessionStatus) -> UiEvent {
        UiEvent::SessionUpdate {
            session_id: id,
            name: name.into(),
            status,
        }
    }

    fn permission_request(id: u16, desc: &str) -> UiEvent {
        UiEvent::PermissionRequest {
            session_id: id,
            action_desc: desc.into(),
        }
    }

    // F4.1: 启动进入待机
    #[test]
    fn initial_state_is_standby() {
        let sm = ScreenStateMachine::new();
        assert_eq!(sm.state(), ScreenState::Standby);
        assert!(sm.sessions().is_empty());
    }

    // F4.2: 待机→Normal
    #[test]
    fn standby_to_normal_on_session_update() {
        let mut sm = ScreenStateMachine::new();
        let action = sm.handle_event(&session_update(1, "RustAgent", SessionStatus::Idle));
        assert_eq!(sm.state(), ScreenState::Normal);
        assert_eq!(action, UiAction::None);
        assert_eq!(sm.sessions().len(), 1);
    }

    // F4.3: Normal rotate directly switches session (no Select entry)
    #[test]
    fn normal_knob_rotate_switches_directly() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        sm.handle_event(&session_update(2, "B", SessionStatus::Thinking));
        assert_eq!(sm.state(), ScreenState::Normal);

        let action = sm.handle_event(&UiEvent::KnobRotate { steps: 1 });
        assert_eq!(sm.state(), ScreenState::Normal); // stays in Normal
        assert_eq!(sm.active_index(), 1);
        assert_eq!(action, UiAction::SwitchSession { session_id: 2 });
    }

    // F4.3b: KnobPress in Normal enters Select
    #[test]
    fn normal_knob_press_enters_select() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        sm.handle_event(&session_update(2, "B", SessionStatus::Thinking));
        assert_eq!(sm.state(), ScreenState::Normal);

        sm.handle_event(&UiEvent::KnobPress);
        assert_eq!(sm.state(), ScreenState::Select);
    }

    // F4.4: Select 超时返回 (event-based)
    #[test]
    fn select_timeout_returns_to_normal() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        sm.handle_event(&UiEvent::KnobPress); // press to enter Select
        assert_eq!(sm.state(), ScreenState::Select);

        sm.handle_event(&UiEvent::IdleTimeout);
        assert_eq!(sm.state(), ScreenState::Normal);
    }

    // F4.4: Select 超时返回 (tick-based, 3s = 90 frames at 30fps)
    #[test]
    fn select_timeout_via_tick() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        sm.handle_event(&UiEvent::KnobPress); // press to enter Select
        assert_eq!(sm.state(), ScreenState::Select);

        for _ in 0..89 {
            assert!(!sm.tick());
            assert_eq!(sm.state(), ScreenState::Select);
        }
        assert!(sm.tick());
        assert_eq!(sm.state(), ScreenState::Normal);
    }

    // F4.4: Activity resets idle timer
    #[test]
    fn select_activity_resets_idle_timer() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        sm.handle_event(&session_update(2, "B", SessionStatus::Idle));
        sm.handle_event(&UiEvent::KnobPress); // press to enter Select
        assert_eq!(sm.state(), ScreenState::Select);

        for _ in 0..80 { sm.tick(); }
        assert_eq!(sm.state(), ScreenState::Select);

        sm.handle_event(&UiEvent::KnobRotate { steps: 1 });

        for _ in 0..80 { sm.tick(); }
        assert_eq!(sm.state(), ScreenState::Select);

        for _ in 0..10 { sm.tick(); }
        assert_eq!(sm.state(), ScreenState::Normal);
    }

    // F4.5: Select 旋钮确认切换 session
    #[test]
    fn select_knob_press_switches_session() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        sm.handle_event(&session_update(2, "B", SessionStatus::Thinking));
        sm.handle_event(&UiEvent::KnobPress); // enter Select
        assert_eq!(sm.state(), ScreenState::Select);

        // Rotate in Select to pick session 2
        sm.handle_event(&UiEvent::KnobRotate { steps: 1 });
        assert_eq!(sm.select_index(), 1);

        // Press to confirm
        let action = sm.handle_event(&UiEvent::KnobPress);
        assert_eq!(sm.state(), ScreenState::Normal);
        assert_eq!(sm.active_index(), 1);
        assert_eq!(action, UiAction::SwitchSession { session_id: 2 });
    }

    // F4.6: Permission→Allow
    #[test]
    fn permission_triggers_allow_state() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        assert_eq!(sm.state(), ScreenState::Normal);

        sm.handle_event(&permission_request(1, "Write main.rs"));
        assert_eq!(sm.state(), ScreenState::Allow);
        assert_eq!(sm.permissions().len(), 1);
    }

    // F4.7: Allow 快捷 Allow (SEND button)
    #[test]
    fn allow_quick_allow_via_send() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        sm.handle_event(&permission_request(1, "Write main.rs"));

        let action = sm.handle_event(&UiEvent::ButtonPress(ButtonId::Send));
        assert_eq!(sm.state(), ScreenState::Normal);
        assert_eq!(
            action,
            UiAction::PermissionResponse {
                session_id: 1,
                action: PermissionAction::Allow,
            }
        );
        assert!(sm.permissions().is_empty());
    }

    // F4.8: Allow 快捷 Deny (CANCEL button)
    #[test]
    fn allow_quick_deny_via_cancel() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        sm.handle_event(&permission_request(1, "Write main.rs"));

        let action = sm.handle_event(&UiEvent::ButtonPress(ButtonId::Cancel));
        assert_eq!(sm.state(), ScreenState::Normal);
        assert_eq!(
            action,
            UiAction::PermissionResponse {
                session_id: 1,
                action: PermissionAction::Deny,
            }
        );
    }

    // F4.9: Allow 旋钮 Always
    #[test]
    fn allow_knob_always() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        sm.handle_event(&permission_request(1, "Write main.rs"));
        assert_eq!(sm.state(), ScreenState::Allow);

        // Rotate to Always (index 2): Allow=0, Deny=1, Always=2
        sm.handle_event(&UiEvent::KnobRotate { steps: 2 });
        assert_eq!(sm.current_allow_option(), AllowOption::Always);

        let action = sm.handle_event(&UiEvent::KnobPress);
        assert_eq!(
            action,
            UiAction::PermissionResponse {
                session_id: 1,
                action: PermissionAction::Always,
            }
        );
        assert_eq!(sm.state(), ScreenState::Normal);
    }

    // F4.9 variant: SEND confirms Always (not just KnobPress)
    #[test]
    fn allow_send_confirms_always() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        sm.handle_event(&permission_request(1, "Write main.rs"));

        // Rotate to Always, confirm with SEND (not KnobPress)
        sm.handle_event(&UiEvent::KnobRotate { steps: 2 });
        assert_eq!(sm.current_allow_option(), AllowOption::Always);

        let action = sm.handle_event(&UiEvent::ButtonPress(ButtonId::Send));
        assert_eq!(
            action,
            UiAction::PermissionResponse {
                session_id: 1,
                action: PermissionAction::Always,
            }
        );
    }

    // F4.10: 多审批排队
    #[test]
    fn multi_permission_queue() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        sm.handle_event(&session_update(2, "B", SessionStatus::Idle));
        sm.handle_event(&session_update(3, "C", SessionStatus::Idle));

        sm.handle_event(&permission_request(1, "Write a.rs"));
        sm.handle_event(&permission_request(2, "Write b.rs"));
        sm.handle_event(&permission_request(3, "Write c.rs"));

        assert_eq!(sm.permissions().len(), 3);
        assert_eq!(sm.state(), ScreenState::Allow);

        // Resolve first
        let action = sm.handle_event(&UiEvent::ButtonPress(ButtonId::Send));
        assert_eq!(
            action,
            UiAction::PermissionResponse {
                session_id: 1,
                action: PermissionAction::Allow,
            }
        );
        // Should stay in Allow since there are more
        assert_eq!(sm.state(), ScreenState::Allow);
        assert_eq!(sm.permissions().len(), 2);

        // Resolve second
        sm.handle_event(&UiEvent::ButtonPress(ButtonId::Send));
        assert_eq!(sm.permissions().len(), 1);
        assert_eq!(sm.state(), ScreenState::Allow);

        // Resolve third
        sm.handle_event(&UiEvent::ButtonPress(ButtonId::Send));
        assert!(sm.permissions().is_empty());
        assert_eq!(sm.state(), ScreenState::Normal);
    }

    // F4.11: 所有 session 结束回到 Standby
    #[test]
    fn all_sessions_gone_returns_to_standby() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        assert_eq!(sm.state(), ScreenState::Normal);

        sm.handle_event(&UiEvent::SessionRemoved { session_id: 1 });
        assert_eq!(sm.state(), ScreenState::Standby);
        assert!(sm.sessions().is_empty());
    }

    #[test]
    fn session_update_modifies_existing() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        sm.handle_event(&session_update(1, "A", SessionStatus::Thinking));
        assert_eq!(sm.sessions().len(), 1);
        assert_eq!(sm.sessions()[0].status, SessionStatus::Thinking);
    }

    #[test]
    fn select_index_wraps_around() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        sm.handle_event(&session_update(2, "B", SessionStatus::Idle));
        sm.handle_event(&session_update(3, "C", SessionStatus::Idle));

        // Enter select via KnobPress
        sm.handle_event(&UiEvent::KnobPress);
        assert_eq!(sm.state(), ScreenState::Select);

        // Rotate past end wraps to beginning
        sm.handle_event(&UiEvent::KnobRotate { steps: 10 });
        assert!(sm.select_index() < 3);

        // Negative rotation also wraps
        sm.handle_event(&UiEvent::KnobRotate { steps: -10 });
        assert!(sm.select_index() < 3);
    }

    #[test]
    fn allow_option_wraps_around() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        sm.handle_event(&permission_request(1, "Write"));

        // Rotate past Always wraps to Allow
        sm.handle_event(&UiEvent::KnobRotate { steps: 3 });
        assert_eq!(sm.current_allow_option(), AllowOption::Allow);

        // Negative wrap
        sm.handle_event(&UiEvent::KnobRotate { steps: -1 });
        assert_eq!(sm.current_allow_option(), AllowOption::Always);
    }

    #[test]
    fn session_button_cycles_permissions() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        sm.handle_event(&session_update(2, "B", SessionStatus::Idle));
        sm.handle_event(&permission_request(1, "Write a.rs"));
        sm.handle_event(&permission_request(2, "Write b.rs"));

        assert_eq!(sm.permission_view_index(), 0); // first one triggered Allow
        sm.handle_event(&UiEvent::ButtonPress(ButtonId::Session));
        assert_eq!(sm.permission_view_index(), 1); // cycled to second
    }

    #[test]
    fn permission_resolved_externally_dismisses() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        sm.handle_event(&permission_request(1, "Write"));
        assert_eq!(sm.state(), ScreenState::Allow);

        sm.handle_event(&UiEvent::PermissionResolved {
            session_id: 1,
            action: PermissionAction::Allow,
        });
        assert!(sm.permissions().is_empty());
        assert_eq!(sm.state(), ScreenState::Normal);
    }

    #[test]
    fn standby_knob_rotate_is_noop() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&UiEvent::KnobRotate { steps: 1 });
        assert_eq!(sm.state(), ScreenState::Standby);
    }

    #[test]
    fn normal_with_single_session_rotate_no_switch() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        // Single session: rotate does nothing (can't switch to same session)
        let action = sm.handle_event(&UiEvent::KnobRotate { steps: 1 });
        assert_eq!(sm.state(), ScreenState::Normal);
        assert_eq!(action, UiAction::None);
    }

    #[test]
    fn normal_knob_press_single_session_enters_select() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        sm.handle_event(&UiEvent::KnobPress);
        assert_eq!(sm.state(), ScreenState::Select);
    }

    #[test]
    fn frame_counter_ticks() {
        let mut sm = ScreenStateMachine::new();
        assert_eq!(sm.frame(), 0);
        sm.tick();
        sm.tick();
        assert_eq!(sm.frame(), 2);
    }

    // ── Notify screen tests ──

    fn make_notifications() -> Vec<NotificationEntry> {
        vec![
            NotificationEntry {
                id: 1,
                session_id: 10,
                session_name: "Claude".into(),
                status: SessionStatus::Done,
                description: "Task completed".into(),
                timestamp: 1000,
                read: false,
            },
            NotificationEntry {
                id: 2,
                session_id: 20,
                session_name: "Codex".into(),
                status: SessionStatus::Error,
                description: "Build failed".into(),
                timestamp: 2000,
                read: false,
            },
            NotificationEntry {
                id: 3,
                session_id: 30,
                session_name: "GPT".into(),
                status: SessionStatus::Idle,
                description: "Old message".into(),
                timestamp: 500,
                read: true,
            },
        ]
    }

    #[test]
    fn normal_session_button_enters_notify_when_unread() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        sm.set_notifications(make_notifications());
        assert_eq!(sm.state(), ScreenState::Normal);
        assert_eq!(sm.unread_count(), 2);

        sm.handle_event(&UiEvent::ButtonPress(ButtonId::Session));
        assert_eq!(sm.state(), ScreenState::Notify);
    }

    #[test]
    fn normal_session_button_opens_notify_even_when_empty() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        assert_eq!(sm.unread_count(), 0);

        sm.handle_event(&UiEvent::ButtonPress(ButtonId::Session));
        // Now always opens Notify (shows "No notifications" if empty)
        assert_eq!(sm.state(), ScreenState::Notify);

        // Press SESSION again → toggle back to Normal
        sm.handle_event(&UiEvent::ButtonPress(ButtonId::Session));
        assert_eq!(sm.state(), ScreenState::Normal);
    }

    #[test]
    fn notify_cancel_deletes_notification() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        sm.set_notifications(make_notifications());
        let initial = sm.notifications().len();
        sm.handle_event(&UiEvent::ButtonPress(ButtonId::Session));
        assert_eq!(sm.state(), ScreenState::Notify);

        // CANCEL = delete current notification (not close)
        sm.handle_event(&UiEvent::ButtonPress(ButtonId::Cancel));
        assert_eq!(sm.notifications().len(), initial - 1);
        // If more notifications remain, stay in Notify
        if sm.notifications().is_empty() {
            assert_eq!(sm.state(), ScreenState::Normal);
        } else {
            assert_eq!(sm.state(), ScreenState::Notify);
        }
    }

    #[test]
    fn notify_knob_press_switches_session_and_removes() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        sm.set_notifications(make_notifications());
        let initial_count = sm.notifications().len();
        sm.handle_event(&UiEvent::ButtonPress(ButtonId::Session));
        assert_eq!(sm.state(), ScreenState::Notify);

        // Aggregated: first row = highest priority session (Error=session 20)
        let action = sm.handle_event(&UiEvent::KnobPress);
        assert_eq!(sm.state(), ScreenState::Normal);
        // Should switch to first aggregated session
        if let UiAction::SwitchSession { session_id } = action {
            // All notifications for that session should be removed
            assert!(sm.notifications().iter().all(|n| n.session_id != session_id));
            assert!(sm.notifications().len() < initial_count);
        } else {
            panic!("expected SwitchSession");
        }
    }

    #[test]
    fn notify_send_button_switches_session() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        sm.set_notifications(make_notifications());
        sm.handle_event(&UiEvent::ButtonPress(ButtonId::Session));
        assert_eq!(sm.state(), ScreenState::Notify);

        let action = sm.handle_event(&UiEvent::ButtonPress(ButtonId::Send));
        assert_eq!(sm.state(), ScreenState::Normal);
        assert!(matches!(action, UiAction::SwitchSession { .. }));
    }

    #[test]
    fn notify_knob_rotate_scrolls_index() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        sm.set_notifications(make_notifications());
        sm.handle_event(&UiEvent::ButtonPress(ButtonId::Session));
        assert_eq!(sm.notify_index(), 0);

        sm.handle_event(&UiEvent::KnobRotate { steps: 1 });
        assert_eq!(sm.notify_index(), 1);

        sm.handle_event(&UiEvent::KnobRotate { steps: 1 });
        assert_eq!(sm.notify_index(), 2);

        // Wraps around
        sm.handle_event(&UiEvent::KnobRotate { steps: 1 });
        assert_eq!(sm.notify_index(), 0);
    }

    #[test]
    fn notify_idle_timeout_returns_to_normal() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        sm.set_notifications(make_notifications());
        sm.handle_event(&UiEvent::ButtonPress(ButtonId::Session));
        assert_eq!(sm.state(), ScreenState::Notify);

        sm.handle_event(&UiEvent::IdleTimeout);
        assert_eq!(sm.state(), ScreenState::Normal);
    }

    #[test]
    fn toast_show_and_tick_removes() {
        let mut sm = ScreenStateMachine::new();
        sm.show_toast(1, "Claude".into(), "Done".into(), SessionStatus::Done);
        assert_eq!(sm.toasts().len(), 1);
        assert_eq!(sm.toasts()[0].remaining_frames, 150);

        // Tick 149 times — should still be there
        for _ in 0..149 {
            sm.tick();
        }
        assert_eq!(sm.toasts().len(), 1);
        assert_eq!(sm.toasts()[0].remaining_frames, 1);

        // One more tick removes it
        sm.tick();
        assert!(sm.toasts().is_empty());
    }

    #[test]
    fn toast_cleared_on_user_input() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        sm.show_toast(1, "Claude".into(), "Done".into(), SessionStatus::Done);
        assert_eq!(sm.toasts().len(), 1);

        sm.handle_event(&UiEvent::KnobPress);
        assert!(sm.toasts().is_empty());
    }

    #[test]
    fn unread_count_tracks_correctly() {
        let mut sm = ScreenStateMachine::new();
        assert_eq!(sm.unread_count(), 0);

        sm.set_notifications(make_notifications());
        assert_eq!(sm.unread_count(), 2); // 2 unread, 1 read

        // Mark first as read
        sm.notifications[0].read = true;
        assert_eq!(sm.unread_count(), 1);
    }

    #[test]
    fn toast_max_two_stacked() {
        let mut sm = ScreenStateMachine::new();
        sm.show_toast(1, "A".into(), "1".into(), SessionStatus::Done);
        sm.show_toast(2, "B".into(), "2".into(), SessionStatus::Done);
        sm.show_toast(3, "C".into(), "3".into(), SessionStatus::Done);
        assert_eq!(sm.toasts().len(), 2);
        // First toast should be dropped (FIFO)
        assert_eq!(sm.toasts()[0].session_name, "B");
        assert_eq!(sm.toasts()[1].session_name, "C");
    }

    #[test]
    fn notification_list_update_event() {
        let mut sm = ScreenStateMachine::new();
        sm.handle_event(&session_update(1, "A", SessionStatus::Idle));
        let notifs = make_notifications();
        sm.handle_event(&UiEvent::NotificationListUpdate {
            notifications: notifs.clone(),
        });
        assert_eq!(sm.notifications().len(), 3);
        assert_eq!(sm.unread_count(), 2);
    }
}
