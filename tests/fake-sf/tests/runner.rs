//! Drives `gp_atlas_core::runner` through the fake `sf` (SPEC §11.2).

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use gp_atlas_core::classify::{CapState, UnknownReason, UnreachableReason, classify};
use gp_atlas_core::command::ReadOnlyCommand;
use gp_atlas_core::envelope::{SfOutput, parse};
use gp_atlas_core::ids::OrgRef;
use gp_atlas_core::manifest::Manifest;
use gp_atlas_core::runner::{RunnerConfig, SfRunner};

const BIN: &str = env!("CARGO_BIN_EXE_fake-sf");
/// Set when an outer test re-runs this binary for an `inner_*` test.
const INNER: &str = "GP_ATLAS_INNER_TEST";

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fake-sf-runner-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn runner(timeout: Duration) -> SfRunner {
    let mut cfg = RunnerConfig::new(PathBuf::from(BIN));
    cfg.timeout = timeout;
    SfRunner::new(cfg)
}

fn m() -> Manifest {
    Manifest::embedded().unwrap()
}

#[test]
fn success_and_error_are_classified() {
    let r = runner(Duration::from_secs(30));
    let ok = r
        .run(
            &ReadOnlyCommand::PkgList {
                hub: OrgRef::new("fake0009").unwrap(),
                verbose: false,
                api_version: None,
            },
            &m(),
            None,
            None,
        )
        .unwrap();
    assert_eq!(ok.exit_code, Some(0));
    assert_eq!(classify(&ok).state, CapState::Allowed);
    let SfOutput::Success { result, .. } = parse(&ok.stdout).unwrap() else {
        panic!()
    };
    assert_eq!(result.as_array().unwrap().len(), 11);

    let err = r
        .run(
            &ReadOnlyCommand::PkgList {
                hub: OrgRef::new("nobody@example.com").unwrap(),
                verbose: false,
                api_version: None,
            },
            &m(),
            None,
            None,
        )
        .unwrap();
    assert_eq!(err.exit_code, Some(2));
    assert_eq!(
        classify(&err).state,
        CapState::Unreachable(UnreachableReason::NotAuthenticated)
    );
}

#[test]
fn timeout_kills_the_child() {
    let dir = scratch("timeout");
    std::fs::write(
        dir.join("index.json"),
        r#"{"cases":[{"argv":["alias","list","--json"],"delay_ms":20000}]}"#,
    )
    .unwrap();
    // The runner passes the parent's env through, so the index is selected by
    // re-running this test binary with the variable set (no unsafe set_var).
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "inner_timeout", "--ignored", "--nocapture"])
        .env(INNER, "1")
        .env("GP_ATLAS_FAKE_SF_INDEX", dir.join("index.json"))
        .status()
        .unwrap();
    assert!(status.success());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "run by timeout_kills_the_child"]
fn inner_timeout() {
    if std::env::var_os(INNER).is_none() {
        return;
    }
    let r = runner(Duration::from_millis(400));
    let out = r
        .run(&ReadOnlyCommand::AliasList, &m(), None, None)
        .unwrap();
    assert!(out.timed_out);
    assert_eq!(out.exit_code, None);
    assert!(out.duration < Duration::from_secs(5), "{:?}", out.duration);
    assert_eq!(
        classify(&out).state,
        CapState::Unknown(UnknownReason::Timeout)
    );
}

#[test]
fn secrets_env_var_is_removed_and_child_env_is_set() {
    let dir = scratch("env");
    std::fs::write(
        dir.join("index.json"),
        r#"{"cases":[{"argv":["version","--json"],
            "env_report":["SF_TEMP_SHOW_SECRETS","NO_COLOR","SF_SKIP_NEW_VERSION_CHECK","SF_AUTOUPDATE_DISABLE"]}]}"#,
    )
    .unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "inner_env", "--ignored", "--nocapture"])
        .env(INNER, "1")
        .env("GP_ATLAS_FAKE_SF_INDEX", dir.join("index.json"))
        .env("SF_TEMP_SHOW_SECRETS", "true")
        .status()
        .unwrap();
    assert!(status.success());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "run by secrets_env_var_is_removed_and_child_env_is_set"]
fn inner_env() {
    if std::env::var_os(INNER).is_none() {
        return;
    }
    assert_eq!(std::env::var("SF_TEMP_SHOW_SECRETS").as_deref(), Ok("true"));
    let out = runner(Duration::from_secs(30))
        .run(&ReadOnlyCommand::Version, &m(), None, None)
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let env = &v["env"];
    assert!(env["SF_TEMP_SHOW_SECRETS"].is_null(), "{env}");
    assert_eq!(env["NO_COLOR"], "1");
    assert_eq!(env["SF_SKIP_NEW_VERSION_CHECK"], "true");
    assert_eq!(env["SF_AUTOUPDATE_DISABLE"], "true");
}

#[test]
fn doctor_against_fixtures() {
    let report = gp_atlas_core::doctor::run(&runner(Duration::from_secs(30)), &m());
    assert!(report.d1_runnable.is_pass());
    assert!(report.d2_version.is_pass());
    assert!(report.d3_packaging_plugin.is_pass());
    // No `commands --json` fixture is committed (1.3 MB), so D4 fails here.
    assert!(!report.d4_contract.is_pass());
    assert!(report.version_allowed(None));
}
