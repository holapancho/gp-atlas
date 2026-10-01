//! Regenerates `manifest/sf-2.150.6.json` from an installed `sf` (SPEC §4.4).
//!
//! ```text
//! extract-manifest [--sf <bin>] [--input <commands.json>] [--out <file>] [--check]
//! ```
//!
//! * `--sf`     sf binary. Default: `$GP_ATLAS_SF_BIN`, else `sf` on PATH.
//! * `--input`  read saved `sf commands --json` output instead of running sf
//!   (skips the version check; for offline experiments only).
//! * `--out`    write the manifest here (default: stdout).
//! * `--check`  do not write; exit 1 if `--out` differs from the generated manifest.
//!
//! Refuses to run against any CLI other than `@salesforce/cli/2.150.6`.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use gp_atlas_core::REQUIRED_CLI_VERSION_STRING;
use gp_atlas_core::manifest::Manifest;
use serde_json::Value;

#[derive(Default)]
struct Args {
    sf: Option<PathBuf>,
    input: Option<PathBuf>,
    out: Option<PathBuf>,
    check: bool,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("extract-manifest: {e}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let args = parse_args()?;

    let commands: Value = match &args.input {
        Some(path) => {
            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("reading {}: {e}", path.display()))?;
            serde_json::from_str(&text).map_err(|e| format!("parsing {}: {e}", path.display()))?
        }
        None => {
            let bin = resolve_sf(args.sf.as_deref())?;
            let version = sf_json(&bin, &["version", "--json"])?;
            let found = version.get("cliVersion").and_then(Value::as_str);
            if found != Some(REQUIRED_CLI_VERSION_STRING) {
                return Err(format!(
                    "{} reports cliVersion {:?}; expected {REQUIRED_CLI_VERSION_STRING:?}. \
                     Install it with: {}",
                    bin.display(),
                    found.unwrap_or("<missing>"),
                    gp_atlas_core::CLI_INSTALL_COMMAND
                ));
            }
            sf_json(&bin, &["commands", "--json"])?
        }
    };

    let manifest = Manifest::extract(&commands).map_err(|e| e.to_string())?;
    let json = manifest.to_pretty_json();

    match (&args.out, args.check) {
        (Some(out), true) => {
            let existing = std::fs::read_to_string(out).unwrap_or_default();
            if existing == json {
                eprintln!("{} is up to date", out.display());
                Ok(ExitCode::SUCCESS)
            } else {
                eprintln!("{} differs from the generated manifest", out.display());
                Ok(ExitCode::FAILURE)
            }
        }
        (None, true) => Err("--check requires --out".into()),
        (Some(out), false) => {
            std::fs::write(out, json).map_err(|e| format!("writing {}: {e}", out.display()))?;
            eprintln!(
                "wrote {} ({} commands)",
                out.display(),
                manifest.commands.len()
            );
            Ok(ExitCode::SUCCESS)
        }
        (None, false) => {
            print!("{json}");
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args::default();
    let mut it = std::env::args_os().skip(1);
    while let Some(arg) = it.next() {
        let arg = arg.to_string_lossy().into_owned();
        let mut value = |name: &str| {
            it.next()
                .map(PathBuf::from)
                .ok_or_else(|| format!("{name} requires a value"))
        };
        match arg.as_str() {
            "--sf" => args.sf = Some(value("--sf")?),
            "--input" => args.input = Some(value("--input")?),
            "--out" => args.out = Some(value("--out")?),
            "--check" => args.check = true,
            "-h" | "--help" => {
                println!(
                    "usage: extract-manifest [--sf <bin>] [--input <commands.json>] [--out <file>] [--check]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok(args)
}

/// `--sf` → `$GP_ATLAS_SF_BIN` → `sf` on PATH (resolves `sf.cmd` on Windows).
fn resolve_sf(explicit: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(p) = explicit {
        return Ok(p.to_path_buf());
    }
    if let Some(p) = std::env::var_os("GP_ATLAS_SF_BIN").filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(p));
    }
    which::which("sf").map_err(|e| {
        format!(
            "sf not found on PATH ({e}); install it with: {}",
            gp_atlas_core::CLI_INSTALL_COMMAND
        )
    })
}

/// Runs a read-only sf command (argv, no shell) and parses stdout as JSON.
fn sf_json(bin: &Path, argv: &[&str]) -> Result<Value, String> {
    let out = Command::new(bin)
        .args(argv)
        .env("NO_COLOR", "1")
        .env("SF_SKIP_NEW_VERSION_CHECK", "true")
        .env("SF_AUTOUPDATE_DISABLE", "true")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("running {} {}: {e}", bin.display(), argv.join(" ")))?;
    if !out.status.success() {
        return Err(format!(
            "`sf {}` exited with {}: {}",
            argv.join(" "),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    serde_json::from_slice(&out.stdout)
        .map_err(|e| format!("`sf {}` did not print JSON: {e}", argv.join(" ")))
}
