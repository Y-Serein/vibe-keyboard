use vk_daemon::config;
#[cfg(test)]
use vk_daemon::{focus, session};

use clap::{Parser, Subcommand};
#[cfg(test)]
use session::store::SessionStore;

#[derive(Parser)]
#[command(name = "vk-daemon")]
#[command(about = "Vibe Keyboard daemon — manages AI coding sessions")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run daemon server (hook server + IPC listener)
    Serve {
        /// Run headless (no Tauri GUI)
        #[arg(long)]
        headless: bool,
    },
    /// Session management
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },
    /// Window focus management
    Focus {
        /// Session ID to focus
        session_id: u16,
    },
    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Setup hook for an AI tool
    Setup {
        /// Tool name: claude-code, cursor, codex
        tool: String,
        /// Daemon hook port (default: 19280)
        #[arg(long, default_value = "19280")]
        port: u16,
    },
}

#[derive(Subcommand)]
enum SessionAction {
    /// List all active sessions
    List,
    /// Show details of a specific session
    Status {
        /// Session ID
        id: u16,
    },
    /// Inject mock sessions for testing
    Mock,
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration
    Show,
    /// Set a configuration value
    Set {
        /// Key path (e.g. "yolo.active")
        key: String,
        /// Value
        value: String,
    },
}

fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { headless } => {
            let rt = tokio::runtime::Runtime::new().unwrap();
            if let Err(e) = rt.block_on(vk_daemon::server::run_serve(headless)) {
                eprintln!("[daemon] serve error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Setup { tool, port } => handle_setup(&tool, port),
        other => {
            // CLI subcommands connect to running daemon via HTTP API
            let port = 19280u16; // default hook port
            let rt = tokio::runtime::Runtime::new().unwrap();
            match other {
                Commands::Session { action } => rt.block_on(handle_session_remote(action, port)),
                Commands::Focus { session_id } => {
                    rt.block_on(handle_focus_remote(session_id, port));
                }
                Commands::Config { action } => handle_config(action),
                Commands::Serve { .. } | Commands::Setup { .. } => unreachable!(),
            }
        }
    }
}

fn handle_config(action: ConfigAction) {
    let config_path = config::default_config_path();
    match action {
        ConfigAction::Show => {
            let cfg = config::load_config(&config_path);
            let output = format_config_show(&cfg, &config_path);
            print!("{output}");
        }
        ConfigAction::Set { key, value } => {
            let output = execute_config_set(&config_path, &key, &value);
            print!("{output}");
        }
    }
}

fn format_config_show(cfg: &config::DaemonConfig, path: &std::path::Path) -> String {
    let toml_str = toml::to_string_pretty(cfg).unwrap_or_else(|e| format!("serialize error: {e}"));
    format!("[daemon] config from {}\n{toml_str}", path.display())
}

fn execute_config_set(path: &std::path::Path, key: &str, value: &str) -> String {
    let mut cfg = config::load_config(path);
    match key {
        "general.hook_port" => {
            match value.parse::<u16>() {
                Ok(v) => cfg.general.hook_port = v,
                Err(e) => return format!("[daemon] invalid port: {e}\n"),
            }
        }
        "general.log_level" => cfg.general.log_level = value.to_string(),
        "yolo.active" => {
            match value.parse::<bool>() {
                Ok(v) => cfg.yolo.active = v,
                Err(e) => return format!("[daemon] invalid bool: {e}\n"),
            }
        }
        "ipc.socket_path" => cfg.ipc.socket_path = value.to_string(),
        _ => return format!("[daemon] unknown config key: {key}\n"),
    }
    match config::save_config(path, &cfg) {
        Ok(()) => format!("[daemon] config set {key}={value} (saved to {})\n", path.display()),
        Err(e) => format!("[daemon] save failed: {e}\n"),
    }
}

#[cfg(test)]
fn format_session_list(store: &SessionStore) -> String {
    let sessions = store.list();
    if sessions.is_empty() {
        return "[daemon] no active sessions\n".to_string();
    }
    let mut out = String::new();
    for s in &sessions {
        out.push_str(&format!(
            "  #{} {} {:?} {}\n",
            s.info.id,
            s.info.name,
            s.info.status,
            if s.info.has_permission_request { "[!perm]" } else { "" }
        ));
    }
    out.push_str(&format!("[daemon] {} sessions\n", sessions.len()));
    out
}

#[cfg(test)]
fn format_session_status(store: &SessionStore, id: u16) -> String {
    match store.get(id) {
        Some(s) => format!(
            "[daemon] session #{}: name={} status={:?} permission={}\n",
            s.info.id, s.info.name, s.info.status, s.info.has_permission_request
        ),
        None => format!("[daemon] session #{id} not found\n"),
    }
}

#[cfg(test)]
fn inject_mock_sessions(store: &mut SessionStore) -> String {
    use vk_protocol::message::SessionStatus;

    store.update(session::store::DaemonSession {
        info: vk_protocol::message::SessionInfo::new(1, "RustAgent", SessionStatus::Thinking),
        ..Default::default()
    });
    store.update(session::store::DaemonSession {
        info: vk_protocol::message::SessionInfo::new(2, "FrontEnd", SessionStatus::Idle),
        ..Default::default()
    });
    store.update(session::store::DaemonSession {
        info: vk_protocol::message::SessionInfo {
            id: 3,
            name: "DevOps".into(),
            status: SessionStatus::PermissionNeeded,
            has_permission_request: true,
            ..Default::default()
        },
        ..Default::default()
    });
    "[daemon] injected 3 mock sessions\n".to_string()
}

async fn handle_session_remote(action: SessionAction, port: u16) {
    let url = format!("http://localhost:{port}");
    match action {
        SessionAction::List => {
            let resp = simple_http_get(&format!("{url}/sessions")).await;
            match resp {
                Ok(body) => {
                    let sessions: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap_or_else(|e| {
                        eprintln!("warning: failed to parse API response: {e}");
                        Default::default()
                    });
                    if sessions.is_empty() {
                        println!("[daemon] no active sessions");
                    } else {
                        for s in &sessions {
                            println!("  #{} {} [{}] perm={}",
                                s["id"], s["name"], s["status"], s["has_permission"]);
                        }
                        println!("[daemon] {} sessions", sessions.len());
                    }
                }
                Err(e) => eprintln!("[daemon] cannot connect to daemon: {e}"),
            }
        }
        SessionAction::Status { id } => {
            let resp = simple_http_get(&format!("{url}/sessions")).await;
            match resp {
                Ok(body) => {
                    let sessions: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap_or_else(|e| {
                        eprintln!("warning: failed to parse API response: {e}");
                        Default::default()
                    });
                    match sessions.iter().find(|s| s["id"] == id) {
                        Some(s) => println!("{}", serde_json::to_string_pretty(s).unwrap()),
                        None => println!("[daemon] session #{id} not found"),
                    }
                }
                Err(e) => eprintln!("[daemon] cannot connect to daemon: {e}"),
            }
        }
        SessionAction::Mock => {
            println!("[daemon] mock sessions not supported in remote mode — use standalone");
        }
    }
}

async fn handle_focus_remote(session_id: u16, port: u16) {
    let url = format!("http://localhost:{port}/button");
    let body = serde_json::json!({"id": "session"});
    match simple_http_post(&url, &body.to_string()).await {
        Ok(_) => println!("[daemon] focus request sent for session #{session_id}"),
        Err(e) => eprintln!("[daemon] cannot connect to daemon: {e}"),
    }
}

async fn simple_http_get(url: &str) -> Result<String, String> {
    reqwest::get(url).await.map_err(|e| e.to_string())?
        .text().await.map_err(|e| e.to_string())
}

async fn simple_http_post(url: &str, body: &str) -> Result<String, String> {
    reqwest::Client::new()
        .post(url)
        .header("content-type", "application/json")
        .body(body.to_string())
        .send().await.map_err(|e| e.to_string())?
        .text().await.map_err(|e| e.to_string())
}

fn handle_setup(tool: &str, port: u16) {
    if let Err(e) = handle_setup_inner(tool, port) {
        eprintln!("[setup] Error: {e}");
        std::process::exit(1);
    }
}

fn handle_setup_inner(tool: &str, port: u16) -> Result<(), String> {
    match tool {
        "claude-code" => {
            let settings_path = dirs::home_dir()
                .map(|h| h.join(".claude").join("settings.json"))
                .ok_or("cannot determine home directory")?;

            println!("[setup] Installing vk-daemon hook for Claude Code...");
            println!("[setup] Settings file: {}", settings_path.display());

            let mut settings: serde_json::Value = if settings_path.exists() {
                let content = std::fs::read_to_string(&settings_path)
                    .map_err(|e| format!("cannot read {}: {e}", settings_path.display()))?;
                serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
            } else {
                serde_json::json!({})
            };

            // Ensure settings is an object
            if !settings.is_object() {
                settings = serde_json::json!({});
            }

            let hooks = settings.as_object_mut().ok_or("settings is not an object")?
                .entry("hooks")
                .or_insert_with(|| serde_json::json!({}));

            let event_types = [
                "PreToolUse", "PostToolUse", "Notification",
                "SessionStart", "SessionEnd", "Stop",
                "UserPromptSubmit", "SubagentStart", "SubagentStop",
            ];

            for event_type in event_types {
                let hook_array = hooks.as_object_mut().ok_or("hooks is not an object")?
                    .entry(event_type)
                    .or_insert_with(|| serde_json::json!([]));

                if let Some(arr) = hook_array.as_array_mut() {
                    arr.retain(|entry| {
                        let has_our_hook = entry.get("hooks")
                            .and_then(|h| h.as_array())
                            .map(|hs| hs.iter().any(|h| {
                                h.get("command").and_then(|c| c.as_str())
                                    .is_some_and(|c| c.contains(&format!("localhost:{port}")))
                            }))
                            .unwrap_or(false);
                        !has_our_hook
                    });
                }

                let hook_cmd = format!(
                    r#"VK_TTY=""; if [ -n "${{TMUX:-}}" ]; then VK_TTY=$(tmux display-message -p '#{{client_tty}}' 2>/dev/null); else P=$PPID; while [ "$P" -gt 1 ] 2>/dev/null; do T=$(ps -p "$P" -o tty= 2>/dev/null | tr -d ' '); [ -n "$T" ] && [ "$T" != "??" ] && VK_TTY="/dev/$T"; P=$(ps -p "$P" -o ppid= 2>/dev/null | tr -d ' '); done; fi; export VK_TTY; jq -c '. + {{"type":"{event_type}","source":"claude-code","cwd":env.PWD,"bundle_id":(env.TERM_PROGRAM // ""),"session_tty":(env.VK_TTY // "")}}' | curl -s -X POST http://localhost:{port}/event -H 'Content-Type: application/json' -d @-"#,
                );

                let hook_entry = serde_json::json!({
                    "matcher": "",
                    "hooks": [{
                        "type": "command",
                        "command": hook_cmd,
                        "timeout": 5000
                    }]
                });
                if let Some(arr) = hook_array.as_array_mut() {
                    arr.push(hook_entry);
                }
                println!("[setup] Installed {event_type} hook");
            }

            if let Some(parent) = settings_path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let json = serde_json::to_string_pretty(&settings)
                .map_err(|e| format!("cannot serialize settings: {e}"))?;
            std::fs::write(&settings_path, &json)
                .map_err(|e| format!("cannot write {}: {e}", settings_path.display()))?;

            println!("[setup] Done! Restart Claude Code to activate hooks.");
            println!("[setup] Daemon endpoint: http://localhost:{port}/event");
            Ok(())
        }
        other => {
            Err(format!("Unknown tool: {other}. Supported: claude-code"))
        }
    }
}

#[cfg(test)]
fn execute_focus(session_id: u16, store: &SessionStore) -> String {
    match store.get(session_id) {
        Some(s) => {
            let result = focus::macos::activate_window(s);
            match result {
                Ok(()) => format!("[daemon] focused session #{session_id} ({})\n", s.info.name),
                Err(e) => format!("[daemon] focus failed for #{session_id}: {e}\n"),
            }
        }
        None => format!("[daemon] session #{session_id} not found\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_list_empty() {
        let store = SessionStore::new();
        let output = format_session_list(&store);
        assert!(output.contains("no active sessions"));
    }

    #[test]
    fn session_mock_and_list() {
        let mut store = SessionStore::new();
        let mock_output = inject_mock_sessions(&mut store);
        assert!(mock_output.contains("3 mock sessions"));

        let list_output = format_session_list(&store);
        assert!(list_output.contains("RustAgent"));
        assert!(list_output.contains("FrontEnd"));
        assert!(list_output.contains("DevOps"));
        assert!(list_output.contains("3 sessions"));
    }

    #[test]
    fn session_status_found() {
        let mut store = SessionStore::new();
        inject_mock_sessions(&mut store);
        let output = format_session_status(&store, 1);
        assert!(output.contains("RustAgent"));
        assert!(output.contains("Thinking"));
    }

    #[test]
    fn session_status_not_found() {
        let store = SessionStore::new();
        let output = format_session_status(&store, 999);
        assert!(output.contains("not found"));
    }

    #[test]
    fn focus_not_found() {
        let store = SessionStore::new();
        let output = execute_focus(999, &store);
        assert!(output.contains("not found"));
    }

    #[test]
    fn focus_no_window_info() {
        let mut store = SessionStore::new();
        inject_mock_sessions(&mut store);
        let output = execute_focus(1, &store);
        // No window info → focus should report failure or success depending on platform
        assert!(
            output.contains("focused") || output.contains("failed"),
            "should report focus result: {output}"
        );
    }
}
