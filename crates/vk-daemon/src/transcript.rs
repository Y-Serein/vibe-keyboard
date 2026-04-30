//! JSONL transcript parser — extracts model/tokens/cost from Claude Code transcripts.
//!
//! Real path: ~/.claude/projects/{project-path-hash}/{session-uuid}.jsonl
//! Real format: {"type":"assistant","message":{"model":"claude-opus-4-6"},"usage":{"input_tokens":N,"output_tokens":N}}

use std::io::{BufRead, Seek, SeekFrom};
use std::path::PathBuf;
use std::time::SystemTime;

// Re-export DiscoveredSession from discovery module for backward compatibility.
pub use crate::discovery::DiscoveredSession;

/// Model pricing table (per 1M tokens): (name_contains, input_price, output_price).
const PRICING: &[(&str, f64, f64)] = &[
    ("opus-4", 15.0, 75.0),
    ("sonnet-4", 3.0, 15.0),
    ("haiku-4", 0.80, 4.0),
    ("claude-3-5-sonnet", 3.0, 15.0),
    ("claude-3-opus", 15.0, 75.0),
];

/// Extracted transcript data.
#[derive(Debug, Clone, Default)]
pub struct TranscriptData {
    pub model: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cost_usd: f64,
    pub context_pct: u8,
    /// User's last human input
    pub last_message: String,
    /// AI's last output
    pub last_ai_output: String,
    /// Inferred status from last entry type
    pub inferred_status: String,
}

/// Incremental file scanner.
#[derive(Debug)]
pub struct FileOffset {
    pub path: PathBuf,
    pub offset: u64,
    pub last_modified: Option<SystemTime>,
    pub data: TranscriptData,
}

impl FileOffset {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            offset: 0,
            last_modified: None,
            data: TranscriptData::default(),
        }
    }

    /// Scan for new lines since last read. Returns true if data changed.
    pub fn scan(&mut self) -> bool {
        let metadata = match std::fs::metadata(&self.path) {
            Ok(m) => m,
            Err(_) => return false,
        };

        let modified = metadata.modified().ok();
        let file_len = metadata.len();
        if file_len < self.offset {
            self.offset = 0;
        }
        if modified == self.last_modified && self.offset > 0 && file_len == self.offset {
            return false;
        }

        let file = match std::fs::File::open(&self.path) {
            Ok(f) => f,
            Err(_) => return false,
        };

        let mut reader = std::io::BufReader::new(file);

        // First read of large file: skip to last 4MB (need enough to find human messages)
        if self.offset == 0 && file_len > 4 * 1024 * 1024 {
            let start = file_len - 4 * 1024 * 1024;
            reader.seek(SeekFrom::Start(start)).ok();
            let mut discard = String::new();
            reader.read_line(&mut discard).ok();
            self.offset = start + discard.len() as u64;
        } else if self.offset > 0 {
            reader.seek(SeekFrom::Start(self.offset)).ok();
        }

        let mut changed = false;
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(n) => {
                    self.offset += n as u64;
                    if let Some(parsed) = parse_jsonl_line(&line) {
                        self.apply_parsed(&parsed);
                        changed = true;
                    }
                }
                Err(_) => break,
            }
        }

        self.last_modified = modified;
        changed
    }

    fn apply_parsed(&mut self, entry: &JsonlEntry) {
        match entry {
            JsonlEntry::Assistant { model, usage, content } => {
                if let Some(m) = model {
                    self.data.model = m.clone();
                }
                // Infer status + capture AI output for display
                if let Some(c) = content {
                    self.data.inferred_status = "writing".into();
                    // Store AI's last output (truncated, ASCII-safe)
                    let truncated: String = c.chars().take(120).collect();
                    self.data.last_ai_output = truncated;
                }
                if let Some(u) = usage {
                    self.data.tokens_in += u.input_tokens;
                    self.data.tokens_out += u.output_tokens;
                    self.data.cost_usd = calculate_cost(
                        &self.data.model,
                        self.data.tokens_in,
                        self.data.tokens_out,
                    );
                    let ctx_window = context_window(&self.data.model);
                    self.data.context_pct = std::cmp::min(
                        100,
                        ((u.input_tokens as f64 / ctx_window as f64) * 100.0).round() as u8,
                    );
                }
            }
            JsonlEntry::Human { content } => {
                let truncated: String = content.chars().take(120).collect();
                self.data.last_message = truncated;
                self.data.inferred_status = "thinking".into(); // user sent → AI thinking
            }
        }
    }
}

#[derive(Debug)]
enum JsonlEntry {
    Assistant {
        model: Option<String>,
        usage: Option<UsageData>,
        content: Option<String>,
    },
    Human {
        content: String,
    },
}

