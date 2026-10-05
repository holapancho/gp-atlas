//! Contract tests against a real `sf` (SPEC §11.3). Ignored by default; CI runs
//! them with `--ignored`. The sf binary is `$GP_ATLAS_SF_BIN` or `sf` on PATH.
//!
//! `GP_ATLAS_CONTRACT_MODE`:
//! * `baseline` (default) — `sf` must be exactly 2.150.6 and match the
//!   committed manifest byte for byte (job `sf-contract`).
//! * `compat` — any supported newer `sf`: the version gate passes, the
//!   packaging plugin is the bundled one, and no allow-listed command drifted
//!   (job `sf-latest`).

use std::path::PathBuf;
use std::process::{Command, Stdio};

use gp_atlas_core::blocklist::is_blocked_command;
use gp_atlas_core::doctor::{check_contract, check_packaging_plugin, check_version};
use gp_atlas_core::manifest::Manifest;
use gp_atlas_core::{
    BASELINE_CLI_VERSION_STRING, PACKAGING_PLUGIN_BASELINE_VERSION, PACKAGING_PLUGIN_NAME,
};
use serde_json::Value;

fn compat() -> bool {
    std::env::var("GP_ATLAS_CONTRACT_MODE").as_deref() == Ok("compat")
}

fn sf_bin() -> PathBuf {
    std::env::var_os("GP_ATLAS_SF_BIN")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| which::which("sf").expect("sf on PATH"))
}

fn sf_json(argv: &[&str]) -> Value {
    let out = Command::new(sf_bin())
        .args(argv)
        .env("NO_COLOR", "1")
        .env("SF_SKIP_NEW_VERSION_CHECK", "true")
        .env("SF_AUTOUPDATE_DISABLE", "true")
        .stdin(Stdio::null())
        .output()
        .expect("spawn sf");
    assert!(out.status.success(), "sf {argv:?} failed: {out:?}");
    serde_json::from_slice(&out.stdout).expect("sf printed JSON")
}

#[test]
#[ignore = "needs a real sf; run in the sf-contract / sf-latest CI jobs"]
fn cli_version() {
    let v = sf_json(&["version", "--json"]);
    if compat() {
        let (check, found) = check_version(&v);
        assert!(check.is_pass(), "{check:?}");
        eprintln!("compat run against {}", found.unwrap_or_default());
    } else {
        assert_eq!(v["cliVersion"], BASELINE_CLI_VERSION_STRING);
    }
}

#[test]
#[ignore = "needs a real sf; run in the sf-contract / sf-latest CI jobs"]
fn packaging_plugin_is_bundled_core() {
    // F3: bare array; D3: bundled packaging plugin.
    let v = sf_json(&["plugins", "--json"]);
    let check = check_packaging_plugin(&v);
    assert!(check.is_pass(), "{check:?}");
    if !compat() {
        let p = v
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == PACKAGING_PLUGIN_NAME)
            .unwrap();
        assert_eq!(p["version"], PACKAGING_PLUGIN_BASELINE_VERSION);
    }
}

#[test]
#[ignore = "needs a real sf; run in the sf-contract / sf-latest CI jobs"]
fn live_commands_match_committed_manifest() {
    let live_json = sf_json(&["commands", "--json"]);
    let committed = Manifest::embedded().expect("committed manifest");
    if compat() {
        let (check, drift) = check_contract(&live_json, &committed);
        assert!(
            check.is_pass(),
            "commands drifted from the 2.150.6 baseline (GP Atlas disables them): {drift:#?}"
        );
    } else {
        let live = Manifest::extract(&live_json).expect("extract");
        assert_eq!(live, committed);
    }
}

#[test]
#[ignore = "needs a real sf; run in the sf-contract / sf-latest CI jobs"]
fn blocklist_covers_real_mutating_commands() {
    // The ids the blocklist is written against must exist in the real CLI,
    // otherwise the rule silently protects nothing.
    let commands = sf_json(&["commands", "--json"]);
    let ids: Vec<&str> = commands
        .as_array()
        .expect("bare array")
        .iter()
        .filter_map(|c| c["id"].as_str())
        .collect();
    for id in [
        "package:version:create",
        "package:version:promote",
        "package:version:delete",
        "package:version:update",
        "package:create",
        "package:install",
        "package:uninstall",
        "package:convert",
        "package:version:retrieve",
        "package:push-upgrade:schedule",
        "package:push-upgrade:abort",
        "package1:version:create",
        "org:display",
        "org:login:web",
        "org:logout",
        "config:set",
        "alias:set",
        "plugins:install",
        "plugins:link",
        "plugins:uninstall",
        "plugins:update",
    ] {
        assert!(ids.contains(&id), "{id} not in sf commands --json");
        assert!(is_blocked_command(id), "{id} not blocklisted");
    }
    // And no manifest command is blocked.
    for c in &Manifest::embedded().unwrap().commands {
        assert!(!is_blocked_command(&c.id), "{}", c.id);
    }
}
