//! Access Matrix result cache (SPEC §7.5, AC-17).
//!
//! Probe results are kept for [`PROBE_TTL`] in a JSON file in the OS cache
//! folder, so reopening the app shows recent results without re-running
//! ~5 `sf` calls per org. Entries are keyed by CLI version, org username and
//! capability; a different CLI version invalidates them. Only states and the
//! already-scrubbed error text are stored, never tokens (none are obtained).

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::classify::{CapState, DenyReason, UnknownReason, UnreachableReason};

/// How long a probe result stays valid (§7.5: 15 min for probes).
pub const PROBE_TTL: Duration = Duration::from_secs(15 * 60);

const FILE_NAME: &str = "access-matrix.json";
const FORMAT: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedProbe {
    pub username: String,
    /// `Capability::name()`.
    pub capability: String,
    /// [`encode_state`].
    pub state: String,
    pub error_name: String,
    pub message: String,
    pub rule: String,
    pub command: String,
    /// Seconds since the Unix epoch.
    pub checked_at: u64,
}

impl CachedProbe {
    pub fn checked_at(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(self.checked_at)
    }

    pub fn is_fresh(&self, now: SystemTime) -> bool {
        now.duration_since(self.checked_at())
            .is_ok_and(|age| age < PROBE_TTL)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CacheFile {
    format: u32,
    cli_version: String,
    entries: Vec<CachedProbe>,
}

/// `GP_ATLAS_CACHE_DIR`, else the OS cache folder + `gp-atlas`:
/// `~/Library/Caches` (macOS), `%LOCALAPPDATA%` (Windows),
/// `$XDG_CACHE_HOME` or `~/.cache` (Linux/other).
pub fn cache_dir() -> Option<PathBuf> {
    let env = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if let Some(d) = env("GP_ATLAS_CACHE_DIR") {
        return Some(d);
    }
    let base = if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library/Caches"))
    } else if cfg!(windows) {
        env("LOCALAPPDATA")
    } else {
        env("XDG_CACHE_HOME").or_else(|| env("HOME").map(|h| h.join(".cache")))
    };
    base.map(|b| b.join("gp-atlas"))
}

pub fn cache_file() -> Option<PathBuf> {
    cache_dir().map(|d| d.join(FILE_NAME))
}

/// Fresh entries for this CLI version. A missing or unreadable file is an
/// empty cache, never an error.
pub fn load(path: &Path, cli_version: &str, now: SystemTime) -> Vec<CachedProbe> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    match serde_json::from_slice::<CacheFile>(&bytes) {
        Ok(f) if f.format == FORMAT && f.cli_version == cli_version => f
            .entries
            .into_iter()
            .filter(|e| e.is_fresh(now) && decode_state(&e.state).is_some())
            .collect(),
        _ => Vec::new(),
    }
}

