//! Permission handling — YOLO mode and permission queue management.

use vk_protocol::message::PermissionAction;

/// YOLO configuration for auto-approval.
#[derive(Debug, Clone)]
pub struct YoloConfig {
    pub active: bool,
    pub allow: Vec<String>,
    pub deny: Vec<String>,
    pub notify_auto_allow: bool,
    pub auto_allow_log: bool,
}

impl Default for YoloConfig {
    fn default() -> Self {
        Self {
            active: false,
            allow: vec![
                "Read(*)".into(),
                "Glob(*)".into(),
                "Grep(*)".into(),
            ],
            deny: vec![
                "Bash(git push*)".into(),
                "Bash(rm -rf*)".into(),
                "Bash(sudo*)".into(),
            ],
            notify_auto_allow: true,
            auto_allow_log: true,
        }
    }
}

/// Pending permission in the daemon queue.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingPermission {
    pub session_id: u16,
    pub tool_name: String,
    pub tool_input: String,
}

/// YOLO decision result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YoloDecision {
    AutoAllow,
    AutoDeny,
    AskUser,
}

/// Evaluate a permission request against YOLO config.
///
/// Pattern format: `ToolName(input)` matched against glob rules.
/// Parentheses in patterns are escaped for glob matching.
pub fn evaluate_yolo(config: &YoloConfig, tool_name: &str, tool_input: &str) -> YoloDecision {
    if !config.active {
        return YoloDecision::AskUser;
    }

    // Deny list takes priority
    for deny in &config.deny {
        if matches_rule(deny, tool_name, tool_input) {
            return YoloDecision::AutoDeny;
        }
    }

    // Check allow list
    for allow in &config.allow {
        if matches_rule(allow, tool_name, tool_input) {
            return YoloDecision::AutoAllow;
        }
    }

    // Not in any list → ask user
    YoloDecision::AskUser
}

/// Match a rule like "Write(*)" or "Bash(git push*)" against tool_name + tool_input.
///
/// Rule format: `ToolPattern(InputPattern)` where both parts use glob matching.
/// Single `*` is upgraded to `**` to match path separators in tool_input.
fn matches_rule(rule: &str, tool_name: &str, tool_input: &str) -> bool {
    if let Some(paren_pos) = rule.find('(') {
        let tool_pattern = &rule[..paren_pos];
        let input_pattern = rule[paren_pos + 1..].trim_end_matches(')');
        // Upgrade single * to ** so it matches path separators (/)
        let input_glob = upgrade_glob(input_pattern);
        glob_match::glob_match(tool_pattern, tool_name)
            && glob_match::glob_match(&input_glob, tool_input)
    } else {
        glob_match::glob_match(rule, tool_name)
    }
}

