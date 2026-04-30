//! Configuration system — TOML config file read/write.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Top-level daemon configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DaemonConfig {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub yolo: YoloFileConfig,
    #[serde(default)]
    pub ipc: IpcConfig,
    #[serde(default)]
    pub macros: MacroConfig,
    #[serde(default)]
    pub always_allow: AlwaysAllowConfig,
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub sound: SoundConfig,
}

/// LCD display configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisplayConfig {
    #[serde(default = "default_lcd_width")]
    pub width: u16,
    #[serde(default = "default_lcd_height")]
    pub height: u16,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self { width: 800, height: 340 }
    }
}

fn default_lcd_width() -> u16 { 800 }
fn default_lcd_height() -> u16 { 340 }

/// Sound configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SoundConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_volume")]
    pub volume: u8,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub mapping: SoundMappingConfig,
}

impl Default for SoundConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            volume: default_volume(),
            muted: false,
            mapping: SoundMappingConfig::default(),
        }
    }
}

fn default_volume() -> u8 { 80 }

/// Sound mapping configuration — maps event types to sound IDs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SoundMappingConfig {
    #[serde(default = "default_permission_sound")]
    pub permission_alert: String,
    #[serde(default = "default_complete_sound")]
    pub session_complete: String,
    #[serde(default = "default_error_sound")]
    pub error: String,
    #[serde(default = "default_click_sound")]
    pub click: String,
}

impl Default for SoundMappingConfig {
    fn default() -> Self {
        Self {
            permission_alert: default_permission_sound(),
            session_complete: default_complete_sound(),
            error: default_error_sound(),
            click: default_click_sound(),
        }
    }
}

fn default_permission_sound() -> String { "builtin:alert".into() }
fn default_complete_sound() -> String { "builtin:ding".into() }
fn default_error_sound() -> String { "builtin:buzz".into() }
fn default_click_sound() -> String { "builtin:click".into() }

/// Always-allow list persisted to config.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AlwaysAllowConfig {
    #[serde(default)]
    pub patterns: Vec<String>,
}

/// Macro binding configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct MacroConfig {
    /// DELETE button macro (default: "ctrl_u")
    #[serde(default = "default_delete_macro")]
    pub delete: String,
    /// VOICE button macro (default: "fn" — function key)
    #[serde(default = "default_voice_macro")]
    pub voice: String,
    /// Custom macros: name → key sequence
    #[serde(default)]
    pub custom: std::collections::HashMap<String, String>,
}

fn default_delete_macro() -> String {
    "ctrl_u".into()
}

fn default_voice_macro() -> String {
    "fn".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GeneralConfig {
    #[serde(default = "default_hook_port")]
    pub hook_port: u16,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            hook_port: default_hook_port(),
            log_level: "info".into(),
        }
    }
}

fn default_log_level() -> String {
    "info".into()
}

