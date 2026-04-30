//! Vibe Keyboard Tauri backend — polls vk-daemon HTTP API and bridges to React frontend.

use serde::{Deserialize, Serialize};
use tauri::Emitter;
use vk_core::sounds;

const DEFAULT_DAEMON_URL: &str = "http://localhost:19280";

/// Session info received from daemon and sent to frontend.
#[derive(Serialize, Deserialize, Clone, Debug)]
struct SessionInfoJs {
    id: u16,
    name: String,
    status: String,
    has_permission: bool,
    #[serde(default)]
    source: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    cwd: String,
    #[serde(default)]
    tokens_in: u64,
    #[serde(default)]
    tokens_out: u64,
    #[serde(default)]
    cost_usd: f64,
    #[serde(default)]
    context_pct: u8,
}

// ── Tauri Commands ──

#[tauri::command]
async fn button_press(id: String, action: Option<String>) -> Result<String, String> {
    let client = reqwest::Client::new();
    let mut body = serde_json::json!({ "id": id });
    if let Some(a) = action {
        body["action"] = serde_json::json!(a);
    }
    let resp = client
        .post(format!("{DEFAULT_DAEMON_URL}/button"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    if resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        Ok(body)
    } else {
        Err(format!("daemon returned {}", resp.status()))
    }
}

#[tauri::command]
async fn knob_action(action: String) -> Result<String, String> {
    let client = reqwest::Client::new();
    let body = serde_json::json!({ "action": action, "steps": 1 });
    let resp = client
        .post(format!("{DEFAULT_DAEMON_URL}/knob"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    if resp.status().is_success() {
        Ok(format!("knob {action}"))
    } else {
        Err(format!("daemon returned {}", resp.status()))
    }
}

#[tauri::command]
async fn get_config() -> Result<serde_json::Value, String> {
    let resp = reqwest::get(format!("{DEFAULT_DAEMON_URL}/config"))
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    let val: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("parse failed: {e}"))?;
    Ok(val)
}

#[tauri::command]
async fn set_config(key: String, value: String) -> Result<String, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{DEFAULT_DAEMON_URL}/config"))
        .json(&serde_json::json!({ "key": key, "value": value }))
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    if resp.status().is_success() {
        Ok("ok".into())
    } else {
        Err(format!("daemon returned {}", resp.status()))
    }
}

#[tauri::command]
async fn get_activity_log() -> Result<Vec<String>, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{DEFAULT_DAEMON_URL}/log"))
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    let data: Vec<String> = resp.json().await.map_err(|e| format!("parse failed: {e}"))?;
    Ok(data)
}

#[tauri::command]
async fn install_hooks() -> Result<String, String> {
    // Run vk-daemon setup to install hooks
    let output = std::process::Command::new("cargo")
        .args(["run", "-p", "vk-daemon", "--", "setup"])
        .current_dir(env!("CARGO_MANIFEST_DIR").to_string() + "/..")
        .output()
        .map_err(|e| format!("failed to run setup: {e}"))?;
    if output.status.success() {
        Ok("Hooks installed".into())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("setup failed: {stderr}"))
    }
}

#[tauri::command]
async fn open_accessibility_settings() -> Result<String, String> {
    std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn()
        .map_err(|e| format!("failed: {e}"))?;
    Ok("opened".into())
}

#[tauri::command]
async fn get_setup_status() -> Result<serde_json::Value, String> {
    let resp = reqwest::get(format!("{DEFAULT_DAEMON_URL}/setup/status"))
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    let val: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("parse failed: {e}"))?;
    Ok(val)
}

#[tauri::command]
async fn setup_install(tool_id: String) -> Result<String, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{DEFAULT_DAEMON_URL}/setup/install/{tool_id}"))
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    if resp.status().is_success() {
        Ok("ok".into())
    } else {
        let body = resp.text().await.unwrap_or_default();
        Err(format!("install failed: {body}"))
    }
}

#[tauri::command]
async fn setup_uninstall(tool_id: String) -> Result<String, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{DEFAULT_DAEMON_URL}/setup/uninstall/{tool_id}"))
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    if resp.status().is_success() {
        Ok("ok".into())
    } else {
        let body = resp.text().await.unwrap_or_default();
        Err(format!("uninstall failed: {body}"))
    }
}

#[tauri::command]
async fn brew_install(package: String) -> Result<String, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{DEFAULT_DAEMON_URL}/setup/brew-install/{package}"))
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    if resp.status().is_success() {
        Ok("ok".into())
    } else {
        let body = resp.text().await.unwrap_or_default();
        Err(format!("brew install failed: {body}"))
    }
}

#[tauri::command]
async fn brew_uninstall(package: String) -> Result<String, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{DEFAULT_DAEMON_URL}/setup/brew-uninstall/{package}"))
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    if resp.status().is_success() {
        Ok("ok".into())
    } else {
        let body = resp.text().await.unwrap_or_default();
        Err(format!("brew uninstall failed: {body}"))
    }
}

#[tauri::command]
async fn get_sounds() -> Result<serde_json::Value, String> {
    let resp = reqwest::get(format!("{DEFAULT_DAEMON_URL}/sounds"))
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    let val: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("parse failed: {e}"))?;
    Ok(val)
}