#[derive(Debug)]
struct UsageData {
    input_tokens: u64,
    output_tokens: u64,
}

/// Parse a single JSONL line from a real Claude Code transcript.
///
/// Real format variants:
/// - `{"type":"assistant","message":{"model":"...","content":[...]},"usage":{"input_tokens":N,...}}`
/// - Older: `{"type":"assistant","model":"...","usage":{"input_tokens":N,...}}`
fn parse_jsonl_line(line: &str) -> Option<JsonlEntry> {
    let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    let obj = v.as_object()?;

    let typ = obj.get("type")?.as_str()?;

    // Parse user/human messages for last_message display
    // Claude Code uses "user", older format uses "human"
    if typ == "user" || typ == "human" {
        let content = obj.get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| {
                if let Some(s) = c.as_str() { return Some(s.to_string()); }
                if let Some(arr) = c.as_array() {
                    return arr.iter().find_map(|b| {
                        if b.get("type")?.as_str()? == "text" {
                            b.get("text")?.as_str().map(|s| s.to_string())
                        } else { None }
                    });
                }
                None
            })
            .or_else(|| obj.get("content").and_then(|v| v.as_str()).map(|s| s.to_string()));
        if let Some(c) = content {
            if !c.is_empty() {
                return Some(JsonlEntry::Human { content: c });
            }
        }
        return None;
    }

    if typ != "assistant" {
        return None;
    }

    // Model: try message.model first, then top-level model
    let model = obj
        .get("message")
        .and_then(|m| m.get("model"))
        .or_else(|| obj.get("model"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Usage: try top-level usage first (newer format), then message.usage
    let usage_val = obj.get("usage").or_else(|| {
        obj.get("message").and_then(|m| m.get("usage"))
    });
    let usage = usage_val.and_then(|u| {
        let input_tokens = u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0)
            + u.get("cache_read_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
        let output_tokens = u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
        if input_tokens > 0 || output_tokens > 0 {
            Some(UsageData { input_tokens, output_tokens })
        } else {
            None
        }
    });

    // Content: try message.content (array of blocks) or top-level content
    let content = obj
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| {
            if let Some(arr) = c.as_array() {
                // Find first text block
                arr.iter()
                    .find_map(|block| {
                        if block.get("type")?.as_str()? == "text" {
                            block.get("text")?.as_str().map(|s| s.to_string())
                        } else {
                            None
                        }
                    })
            } else {
                c.as_str().map(|s| s.to_string())
            }
        })
        .or_else(|| obj.get("content").and_then(|v| v.as_str()).map(|s| s.to_string()));

    Some(JsonlEntry::Assistant { model, usage, content })
}

/// Context window size per model (from SC cost.rs).
fn context_window(model: &str) -> u64 {
    let m = model.to_lowercase();
    if m.contains("opus") {
        1_000_000
    } else if m.contains("sonnet") {
        200_000
    } else if m.contains("haiku") {
        200_000
    } else {
        200_000
    }
}

fn calculate_cost(model: &str, tokens_in: u64, tokens_out: u64) -> f64 {
    let (input_price, output_price) = PRICING
        .iter()
        .find(|(name, _, _)| model.contains(name))
        .map(|(_, i, o)| (*i, *o))
        .unwrap_or((3.0, 15.0));

    (tokens_in as f64 * input_price / 1_000_000.0)
        + (tokens_out as f64 * output_price / 1_000_000.0)
}

/// Find transcript file for a session. Real path format:
/// ~/.claude/projects/{project-path-hash}/{session-uuid}.jsonl
pub fn find_transcript_path(session_id: &str) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let projects_dir = home.join(".claude").join("projects");

    if !projects_dir.exists() {
        return None;
    }

    // Search all project directories for {session_id}.jsonl
    for entry in std::fs::read_dir(&projects_dir).ok()? {
        let entry = entry.ok()?;
        if !entry.file_type().ok()?.is_dir() {
            continue;
        }
        let transcript = entry.path().join(format!("{session_id}.jsonl"));
        if transcript.exists() {
            return Some(transcript);
        }
    }

    None
}

/// Scan ~/.claude/projects for active sessions (modified within last 24 hours).
///
/// This delegates to [`crate::discovery::FilesystemDiscovery`] — the canonical
/// implementation now lives in `discovery.rs`.
pub fn discover_active_sessions() -> Vec<DiscoveredSession> {
    use crate::discovery::{FilesystemDiscovery, SessionDiscovery};
    FilesystemDiscovery.discover()
}