/// Upgrade single `*` to `**` so glob matches across path separators.
/// Already-doubled `**` is left as-is.
fn upgrade_glob(pattern: &str) -> String {
    let mut result = String::with_capacity(pattern.len() * 2);
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '*' {
            if i + 1 < chars.len() && chars[i + 1] == '*' {
                result.push_str("**");
                i += 2;
            } else {
                result.push_str("**");
                i += 1;
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

/// Permission queue manager with always-allow tracking.
#[derive(Debug, Default)]
pub struct PermissionQueue {
    pending: Vec<PendingPermission>,
    /// Tool patterns that were marked "Always" — auto-allow in future.
    always_allow: Vec<String>,
}

const MAX_PENDING: usize = 100;

impl PermissionQueue {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            always_allow: Vec::new(),
        }
    }

    pub fn push(&mut self, perm: PendingPermission) {
        if self.pending.len() >= MAX_PENDING {
            return;
        }
        self.pending.push(perm);
    }

    /// Resolve the first pending permission for a session.
    /// If action is Always, the tool pattern is added to the always-allow list.
    pub fn resolve(&mut self, session_id: u16, action: PermissionAction) -> Option<PendingPermission> {
        let idx = self.pending.iter().position(|p| p.session_id == session_id)?;
        let perm = self.pending.remove(idx);
        if action == PermissionAction::Always {
            // Add to always-allow list for future auto-approval
            let pattern = format!("{}({})", perm.tool_name, perm.tool_input);
            self.always_allow.push(pattern);
        }
        Some(perm)
    }

    /// Check if a tool action is in the always-allow list.
    pub fn is_always_allowed(&self, tool_name: &str, tool_input: &str) -> bool {
        let check = format!("{tool_name}({tool_input})");
        self.always_allow.iter().any(|p| p == &check)
    }

    pub fn always_allow_list(&self) -> &[String] {
        &self.always_allow
    }

    /// Add a pattern to the always-allow list (for loading from config).
    pub fn add_always_allow(&mut self, pattern: String) {
        if !self.always_allow.contains(&pattern) {
            self.always_allow.push(pattern);
        }
    }

    pub fn current(&self) -> Option<&PendingPermission> {
        self.pending.first()
    }

    /// Get all pending permissions (for backfill on reconnect).
    pub fn pending_list(&self) -> &[PendingPermission] {
        &self.pending
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn pending_for_session(&self, session_id: u16) -> bool {
        self.pending.iter().any(|p| p.session_id == session_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yolo_inactive_always_asks() {
        let config = YoloConfig {
            active: false,
            ..Default::default()
        };
        assert_eq!(evaluate_yolo(&config, "Read", "file.rs"), YoloDecision::AskUser);
    }

    #[test]
    fn yolo_allow_read() {
        let config = YoloConfig {
            active: true,
            ..Default::default()
        };
        assert_eq!(evaluate_yolo(&config, "Read", "file.rs"), YoloDecision::AutoAllow);
    }

    #[test]
    fn yolo_deny_git_push() {
        let config = YoloConfig {
            active: true,
            ..Default::default()
        };
        assert_eq!(
            evaluate_yolo(&config, "Bash", "git push origin main"),
            YoloDecision::AutoDeny
        );
    }

    #[test]
    fn yolo_deny_overrides_allow() {
        let config = YoloConfig {
            active: true,
            allow: vec!["Bash(*)".into()],
            deny: vec!["Bash(rm -rf*)".into()],
            ..Default::default()
        };
        // rm -rf matches deny list → deny wins
        assert_eq!(evaluate_yolo(&config, "Bash", "rm -rf /"), YoloDecision::AutoDeny);
        // Other bash → allowed
        assert_eq!(evaluate_yolo(&config, "Bash", "ls"), YoloDecision::AutoAllow);
    }

    #[test]
    fn yolo_unknown_tool_asks() {
        let config = YoloConfig {
            active: true,
            ..Default::default()
        };
        assert_eq!(evaluate_yolo(&config, "UnknownTool", "anything"), YoloDecision::AskUser);
    }

    #[test]
    fn permission_queue_push_resolve() {
        let mut q = PermissionQueue::new();
        q.push(PendingPermission {
            session_id: 1,
            tool_name: "Write".into(),
            tool_input: "main.rs".into(),
        });
        assert_eq!(q.len(), 1);
        assert!(q.pending_for_session(1));

        let resolved = q.resolve(1, PermissionAction::Allow).unwrap();
        assert_eq!(resolved.session_id, 1);
        assert!(q.is_empty());
    }

    #[test]
    fn permission_queue_resolve_unknown_returns_none() {
        let mut q = PermissionQueue::new();
        assert!(q.resolve(999, PermissionAction::Allow).is_none());
    }

    #[test]
    fn permission_queue_current() {
        let mut q = PermissionQueue::new();
        assert!(q.current().is_none());
        q.push(PendingPermission {
            session_id: 1,
            tool_name: "W".into(),
            tool_input: "a".into(),
        });
        q.push(PendingPermission {
            session_id: 2,
            tool_name: "R".into(),
            tool_input: "b".into(),
        });
        assert_eq!(q.current().unwrap().session_id, 1);
    }

    #[test]
    fn permission_queue_multi_resolve() {
        let mut q = PermissionQueue::new();
        q.push(PendingPermission {
            session_id: 1,
            tool_name: "A".into(),
            tool_input: "".into(),
        });
        q.push(PendingPermission {
            session_id: 2,
            tool_name: "B".into(),
            tool_input: "".into(),
        });
        q.push(PendingPermission {
            session_id: 3,
            tool_name: "C".into(),
            tool_input: "".into(),
        });

        q.resolve(2, PermissionAction::Deny); // resolve middle
        assert_eq!(q.len(), 2);
        assert_eq!(q.current().unwrap().session_id, 1); // first still first
    }

    #[test]
    fn yolo_glob_patterns() {
        let config = YoloConfig {
            active: true,
            allow: vec!["Write(*)".into(), "Edit(*)".into()],
            deny: vec![],
            ..Default::default()
        };
        assert_eq!(evaluate_yolo(&config, "Write", "src/main.rs"), YoloDecision::AutoAllow);
        assert_eq!(evaluate_yolo(&config, "Edit", "Cargo.toml"), YoloDecision::AutoAllow);
        assert_eq!(evaluate_yolo(&config, "Delete", "file"), YoloDecision::AskUser);
    }

    #[test]
    fn always_resolve_adds_to_always_list() {
        let mut q = PermissionQueue::new();
        q.push(PendingPermission {
            session_id: 1,
            tool_name: "Write".into(),
            tool_input: "main.rs".into(),
        });
        q.resolve(1, PermissionAction::Always);
        assert!(q.is_always_allowed("Write", "main.rs"));
        assert!(!q.is_always_allowed("Write", "other.rs"));
    }

    #[test]
    fn allow_resolve_does_not_add_always() {
        let mut q = PermissionQueue::new();
        q.push(PendingPermission {
            session_id: 1,
            tool_name: "Write".into(),
            tool_input: "main.rs".into(),
        });
        q.resolve(1, PermissionAction::Allow);
        assert!(!q.is_always_allowed("Write", "main.rs"));
        assert!(q.always_allow_list().is_empty());
    }
}