/// Resolve a sound_id (like "builtin:alert" or "builtin:ding") to embedded WAV data.
/// Also accepts legacy event keys ("permission_alert", "error", etc.) for backward compat.
fn resolve_sound_wav(sound_id: &str) -> Option<&'static [u8]> {
    match sound_id {
        // Sound IDs (from dropdown mapping)
        "builtin:alert" => Some(sounds::WAV_PERMISSION_ALERT),
        "builtin:ding" => Some(sounds::WAV_SESSION_COMPLETE),
        "builtin:buzz" => Some(sounds::WAV_ERROR),
        "builtin:click" => Some(sounds::WAV_CLICK),
        "builtin:none" => None, // Silent
        // Legacy event keys (backward compat)
        "permission_alert" => Some(sounds::WAV_PERMISSION_ALERT),
        "session_complete" => Some(sounds::WAV_SESSION_COMPLETE),
        "error" => Some(sounds::WAV_ERROR),
        "click" => Some(sounds::WAV_CLICK),
        // Custom sounds — load from disk
        s if s.starts_with("custom:") => {
            // Custom sounds can't be static, skip for now (played via daemon)
            None
        }
        _ => None,
    }
}

#[tauri::command]
async fn play_sound(sound_type: String) -> Result<String, String> {
    // Resolve sound_id to WAV data (supports "builtin:alert" and legacy "permission_alert")
    let wav_data = resolve_sound_wav(&sound_type);

    // Also forward to daemon for device playback (best-effort).
    let st = sound_type.clone();
    tokio::spawn(async move {
        let _ = reqwest::Client::new()
            .post(format!("{DEFAULT_DAEMON_URL}/sounds/play"))
            .json(&serde_json::json!({ "sound_type": st }))
            .send()
            .await;
    });

    // Play locally if we have WAV data.
    let Some(wav) = wav_data else {
        return Ok("silent or custom".into());
    };

    let data = wav.to_vec();
    tokio::task::spawn_blocking(move || {
        let (_stream, handle) = rodio::OutputStream::try_default()
            .map_err(|e| format!("audio init failed: {e}"))?;
        let sink = rodio::Sink::try_new(&handle)
            .map_err(|e| format!("sink failed: {e}"))?;
        let cursor = std::io::Cursor::new(data);
        let source = rodio::Decoder::new(cursor)
            .map_err(|e| format!("decode failed: {e}"))?;
        sink.append(source);
        sink.sleep_until_end();
        Ok::<_, String>("ok".into())
    })
    .await
    .map_err(|e| format!("spawn failed: {e}"))?
}

#[tauri::command]
async fn upload_sound(filename: String, data: Vec<u8>) -> Result<String, String> {
    // Validate WAV header
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err("Invalid WAV file".into());
    }
    if data.len() > 512_000 {
        return Err("File too large (max 500KB)".into());
    }

    // Save to custom sounds dir
    let sounds_dir = dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".config"))
        .join("vk-daemon/sounds/custom");
    std::fs::create_dir_all(&sounds_dir).map_err(|e| format!("mkdir: {e}"))?;

    let safe_name = filename.replace(['/', '\\', ':', '?', '*', '"', '<', '>', '|'], "_");
    let dest = sounds_dir.join(&safe_name);
    std::fs::write(&dest, &data).map_err(|e| format!("write: {e}"))?;

    Ok("ok".into())
}

/// Background task: poll daemon /sessions every 500ms and /health every 1s.
async fn poll_daemon(app: tauri::AppHandle) {
    let client = reqwest::Client::new();
    let mut tick: u64 = 0;

    loop {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        tick += 1;

        // Check health every 1s
        if tick.is_multiple_of(2) {
            let healthy = client
                .get(format!("{DEFAULT_DAEMON_URL}/health"))
                .send()
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false);

            let _ = app.emit("connection-status", healthy);

            if !healthy {
                let _ = app.emit(
                    "session-update",
                    serde_json::json!({ "sessions": Vec::<SessionInfoJs>::new(), "active_index": 0 }),
                );
                continue;
            }
        }

        // Fetch sessions every 500ms
        match client.get(format!("{DEFAULT_DAEMON_URL}/sessions")).send().await {
            Ok(resp) => {
                if let Ok(sessions) = resp.json::<Vec<SessionInfoJs>>().await {
                    // Also fetch YOLO state
                    let yolo_active = async {
                        let r = client.get(format!("{DEFAULT_DAEMON_URL}/yolo")).send().await.ok()?;
                        let v: serde_json::Value = r.json().await.ok()?;
                        v.get("active")?.as_bool()
                    }.await.unwrap_or(false);
                    let _ = app.emit(
                        "session-update",
                        serde_json::json!({ "sessions": sessions, "active_index": 0, "yolo_active": yolo_active }),
                    );
                }
            }
            Err(e) => {
                tracing::warn!("failed to fetch sessions: {e}");
            }
        }

        // Fetch LCD frame every 500ms (~2fps for GUI, sufficient for status display)
        match client.get(format!("{DEFAULT_DAEMON_URL}/frame")).send().await {
            Ok(resp) => {
                let width: u16 = resp.headers()
                    .get("X-LCD-Width")
                    .and_then(|v| v.to_str().ok()?.parse().ok())
                    .unwrap_or(800);
                let height: u16 = resp.headers()
                    .get("X-LCD-Height")
                    .and_then(|v| v.to_str().ok()?.parse().ok())
                    .unwrap_or(340);
                if let Ok(bytes) = resp.bytes().await {
                    use base64::Engine;
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                    let _ = app.emit("frame-update", serde_json::json!({
                        "width": width,
                        "height": height,
                        "pixels_b64": b64
                    }));
                }
            }
            Err(_) => {}
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let _ = app.emit("connection-status", false);
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(poll_daemon(handle));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            button_press,
            knob_action,
            get_config,
            set_config,
            install_hooks,
            open_accessibility_settings,
            get_activity_log,
            get_setup_status,
            setup_install,
            setup_uninstall,
            brew_install,
            brew_uninstall,
            get_sounds,
            play_sound,
            upload_sound,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    #[test]
    fn daemon_url_is_valid() {
        let url = super::DEFAULT_DAEMON_URL;
        assert!(url.starts_with("http://"));
        assert!(url.contains("19280"));
    }
}