/// Validate that a transcript path is safe and well-formed.
///
/// Requirements:
/// - Must be an absolute path (starts with `/`)
/// - Must be under ~/.claude/projects/ (canonicalized)
/// - Must end with `.jsonl` extension
/// - Must be a regular file (not FIFO, device, symlink to outside)
pub fn validate_transcript_path(path: &str) -> bool {
    let p = std::path::Path::new(path);
    if !p.is_absolute() { return false; }
    let canonical = match p.canonicalize() {
        Ok(c) => c,
        Err(_) => return false,
    };
    let home = dirs::home_dir().unwrap_or_default();
    let claude_dir = home.join(".claude").join("projects");
    let canonical_claude = claude_dir.canonicalize().unwrap_or(claude_dir);
    if !canonical.starts_with(&canonical_claude) { return false; }
    if canonical.extension().map_or(true, |e| e != "jsonl") { return false; }
    if !canonical.is_file() { return false; }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parse_real_assistant_line() {
        // Real Claude Code format
        let line = r#"{"type":"assistant","message":{"model":"claude-opus-4-6","content":[{"type":"text","text":"I will help"}]},"usage":{"input_tokens":1000,"cache_read_input_tokens":5000,"output_tokens":200}}"#;
        match parse_jsonl_line(line).unwrap() {
            JsonlEntry::Assistant { model, usage, content } => {
                assert_eq!(model.unwrap(), "claude-opus-4-6");
                let u = usage.unwrap();
                assert_eq!(u.input_tokens, 6000); // 1000 + 5000 cache_read
                assert_eq!(u.output_tokens, 200);
                assert_eq!(content.unwrap(), "I will help");
            }
            _ => panic!("expected Assistant"),
        }
    }

    #[test]
    fn parse_assistant_no_content() {
        let line = r#"{"type":"assistant","model":"claude-sonnet-4-6","usage":{"input_tokens":500,"output_tokens":100}}"#;
        match parse_jsonl_line(line).unwrap() {
            JsonlEntry::Assistant { model, usage, content } => {
                assert_eq!(model.unwrap(), "claude-sonnet-4-6");
                assert!(usage.is_some());
                assert!(content.is_none());
            }
            _ => panic!("expected Assistant"),
        }
    }

    #[test]
    fn parse_user_message() {
        // "type":"user" now parses as Human
        let entry = parse_jsonl_line(r#"{"type":"user","message":{"content":"hello world"}}"#);
        assert!(matches!(entry, Some(JsonlEntry::Human { .. })));
    }

    #[test]
    fn parse_non_assistant_returns_none() {
        assert!(parse_jsonl_line(r#"{"type":"file-history-snapshot"}"#).is_none());
        assert!(parse_jsonl_line(r#"{"type":"progress"}"#).is_none());
    }

    #[test]
    fn calculate_cost_opus() {
        let cost = calculate_cost("claude-opus-4-6", 1_000_000, 100_000);
        assert!((cost - 22.5).abs() < 0.01);
    }

    #[test]
    fn file_offset_scan_real_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("abc-123.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, r#"{{"type":"human","message":{{"content":"hello world"}}}}"#).unwrap();
        writeln!(f, r#"{{"type":"assistant","message":{{"model":"claude-opus-4-6","content":[{{"type":"text","text":"Hi there"}}]}},"usage":{{"input_tokens":100,"output_tokens":50}}}}"#).unwrap();
        writeln!(f, r#"{{"type":"human","message":{{"content":"do something"}}}}"#).unwrap();
        writeln!(f, r#"{{"type":"assistant","message":{{"model":"claude-opus-4-6","content":[{{"type":"text","text":"Done"}}]}},"usage":{{"input_tokens":200,"cache_read_input_tokens":300,"output_tokens":80}}}}"#).unwrap();

        let mut scanner = FileOffset::new(path);
        assert!(scanner.scan());
        assert_eq!(scanner.data.model, "claude-opus-4-6");
        assert_eq!(scanner.data.tokens_in, 600); // 100 + 200+300
        assert_eq!(scanner.data.tokens_out, 130); // 50 + 80
        assert!(scanner.data.cost_usd > 0.0);
        assert_eq!(scanner.data.last_message, "do something"); // human's last input
    }

    #[test]
    fn file_offset_incremental() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, r#"{{"type":"assistant","model":"claude-opus-4-6","usage":{{"input_tokens":100,"output_tokens":50}}}}"#).unwrap();
        }

        let mut scanner = FileOffset::new(path.clone());
        scanner.scan();
        assert_eq!(scanner.data.tokens_in, 100);

        {
            let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
            writeln!(f, r#"{{"type":"assistant","model":"claude-opus-4-6","usage":{{"input_tokens":200,"output_tokens":100}}}}"#).unwrap();
        }

        assert!(scanner.scan());
        assert_eq!(scanner.data.tokens_in, 300); // 100 + 200
        assert_eq!(scanner.data.tokens_out, 150); // 50 + 100
    }
}
