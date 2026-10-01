//! Contract tests against a real `sf` (SPEC §11.3). Ignored by default; the
//! `sf-contract` CI job runs them with `--ignored` after installing exactly
//! `@salesforce/cli@2.150.6`. The sf binary is `$GP_ATLAS_SF_BIN` or `sf` on PATH.

use std::path::PathBuf;
use std::process::{Command, Stdio};

use gp_atlas_core::blocklist::is_blocked_command;
use gp_atlas_core::manifest::Manifest;
use gp_atlas_core::{PACKAGING_PLUGIN_NAME, PACKAGING_PLUGIN_VERSION, REQUIRED_CLI_VERSION_STRING};
use serde_json::Value;

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
#[ignore = "needs sf 2.150.6; run in the sf-contract CI job"]
fn cli_version_is_pinned() {
    let v = sf_json(&["version", "--json"]);
    assert_eq!(v["cliVersion"], REQUIRED_CLI_VERSION_STRING);
}

#[test]
#[ignore = "needs sf 2.150.6; run in the sf-contract CI job"]
fn packaging_plugin_is_bundled_core() {
    // F3: bare array; D3: bundled packaging plugin.
    let v = sf_json(&["plugins", "--json"]);
    let p = v
        .as_array()
        .expect("bare array")
        .iter()
        .find(|p| p["name"] == PACKAGING_PLUGIN_NAME)
        .expect("packaging plugin listed");
    assert_eq!(p["version"], PACKAGING_PLUGIN_VERSION);
    assert_eq!(p["type"], "core");
}

#[test]
#[ignore = "needs sf 2.150.6; run in the sf-contract CI job"]
fn live_commands_match_committed_manifest() {
    let live = Manifest::extract(&sf_json(&["commands", "--json"])).expect("extract");
    let committed = Manifest::embedded().expect("committed manifest");
    assert_eq!(live, committed);
}

#[test]
#[ignore = "needs sf 2.150.6; run in the sf-contract CI job"]
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
