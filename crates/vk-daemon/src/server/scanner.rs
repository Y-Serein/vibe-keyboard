//! Background scanners — transcript and process discovery.

use std::collections::HashMap;
use std::sync::Arc;

use tracing::info;

use vk_protocol::message::{DownlinkMessage, SessionStatus};

use crate::cesp;
use crate::transcript;

use super::ipc_handler::send_downlink;
use super::state::DaemonState;

/// Periodic transcript scanner — reads JSONL files and updates DaemonSession rich fields.
pub(super) async fn run_transcript_scanner(state: Arc<DaemonState>) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        // Take scanners out briefly to do file I/O without holding the lock.
        // We swap in an empty map, scan outside the lock, then swap back.
        let mut owned_scanners = {
            let mut scanners = state.transcript_scanners.write().await;
            std::mem::take(&mut *scanners)
        };

        // File I/O happens here — no lock held
        let mut updates: Vec<(u16, transcript::TranscriptData)> = Vec::new();
        for (&session_id, scanner) in owned_scanners.iter_mut() {
            if scanner.scan() {
                updates.push((session_id, scanner.data.clone()));
            }
        }

        // Put scanners back — our scanned entries have updated offsets, so they
        // take priority. Any entries added by hook events while we were scanning
        // are new (not in owned_scanners) and should be preserved.
        {
            let mut scanners = state.transcript_scanners.write().await;
            // Insert scanned entries (updated offsets) back; they overwrite stale copies
            for (id, s) in owned_scanners {
                scanners.insert(id, s);
            }
        }

        // Stale timeout: active sessions with no activity for 30s → Idle
        {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let mut store = state.store.write().await;
            for s in store.list_mut() {
                if matches!(s.info.status, SessionStatus::Thinking | SessionStatus::Writing | SessionStatus::ToolUse) {
                    if s.info.last_activity > 0 && now - s.info.last_activity > 30 {
                        s.info.status = SessionStatus::Done;
                    }
                }
            }
        }

        // Apply updates to DaemonSession + collect CESP transitions
        if !updates.is_empty() {
            // Collect status transitions for CESP routing (done outside store lock)
            let mut cesp_transitions: Vec<(u16, String, SessionStatus, SessionStatus, String)> = Vec::new();
            let mut context_alerts: Vec<(u16, String, u8)> = Vec::new();

            let mut store = state.store.write().await;
            for (session_id, data) in &updates {
                if let Some(s) = store.get_mut(*session_id) {
                    let old_status = s.info.status;
                    let old_context_pct = s.info.context_pct;

                    if !data.model.is_empty() { s.info.model = data.model.clone(); }
                    s.info.tokens_in = data.tokens_in;
                    s.info.tokens_out = data.tokens_out;
                    s.info.cost_usd = data.cost_usd;
                    s.info.context_pct = data.context_pct;
                    if !data.last_message.is_empty() {
                        s.info.last_message = data.last_message.clone();
                    }
                    if !data.last_ai_output.is_empty() {
                        s.info.last_ai_output = data.last_ai_output.clone();
                    }
                    // Infer status from transcript entry type
                    match data.inferred_status.as_str() {
                        "thinking" => s.info.status = SessionStatus::Thinking,
                        "writing" => s.info.status = SessionStatus::Writing,
                        "tool_use" => s.info.status = SessionStatus::ToolUse,
                        "done" => s.info.status = SessionStatus::Done,
                        _ => if s.info.status == SessionStatus::Idle { s.info.status = SessionStatus::Thinking; }
                    }
                    s.info.last_activity = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();

                    // Track status transitions for CESP routing
                    if old_status != s.info.status {
                        cesp_transitions.push((
                            *session_id,
                            s.info.name.clone(),
                            old_status,
                            s.info.status,
                            data.last_message.clone(),
                        ));
                    }

                    // Track context_pct threshold crossing (>90%)
                    if data.context_pct > 90 && old_context_pct <= 90 {
                        context_alerts.push((*session_id, s.info.name.clone(), data.context_pct));
                    }
                }
            }
            let sessions = store.to_protocol_list();
            drop(store);

            // Push updated session list to simulator
            send_downlink(
                &state,
                DownlinkMessage::SessionListUpdate {
                    sessions,
                    active_index: 0,
                },
            )
            .await;

            // CESP event routing for status transitions detected by transcript scanner
            for (sid, name, old_status, new_status, description) in cesp_transitions {
                cesp::route_status_change(
                    &state.notification_queue,
                    state.as_ref(),
                    sid,
                    &name,
                    old_status,
                    new_status,
                    &description,
                )
                .await;
            }

            // CESP event routing for context_pct threshold crossings
            for (sid, name, pct) in context_alerts {
                cesp::route_context_limit(
                    &state.notification_queue,
                    state.as_ref(),
                    sid,
                    &name,
                    pct,
                )
                .await;
            }

            // T16.1: bump render generation on any session/notification state change
            state.bump_render_generation();
        }
    }
}