/// Writes the fresh entries atomically (temp file + rename), owner-only on Unix.
pub fn save(
    path: &Path,
    cli_version: &str,
    entries: &[CachedProbe],
    now: SystemTime,
) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let file = CacheFile {
        format: FORMAT,
        cli_version: cli_version.to_owned(),
        entries: entries
            .iter()
            .filter(|e| e.is_fresh(now))
            .cloned()
            .collect(),
    };
    let json = serde_json::to_vec_pretty(&file).map_err(std::io::Error::other)?;
    let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
    {
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        use std::io::Write;
        let mut f = opts.open(&tmp)?;
        f.write_all(&json)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

pub fn unix_secs(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Stable text form of a [`CapState`] for the cache file.
pub fn encode_state(s: &CapState) -> String {
    match s {
        CapState::Allowed => "Allowed".into(),
        CapState::Denied(r) => format!("Denied:{r:?}"),
        CapState::NotApplicable(r) => format!("NotApplicable:{r}"),
        CapState::Unreachable(r) => format!("Unreachable:{r:?}"),
        CapState::Unknown(r) => format!("Unknown:{r:?}"),
        CapState::ContractDrift(d) => format!("ContractDrift:{d}"),
    }
}

/// Inverse of [`encode_state`]; `None` for anything unknown (entry dropped).
pub fn decode_state(s: &str) -> Option<CapState> {
    let (kind, arg) = s.split_once(':').unwrap_or((s, ""));
    Some(match (kind, arg) {
        ("Allowed", "") => CapState::Allowed,
        ("Denied", "PackagingNotEnabled") => CapState::Denied(DenyReason::PackagingNotEnabled),
        ("Denied", "InsufficientAccess") => CapState::Denied(DenyReason::InsufficientAccess),
        ("Denied", "ApiDisabled") => CapState::Denied(DenyReason::ApiDisabled),
        ("NotApplicable", r) => CapState::NotApplicable(r.to_owned()),
        ("Unreachable", "NotAuthenticated") => {
            CapState::Unreachable(UnreachableReason::NotAuthenticated)
        }
        ("Unreachable", "SessionExpired") => {
            CapState::Unreachable(UnreachableReason::SessionExpired)
        }
        ("Unreachable", "Network") => CapState::Unreachable(UnreachableReason::Network),
        ("Unreachable", "OrgUnavailable") => {
            CapState::Unreachable(UnreachableReason::OrgUnavailable)
        }
        ("Unknown", "Timeout") => CapState::Unknown(UnknownReason::Timeout),
        ("Unknown", "NotJson") => CapState::Unknown(UnknownReason::NotJson),
        ("Unknown", "Unclassified") => CapState::Unknown(UnknownReason::Unclassified),
        ("ContractDrift", d) => CapState::ContractDrift(d.to_owned()),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(user: &str, state: &CapState, at: SystemTime) -> CachedProbe {
        CachedProbe {
            username: user.into(),
            capability: "Pkg2.ListPackages".into(),
            state: encode_state(state),
            error_name: String::new(),
            message: String::new(),
            rule: String::new(),
            command: "sf package list --target-dev-hub x".into(),
            checked_at: unix_secs(at),
        }
    }

    #[test]
    fn states_round_trip() {
        for s in [
            CapState::Allowed,
            CapState::Denied(DenyReason::PackagingNotEnabled),
            CapState::Denied(DenyReason::InsufficientAccess),
            CapState::Denied(DenyReason::ApiDisabled),
            CapState::NotApplicable("not flagged as Dev Hub".into()),
            CapState::Unreachable(UnreachableReason::NotAuthenticated),
            CapState::Unreachable(UnreachableReason::SessionExpired),
            CapState::Unreachable(UnreachableReason::Network),
            CapState::Unreachable(UnreachableReason::OrgUnavailable),
            CapState::Unknown(UnknownReason::Timeout),
            CapState::Unknown(UnknownReason::NotJson),
            CapState::Unknown(UnknownReason::Unclassified),
            CapState::ContractDrift("package list".into()),
        ] {
            assert_eq!(decode_state(&encode_state(&s)), Some(s));
        }
        assert_eq!(decode_state("Denied:Nope"), None);
        // A cancelled probe is not a result worth keeping.
        assert_eq!(decode_state("Unknown:Cancelled"), None);
    }

    #[test]
    fn save_load_ttl_and_version() {
        let dir = std::env::temp_dir().join(format!("gp-atlas-cache-{}", std::process::id()));
        let path = dir.join(FILE_NAME);
        let now = SystemTime::now();
        let old = now - PROBE_TTL - Duration::from_secs(1);
        let entries = vec![
            entry("fresh@x", &CapState::Allowed, now - Duration::from_secs(60)),
            entry("old@x", &CapState::Allowed, old),
        ];
        save(&path, "2.150.6", &entries, now).unwrap();
        let loaded = load(&path, "2.150.6", now);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].username, "fresh@x");
        // Another CLI version invalidates the cache.
        assert!(load(&path, "2.151.0", now).is_empty());
        // 15 minutes later it has expired.
        assert!(load(&path, "2.150.6", now + PROBE_TTL).is_empty());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        // Garbage is an empty cache, not an error.
        std::fs::write(&path, b"not json").unwrap();
        assert!(load(&path, "2.150.6", now).is_empty());
        assert!(load(&dir.join("missing.json"), "2.150.6", now).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
