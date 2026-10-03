//! Spawns `sf` for a [`ReadOnlyCommand`] (SPEC §7.3).
//!
//! Blocking API: frontends call it from worker threads (the egui app will wrap
//! it in its background runtime). No shell is ever involved.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::command::{ArgvError, ReadOnlyCommand};
use crate::manifest::Manifest;

/// Env vars always set for the child (§7.3).
pub const CHILD_ENV: &[(&str, &str)] = &[
    ("NO_COLOR", "1"),
    ("SF_SKIP_NEW_VERSION_CHECK", "true"),
    ("SF_AUTOUPDATE_DISABLE", "true"),
];

/// Env vars always removed from the child (F25: would un-redact secrets).
pub const CHILD_ENV_REMOVE: &[&str] = &["SF_TEMP_SHOW_SECRETS"];

/// Runner settings.
#[derive(Debug, Clone)]
pub struct RunnerConfig {
    pub bin: PathBuf,
    pub timeout: Duration,
    /// Max bytes kept per stream (§4.6: 64 MB).
    pub max_output: usize,
    /// `SF_ORG_MAX_QUERY_LIMIT` for the child (F10), if set.
    pub max_query_limit: Option<u32>,
}

impl RunnerConfig {
    pub fn new(bin: PathBuf) -> Self {
        Self {
            bin,
            timeout: Duration::from_secs(120),
            max_output: 64 * 1024 * 1024,
            max_query_limit: None,
        }
    }
}

/// Result of one `sf` invocation.
#[derive(Debug, Clone)]
pub struct RunOutput {
    /// argv as passed to `sf` (validated, secret-free).
    pub argv: Vec<String>,
    /// Process exit code; `None` if killed.
    pub exit_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub duration: Duration,
    pub timed_out: bool,
    pub cancelled: bool,
    pub truncated: bool,
}

/// Why a command could not be run at all.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error(transparent)]
    Argv(#[from] ArgvError),
    #[error("could not start {bin}: {source}")]
    Spawn {
        bin: String,
        #[source]
        source: std::io::Error,
    },
    #[error("i/o error while running sf: {0}")]
    Io(#[from] std::io::Error),
}

/// Where the `sf` binary was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinSource {
    Explicit,
    EnvVar,
    Path,
}

/// `sf` not found (D1, AC-07).
#[derive(Debug, Clone, thiserror::Error)]
#[error("Salesforce CLI (sf) not found: {0}. Install it with: {install}", install = crate::CLI_INSTALL_COMMAND)]
pub struct CliMissing(pub String);

/// Resolves the sf binary: explicit → `$GP_ATLAS_SF_BIN` → `sf` on PATH (§4.1).
pub fn resolve_sf_bin(explicit: Option<&Path>) -> Result<(PathBuf, BinSource), CliMissing> {
    if let Some(p) = explicit {
        return Ok((p.to_path_buf(), BinSource::Explicit));
    }
    if let Some(p) = std::env::var_os("GP_ATLAS_SF_BIN").filter(|p| !p.is_empty()) {
        return Ok((PathBuf::from(p), BinSource::EnvVar));
    }
    which::which("sf")
        .map(|p| (p, BinSource::Path))
        .map_err(|e| CliMissing(e.to_string()))
}

/// Runs read-only sf commands.
#[derive(Debug, Clone)]
pub struct SfRunner {
    pub cfg: RunnerConfig,
}

impl SfRunner {
    pub fn new(cfg: RunnerConfig) -> Self {
        Self { cfg }
    }

    /// Runs `cmd`. `cwd` should be set only for project-alias runs (§7.3).
    pub fn run(
        &self,
        cmd: &ReadOnlyCommand,
        manifest: &Manifest,
        cwd: Option<&Path>,
        cancel: Option<&AtomicBool>,
    ) -> Result<RunOutput, RunError> {
        let argv = cmd.argv(manifest)?;
        self.run_argv(argv, cwd, cancel)
    }

    fn run_argv(
        &self,
        argv: Vec<String>,
        cwd: Option<&Path>,
        cancel: Option<&AtomicBool>,
    ) -> Result<RunOutput, RunError> {
        let mut command = Command::new(&self.cfg.bin);
        command
            .args(&argv)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in CHILD_ENV {
            command.env(k, v);
        }
        for k in CHILD_ENV_REMOVE {
            command.env_remove(k);
        }
        if let Some(limit) = self.cfg.max_query_limit {
            command.env("SF_ORG_MAX_QUERY_LIMIT", limit.to_string());
        }
        if let Some(dir) = cwd {
            command.current_dir(dir);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            // Own process group so a timeout can kill the whole tree.
            command.process_group(0);
        }

        let start = Instant::now();
        let mut child = command.spawn().map_err(|source| RunError::Spawn {
            bin: self.cfg.bin.display().to_string(),
            source,
        })?;

        let truncated = Arc::new(AtomicBool::new(false));
        let out = reader(child.stdout.take(), self.cfg.max_output, truncated.clone());
        let err = reader(child.stderr.take(), self.cfg.max_output, truncated.clone());

        let deadline = start + self.cfg.timeout;
        let mut timed_out = false;
        let mut cancelled = false;
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break Some(status);
            }
            if Instant::now() >= deadline {
                timed_out = true;
            } else if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
                cancelled = true;
            }
            if timed_out || cancelled {
                kill_tree(&mut child);
                let _ = child.wait();
                break None;
            }
            thread::sleep(Duration::from_millis(20));
        };

        let stdout = out.join().unwrap_or_default();
        let stderr = err.join().unwrap_or_default();
        Ok(RunOutput {
            argv,
            exit_code: status.and_then(|s| s.code()),
            stdout,
            stderr,
            duration: start.elapsed(),
            timed_out,
            cancelled,
            truncated: truncated.load(Ordering::Relaxed),
        })
    }
}

fn reader<R: Read + Send + 'static>(
    stream: Option<R>,
    cap: usize,
    truncated: Arc<AtomicBool>,
) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut buf = Vec::new();
        let Some(mut stream) = stream else {
            return buf;
        };
        let mut chunk = [0u8; 64 * 1024];
        loop {
            match stream.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let room = cap.saturating_sub(buf.len());
                    if n > room {
                        truncated.store(true, Ordering::Relaxed);
                    }
                    // Keep draining so the child never blocks on a full pipe.
                    buf.extend_from_slice(&chunk[..n.min(room)]);
                }
            }
        }
        buf
    })
}

/// Kills the child and its descendants (§4.6).
fn kill_tree(child: &mut Child) {
    let pid = child.id();
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/T", "/F", "/PID", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(unix)]
    {
        // Negative pid = the process group created with `process_group(0)`.
        let _ = Command::new("kill")
            .args(["-KILL", "--", &format!("-{pid}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_rules() {
        assert!(CHILD_ENV_REMOVE.contains(&"SF_TEMP_SHOW_SECRETS"));
        assert!(
            CHILD_ENV
                .iter()
                .any(|(k, _)| *k == "SF_SKIP_NEW_VERSION_CHECK")
        );
    }

    #[test]
    fn explicit_bin_wins() {
        let (p, src) = resolve_sf_bin(Some(Path::new("/x/sf"))).unwrap();
        assert_eq!(p, PathBuf::from("/x/sf"));
        assert_eq!(src, BinSource::Explicit);
    }
}