/// Process scanner — discovers TTY + status for running sessions.
/// Like SC's process_scanner: scans ps, resolves CWD, walks PPID for real TTY.
pub(super) async fn run_process_scanner(state: Arc<DaemonState>) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(15)).await;

        // T16.6: Run the heavy ps+lsof work off the async runtime.
        // Single ps parse + single batch lsof call instead of per-process lsof.
        let procs = tokio::task::spawn_blocking(|| {
            struct ProcInfo { _pid: u32, _ppid: u32, tty: String, cwd: String }

            let output = match std::process::Command::new("ps")
                .args(["axo", "pid,ppid,tty,command"])
                .output()
            {
                Ok(o) => o,
                Err(_) => return Vec::new(),
            };
            let ps_output = String::from_utf8_lossy(&output.stdout);

            // First pass: collect candidate PIDs and their info from ps output
            struct Candidate { pid: u32, ppid: u32, tty: String }
            let mut candidates: Vec<Candidate> = Vec::new();

            for line in ps_output.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 4 { continue; }
                let cmd = parts[3..].join(" ");
                if !cmd.contains("claude") && !cmd.contains("Claude") { continue; }
                if cmd.contains("grep") || cmd.contains("vk-daemon") { continue; }
                let pid: u32 = match parts[0].parse() { Ok(p) => p, Err(_) => continue };
                let ppid: u32 = parts[1].parse().unwrap_or(0);
                let tty = parts[2];
                if tty == "??" || tty.is_empty() { continue; }
                candidates.push(Candidate { pid, ppid, tty: tty.to_string() });
            }

            if candidates.is_empty() {
                return Vec::new();
            }

            // Batch lsof: resolve CWD for all candidate PIDs in a single call
            let pid_list: String = candidates.iter()
                .map(|c| c.pid.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let cwd_map: HashMap<u32, String> = std::process::Command::new("lsof")
                .args(["-a", &format!("-p{pid_list}"), "-d", "cwd", "-Fn"])
                .output()
                .ok()
                .map(|o| {
                    let text = String::from_utf8_lossy(&o.stdout);
                    let mut map = HashMap::new();
                    let mut current_pid: Option<u32> = None;
                    for line in text.lines() {
                        if let Some(pid_str) = line.strip_prefix('p') {
                            current_pid = pid_str.parse().ok();
                        } else if let Some(path) = line.strip_prefix('n') {
                            if let Some(pid) = current_pid {
                                map.insert(pid, path.to_string());
                            }
                        }
                    }
                    map
                })
                .unwrap_or_default();

            let mut procs: Vec<ProcInfo> = Vec::new();
            for c in candidates {
                let cwd = cwd_map.get(&c.pid).cloned().unwrap_or_default();
                if cwd.is_empty() { continue; }
                let real_tty = resolve_real_tty(c.pid, c.ppid);
                procs.push(ProcInfo { _pid: c.pid, _ppid: c.ppid, tty: real_tty, cwd });
            }
            procs
        }).await;

        let procs = match procs {
            Ok(p) => p,
            Err(_) => continue,
        };

        if procs.is_empty() { continue; }

        // Match to sessions and update TTY + status
        let mut store = state.store.write().await;
        for session in store.list_mut() {
            if session.info.cwd.is_empty() { continue; }
            for proc in &procs {
                if proc.cwd == session.info.cwd || proc.cwd.ends_with(&format!("/{}", session.info.name)) {
                    // Update TTY if empty
                    if session.info.session_tty.is_empty() && !proc.tty.is_empty() {
                        // Resolve tmux: pane TTY → client TTY for iTerm2 tab matching
                        let real_tty = resolve_tmux_client_tty(&proc.tty);
                        session.info.session_tty = real_tty;
                        if session.info.bundle_id.is_empty() {
                            session.info.bundle_id = "com.googlecode.iterm2".to_string();
                        }
                        info!("process_scanner: #{} ({}) → tty={}", session.info.id, session.info.name, proc.tty);
                    }
                    // Process is running → mark as active (not idle)
                    if session.info.status == SessionStatus::Idle {
                        session.info.status = SessionStatus::Thinking;
                    }
                    break;
                }
            }
        }
        // T16.1: bump render generation after process scanner updates sessions
        state.bump_render_generation();
    }
}

