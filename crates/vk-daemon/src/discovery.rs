//! Session discovery trait + filesystem backend.
//!
//! Provides a pluggable [`SessionDiscovery`] trait with a built-in implementation:
//! - [`FilesystemDiscovery`]: scans `~/.claude/projects/*/` for recent JSONL transcripts

use std::path::PathBuf;
use std::time::SystemTime;

/// A discovered session from any discovery source.
#[derive(Debug, Clone)]
pub struct DiscoveredSession {
    /// Hook-style session ID or filesystem-derived UUID.
    pub session_id: String,
    /// Project directory name (the hash segment under `~/.claude/projects/`).
    pub project_dir: String,
    /// Full path to the JSONL transcript file (may be empty for process-only discoveries).
    pub transcript_path: PathBuf,
    /// Working directory of the session.
    pub cwd: String,
}

/// Trait for session discovery backends.
pub trait SessionDiscovery: Send + Sync {
    /// Discover currently active sessions.
    fn discover(&self) -> Vec<DiscoveredSession>;
    /// Discovery source name for logging.
    fn name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// FilesystemDiscovery
// ---------------------------------------------------------------------------

/// Discovers sessions by scanning `~/.claude/projects/*/` for JSONL files
/// modified within the last 24 hours.
pub struct FilesystemDiscovery;

impl SessionDiscovery for FilesystemDiscovery {
    fn discover(&self) -> Vec<DiscoveredSession> {
        let home = match dirs::home_dir() {
            Some(h) => h,
            None => return vec![],
        };
        let projects_dir = home.join(".claude").join("projects");
        if !projects_dir.exists() {
            return vec![];
        }

        let mut sessions = Vec::new();
        let cutoff = SystemTime::now()
            .checked_sub(std::time::Duration::from_secs(24 * 3600))
            .unwrap_or(SystemTime::UNIX_EPOCH);

        if let Ok(entries) = std::fs::read_dir(&projects_dir) {
            for entry in entries.flatten() {
                if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    continue;
                }
                let project_dir = entry.path();
                let project_name = entry.file_name().to_string_lossy().to_string();

                let cwd = resolve_cwd_from_project_hash(&project_name);

                if let Ok(files) = std::fs::read_dir(&project_dir) {
                    for file in files.flatten() {
                        let path = file.path();
                        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                            continue;
                        }
                        let modified = file
                            .metadata()
                            .ok()
                            .and_then(|m| m.modified().ok())
                            .unwrap_or(SystemTime::UNIX_EPOCH);
                        if modified < cutoff {
                            continue;
                        }

                        let session_id = path
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default();

                        // Skip subagent directories
                        if session_id == "subagents" {
                            continue;
                        }

                        sessions.push(DiscoveredSession {
                            session_id,
                            project_dir: project_name.clone(),
                            transcript_path: path,
                            cwd: cwd.clone(),
                        });
                    }
                }
            }
        }

        // Sort by modification time (most recent first)
        sessions.sort_by(|a, b| {
            let ma = std::fs::metadata(&a.transcript_path)
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let mb = std::fs::metadata(&b.transcript_path)
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            mb.cmp(&ma)
        });

        sessions
    }

    fn name(&self) -> &str {
        "filesystem"
    }
}

// ---------------------------------------------------------------------------
// CWD resolver (moved from transcript.rs)
// ---------------------------------------------------------------------------

/// Resolve CWD from Claude Code project hash directory name.
/// Hash format: `-Users-hondachen-codes-ai--kvm-vibe--keyboard`
/// Each `/` in the original path became `-`. Hyphens in dir names are preserved.
/// Strategy: greedily build path segments, checking filesystem existence.
pub(crate) fn resolve_cwd_from_project_hash(hash: &str) -> String {
    let s = hash.strip_prefix('-').unwrap_or(hash);
    let parts: Vec<&str> = s.split('-').collect();
    if parts.is_empty() {
        return format!("/{}", s.replace('-', "/"));
    }

    let mut path = String::from("/");
    let mut i = 0;
    while i < parts.len() {
        let mut best_j = i + 1;
        let mut j = parts.len();
        while j > i + 1 {
            let candidate_segment = parts[i..j].join("-");
            let candidate_path = format!("{}{}", path, candidate_segment);
            if std::path::Path::new(&candidate_path).exists() {
                best_j = j;
                break;
            }
            j -= 1;
        }
        let segment = parts[i..best_j].join("-");
        path.push_str(&segment);
        i = best_j;
        if i < parts.len() {
            path.push('/');
        }
    }
    path
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filesystem_discovery_name() {
        let d = FilesystemDiscovery;
        assert_eq!(d.name(), "filesystem");
    }

    #[test]
    fn filesystem_discovery_runs_without_panic() {
        let d = FilesystemDiscovery;
        let _sessions = d.discover(); // may return empty on CI
    }

    #[test]
    fn discovered_session_struct_creation() {
        let ds = DiscoveredSession {
            session_id: "abc-123".into(),
            project_dir: "-Users-test".into(),
            transcript_path: PathBuf::from("/tmp/test.jsonl"),
            cwd: "/Users/test".into(),
        };
        assert_eq!(ds.session_id, "abc-123");
        assert_eq!(ds.cwd, "/Users/test");
        assert_eq!(ds.transcript_path, PathBuf::from("/tmp/test.jsonl"));
    }

    #[test]
    fn resolve_cwd_simple() {
        // Without real filesystem paths, it falls back to single-segment splits
        let result = resolve_cwd_from_project_hash("-tmp-test");
        // Should at least start with '/'
        assert!(result.starts_with('/'));
    }
}
