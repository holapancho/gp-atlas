use std::path::PathBuf;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_fake-sf");

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fake-sf-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(args: &[&str], envs: &[(&str, &std::path::Path)]) -> Output {
    let mut cmd = Command::new(BIN);
    cmd.args(args)
        .env_remove("GP_ATLAS_FAKE_SF_INDEX")
        .env_remove("GP_ATLAS_FAKE_SF_LOG");
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.output().unwrap()
}

fn stdout_json(out: &Output) -> serde_json::Value {
    serde_json::from_slice(&out.stdout).expect("stdout is JSON")
}

#[test]
fn replays_version_fixture() {
    let out = run(&["version", "--json"], &[]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(stdout_json(&out)["cliVersion"], "@salesforce/cli/2.150.6");
}

#[test]
fn replays_error_envelope_with_exit_code() {
    let out = run(
        &[
            "package",
            "list",
            "--target-dev-hub",
            "nobody@example.com",
            "--json",
        ],
        &[],
    );
    assert_eq!(out.status.code(), Some(2));
    let v = stdout_json(&out);
    assert_eq!(v["name"], "NamedOrgNotFoundError");
    assert_eq!(v["status"], 2);
}

#[test]
fn plugins_fixture_is_bare_array_with_core_packaging_plugin() {
    let out = run(&["plugins", "--json"], &[]);
    let v = stdout_json(&out);
    let p = v
        .as_array()
        .expect("F3: bare array")
        .iter()
        .find(|p| p["name"] == "@salesforce/plugin-packaging")
        .expect("packaging plugin listed");
    assert_eq!(p["version"], "3.0.6");
    assert_eq!(p["type"], "core");
}

#[test]
fn unknown_argv_fails_distinctly() {
    let out = run(&["package", "version", "create", "--json"], &[]);
    assert_eq!(out.status.code(), Some(97));
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr).contains("no fixture"));
}

#[test]
fn custom_index_delay_stderr_and_log() {
    let dir = scratch("custom");
    std::fs::write(dir.join("out.json"), r#"{"status":0,"result":[]}"#).unwrap();
    std::fs::write(
        dir.join("index.json"),
        r#"{"cases":[{"argv":["alias","list","--json"],"stdout":"out.json",
            "stderr":"diag","exit_code":0,"delay_ms":50}]}"#,
    )
    .unwrap();
    let log = dir.join("calls.jsonl");
    let _ = std::fs::remove_file(&log);

    let start = std::time::Instant::now();
    let out = run(
        &["alias", "list", "--json"],
        &[
            ("GP_ATLAS_FAKE_SF_INDEX", &dir.join("index.json")),
            ("GP_ATLAS_FAKE_SF_LOG", &log),
        ],
    );
    assert!(start.elapsed().as_millis() >= 50);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(stdout_json(&out)["status"], 0);
    assert_eq!(out.stderr, b"diag");

    let logged = std::fs::read_to_string(&log).unwrap();
    let line: serde_json::Value = serde_json::from_str(logged.trim()).unwrap();
    assert_eq!(line["argv"], serde_json::json!(["alias", "list", "--json"]));
    let _ = std::fs::remove_dir_all(&dir);
}