/// If a TTY belongs to a tmux pane, resolve the actual iTerm2 client TTY.
/// Uses `tmux list-panes -a` to check if the TTY is a pane, then gets client_tty.
pub(super) fn resolve_tmux_client_tty(tty: &str) -> String {
    // Step 1: Find which tmux session this pane TTY belongs to
    let output = std::process::Command::new("tmux")
        .args(["list-panes", "-a", "-F", "#{pane_tty} #{session_name}"])
        .output();
    if let Ok(o) = output {
        let lines = String::from_utf8_lossy(&o.stdout);
        for line in lines.lines() {
            let parts: Vec<&str> = line.trim().splitn(2, ' ').collect();
            if parts.len() == 2 && parts[0] == tty {
                let session_name = parts[1];
                // Step 2: Get the client TTY for THIS specific tmux session
                if let Ok(client) = std::process::Command::new("tmux")
                    .args(["list-clients", "-t", session_name, "-F", "#{client_tty}"])
                    .output()
                {
                    let client_tty = String::from_utf8_lossy(&client.stdout)
                        .lines().next().unwrap_or("").trim().to_string();
                    if !client_tty.is_empty() && client_tty.starts_with("/dev/") {
                        return client_tty;
                    }
                }
            }
        }
    }
    tty.to_string()
}

/// Walk PPID tree to find real terminal TTY (like peon's _resolve_session_tty).
/// In tmux, the direct TTY is a tmux internal PTY — we need the outer terminal's TTY.
fn resolve_real_tty(pid: u32, _ppid: u32) -> String {
    // First try: check if this process is in tmux by looking at env
    // Walk PPID tree collecting TTYs — use the highest ancestor's TTY
    let mut walk_pid = pid;
    let mut last_tty = String::new();
    for _ in 0..10 {
        if walk_pid <= 1 { break; }
        let out = std::process::Command::new("ps")
            .args(["-p", &walk_pid.to_string(), "-o", "ppid=,tty="])
            .output();
        match out {
            Ok(o) => {
                let line = String::from_utf8_lossy(&o.stdout).trim().to_string();
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 2 { break; }
                let ppid: u32 = match parts[0].parse() { Ok(p) => p, Err(_) => break };
                let tty = parts[1];
                if tty != "??" && !tty.is_empty() {
                    last_tty = if tty.starts_with("/dev/") { tty.to_string() } else { format!("/dev/{tty}") };
                }
                walk_pid = ppid;
            }
            Err(_) => break,
        }
    }
    last_tty
}