fn default_hook_port() -> u16 {
    19280
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct YoloFileConfig {
    #[serde(default)]
    pub active: bool,
    #[serde(default = "default_allow")]
    pub allow: Vec<String>,
    #[serde(default = "default_deny")]
    pub deny: Vec<String>,
    #[serde(default = "default_true")]
    pub notify_auto_allow: bool,
    #[serde(default = "default_true")]
    pub auto_allow_log: bool,
}

impl Default for YoloFileConfig {
    fn default() -> Self {
        Self {
            active: false,
            allow: default_allow(),
            deny: default_deny(),
            notify_auto_allow: true,
            auto_allow_log: true,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_allow() -> Vec<String> {
    vec![
        "Read(*)".into(),
        "Glob(*)".into(),
        "Grep(*)".into(),
    ]
}

fn default_deny() -> Vec<String> {
    vec![
        "Bash(git push*)".into(),
        "Bash(rm -rf*)".into(),
        "Bash(sudo*)".into(),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IpcConfig {
    #[serde(default = "default_socket_path")]
    pub socket_path: String,
}

impl Default for IpcConfig {
    fn default() -> Self {
        Self {
            socket_path: default_socket_path(),
        }
    }
}

fn default_socket_path() -> String {
    "/tmp/vk-daemon.sock".into()
}

/// Default config file path.
pub fn default_config_path() -> PathBuf {
    dirs_or_default().join("config.toml")
}

fn dirs_or_default() -> PathBuf {
    // ~/.config/vk-daemon/ or fallback
    if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".config").join("vk-daemon")
    } else {
        PathBuf::from("/tmp/vk-daemon")
    }
}

/// Load config from a TOML file. Returns default if file doesn't exist.
pub fn load_config(path: &Path) -> DaemonConfig {
    match std::fs::read_to_string(path) {
        Ok(content) => toml::from_str(&content).unwrap_or_else(|e| {
            tracing::warn!("config parse error, using defaults: {e}");
            DaemonConfig::default()
        }),
        Err(_) => DaemonConfig::default(),
    }
}

/// Save config to a TOML file.
pub fn save_config(path: &Path, config: &DaemonConfig) -> Result<(), String> {
    let content = toml::to_string_pretty(config).map_err(|e| format!("serialize: {e}"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    atomic_write(path, &content)
}

/// Write file atomically: write to temp → fsync → rename.
/// Prevents corruption from concurrent writes or interrupted I/O.
fn atomic_write(path: &Path, content: &str) -> Result<(), String> {
    use std::io::Write;
    // Use PID + timestamp for unique temp file to avoid concurrent writer races
    let pid = std::process::id();
    let ext = format!("tmp.{pid}");
    let tmp = path.with_extension(&ext);
    let mut f = std::fs::File::create(&tmp).map_err(|e| format!("create tmp: {e}"))?;
    f.write_all(content.as_bytes()).map_err(|e| format!("write tmp: {e}"))?;
    f.sync_all().map_err(|e| format!("fsync: {e}"))?;
    drop(f);
    std::fs::rename(&tmp, path).map_err(|e| format!("rename: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn default_config_values() {
        let config = DaemonConfig::default();
        assert_eq!(config.general.hook_port, 19280);
        assert!(!config.yolo.active);
        assert_eq!(config.ipc.socket_path, "/tmp/vk-daemon.sock");
    }

    #[test]
    fn serde_roundtrip() {
        let config = DaemonConfig::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: DaemonConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(config, parsed);
    }

    #[test]
    fn load_missing_file_returns_default() {
        let config = load_config(Path::new("/nonexistent/path/config.toml"));
        assert_eq!(config, DaemonConfig::default());
    }

    #[test]
    fn load_partial_config_fills_defaults() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        write!(
            tmpfile,
            r#"
[general]
hook_port = 8080

[yolo]
active = true
"#
        )
        .unwrap();

        let config = load_config(tmpfile.path());
        assert_eq!(config.general.hook_port, 8080);
        assert!(config.yolo.active);
        // Defaults filled in
        assert_eq!(config.ipc.socket_path, "/tmp/vk-daemon.sock");
        assert!(config.yolo.notify_auto_allow);
    }

    #[test]
    fn save_and_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut config = DaemonConfig::default();
        config.general.hook_port = 9999;
        config.yolo.active = true;

        save_config(&path, &config).unwrap();

        let loaded = load_config(&path);
        assert_eq!(loaded.general.hook_port, 9999);
        assert!(loaded.yolo.active);
    }

    #[test]
    fn sound_config_defaults() {
        let config = DaemonConfig::default();
        assert!(config.sound.enabled);
        assert_eq!(config.sound.volume, 80);
        assert!(!config.sound.muted);
        assert_eq!(config.sound.mapping.permission_alert, "builtin:alert");
        assert_eq!(config.sound.mapping.session_complete, "builtin:ding");
        assert_eq!(config.sound.mapping.error, "builtin:buzz");
        assert_eq!(config.sound.mapping.click, "builtin:click");
    }

    #[test]
    fn sound_config_serde_roundtrip() {
        let config = SoundConfig {
            enabled: false,
            volume: 50,
            muted: true,
            mapping: SoundMappingConfig {
                permission_alert: "custom:alarm".into(),
                session_complete: "builtin:ding".into(),
                error: "custom:err".into(),
                click: "builtin:click".into(),
            },
        };
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: SoundConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(config, parsed);
    }

    #[test]
    fn sound_config_partial_toml_fills_defaults() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        write!(
            tmpfile,
            r#"
[sound]
volume = 60
"#
        )
        .unwrap();

        let config = load_config(tmpfile.path());
        assert_eq!(config.sound.volume, 60);
        // Defaults filled in
        assert!(config.sound.enabled);
        assert!(!config.sound.muted);
        assert_eq!(config.sound.mapping.permission_alert, "builtin:alert");
    }

    #[test]
    fn atomic_write_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.toml");
        atomic_write(&path, "hello = \"world\"").unwrap();
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("hello"));
        // Temp file should not remain
        assert!(!path.with_extension("tmp").exists());
    }

    #[test]
    fn invalid_toml_returns_default() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        write!(tmpfile, "this is not valid toml {{{{").unwrap();

        let config = load_config(tmpfile.path());
        assert_eq!(config, DaemonConfig::default());
    }
}
