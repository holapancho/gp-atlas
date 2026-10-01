//! `fake-sf`: a test double for the Salesforce CLI (SPEC §11.2).
//!
//! Replays captured fixtures keyed by the exact argv it receives. Point GP Atlas
//! at it with `GP_ATLAS_SF_BIN=<path to fake-sf>`.
//!
//! Environment:
//! * `GP_ATLAS_FAKE_SF_INDEX` — fixture index file (default:
//!   `fixtures/sf-2.150.6/index.json` in this repository). Fixture paths in the
//!   index are relative to the index file's directory.
//! * `GP_ATLAS_FAKE_SF_LOG` — if set, one JSON line `{"argv": [...]}` is
//!   appended to this file per invocation (lets tests assert exactly which
//!   commands were run).
//!
//! Index format:
//! ```json
//! { "cases": [ { "argv": ["version", "--json"], "stdout": "version.json",
//!                "stderr": "optional inline text", "exit_code": 0, "delay_ms": 0 } ] }
//! ```
//! An argv with no matching case prints a message on stderr and exits with
//! [`NO_FIXTURE_EXIT_CODE`].

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use serde::Deserialize;

/// Exit code for an argv that has no fixture. Chosen not to collide with `sf`'s own codes.
const NO_FIXTURE_EXIT_CODE: u8 = 97;
/// Exit code for a broken fixture index or fixture file.
const BAD_FIXTURE_EXIT_CODE: u8 = 98;

#[derive(Deserialize)]
struct Index {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    argv: Vec<String>,
    #[serde(default)]
    stdout: Option<String>,
    #[serde(default)]
    stderr: Option<String>,
    #[serde(default)]
    exit_code: u8,
    #[serde(default)]
    delay_ms: u64,
}

fn default_index() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sf-2.150.6/index.json")
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();

    if let Some(log) = std::env::var_os("GP_ATLAS_FAKE_SF_LOG") {
        let line = serde_json::json!({ "argv": argv }).to_string();
        if let Err(e) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
            .and_then(|mut f| writeln!(f, "{line}"))
        {
            eprintln!(
                "fake-sf: cannot write log {}: {e}",
                Path::new(&log).display()
            );
            return ExitCode::from(BAD_FIXTURE_EXIT_CODE);
        }
    }

    let index_path = std::env::var_os("GP_ATLAS_FAKE_SF_INDEX")
        .map(PathBuf::from)
        .unwrap_or_else(default_index);
    let index: Index = match std::fs::read_to_string(&index_path)
        .map_err(|e| e.to_string())
        .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
    {
        Ok(i) => i,
        Err(e) => {
            eprintln!("fake-sf: cannot load index {}: {e}", index_path.display());
            return ExitCode::from(BAD_FIXTURE_EXIT_CODE);
        }
    };

    let Some(case) = index.cases.iter().find(|c| c.argv == argv) else {
        eprintln!(
            "fake-sf: no fixture for argv {argv:?} in {}",
            index_path.display()
        );
        return ExitCode::from(NO_FIXTURE_EXIT_CODE);
    };

    if case.delay_ms > 0 {
        std::thread::sleep(Duration::from_millis(case.delay_ms));
    }

    if let Some(file) = &case.stdout {
        let dir = index_path.parent().unwrap_or(Path::new("."));
        match std::fs::read(dir.join(file)) {
            Ok(bytes) => {
                let mut out = std::io::stdout().lock();
                // A closed pipe (reader gave up) is not an error for a test double.
                let _ = out.write_all(&bytes).and_then(|()| out.flush());
            }
            Err(e) => {
                eprintln!("fake-sf: cannot read fixture {file}: {e}");
                return ExitCode::from(BAD_FIXTURE_EXIT_CODE);
            }
        }
    }
    if let Some(text) = &case.stderr {
        eprint!("{text}");
    }
    ExitCode::from(case.exit_code)
}
