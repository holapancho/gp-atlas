//! Doctor checks D1–D4 (SPEC §5.2 L0).

use serde_json::Value;

use crate::command::ReadOnlyCommand;
use crate::envelope::{self, SfOutput};
use crate::manifest::{CommandSpec, Manifest};
use crate::runner::{RunOutput, SfRunner};
use crate::{PACKAGING_PLUGIN_NAME, PACKAGING_PLUGIN_VERSION, REQUIRED_CLI_VERSION_STRING};

/// Outcome of one check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Check {
    Pass(String),
    Fail(String),
    Skipped(String),
}

impl Check {
    pub fn is_pass(&self) -> bool {
        matches!(self, Self::Pass(_))
    }
}

/// Contract drift for one command (D4, AC-06).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drift {
    pub command: String,
    pub details: Vec<String>,
}

/// Full Doctor report.
#[derive(Debug, Clone)]
pub struct DoctorReport {
    pub d1_runnable: Check,
    pub d2_version: Check,
    pub found_version: Option<String>,
    pub d3_packaging_plugin: Check,
    pub d4_contract: Check,
    pub drift: Vec<Drift>,
    /// Raw runs, for debugging.
    pub runs: Vec<RunOutput>,
}

impl DoctorReport {
    pub fn all_green(&self) -> bool {
        [
            &self.d1_runnable,
            &self.d2_version,
            &self.d3_packaging_plugin,
            &self.d4_contract,
        ]
        .iter()
        .all(|c| c.is_pass())
    }

    /// Version gate (§4.2): sf features are enabled only for the pinned version,
    /// or for an explicitly allowed one (developer override).
    pub fn version_allowed(&self, allow: Option<&str>) -> bool {
        match &self.found_version {
            Some(v) if v == REQUIRED_CLI_VERSION_STRING => true,
            Some(v) => allow.is_some_and(|a| v == a || v.ends_with(&format!("/{a}"))),
            None => false,
        }
    }
}

/// D2: `cliVersion` from `sf version --json`.
pub fn check_version(out: &Value) -> (Check, Option<String>) {
    match out.get("cliVersion").and_then(Value::as_str) {
        Some(v) if v == REQUIRED_CLI_VERSION_STRING => {
            (Check::Pass(v.to_owned()), Some(v.to_owned()))
        }
        Some(v) => (
            Check::Fail(format!(
                "found {v}, required {REQUIRED_CLI_VERSION_STRING}. Fix: {}",
                crate::CLI_INSTALL_COMMAND
            )),
            Some(v.to_owned()),
        ),
        None => (
            Check::Fail("no cliVersion in `sf version --json`".into()),
            None,
        ),
    }
}

/// D3: the packaging plugin must be the bundled core one (F23, F31).
pub fn check_packaging_plugin(plugins: &Value) -> Check {
    let Some(list) = plugins.as_array() else {
        return Check::Fail("`sf plugins --json` is not an array".into());
    };
    let entries: Vec<&Value> = list
        .iter()
        .filter(|p| p.get("name").and_then(Value::as_str) == Some(PACKAGING_PLUGIN_NAME))
        .collect();
    let Some(p) = entries.first() else {
        return Check::Fail(format!("{PACKAGING_PLUGIN_NAME} not listed"));
    };
    let version = p.get("version").and_then(Value::as_str).unwrap_or("?");
    let kind = p.get("type").and_then(Value::as_str).unwrap_or("?");
    if version == PACKAGING_PLUGIN_VERSION && kind == "core" && entries.len() == 1 {
        Check::Pass(format!("{PACKAGING_PLUGIN_NAME} {version} ({kind})"))
    } else {
        Check::Fail(format!(
            "PackagingPluginOverridden: found {PACKAGING_PLUGIN_NAME} {version} (type {kind}); \
             expected {PACKAGING_PLUGIN_VERSION} (core). A user-installed or linked plugin replaces \
             the bundled one. Fix: sf plugins uninstall {PACKAGING_PLUGIN_NAME}"
        ))
    }
}

/// D4: compares live `sf commands --json` with the manifest per command.
pub fn check_contract(commands: &Value, manifest: &Manifest) -> (Check, Vec<Drift>) {
    let Some(list) = commands.as_array() else {
        return (
            Check::Fail("`sf commands --json` is not an array".into()),
            vec![],
        );
    };
    let mut drift = Vec::new();
    for want in &manifest.commands {
        let live = list
            .iter()
            .find(|c| c.get("id").and_then(Value::as_str) == Some(&want.id))
            .map(CommandSpec::from_raw);
        let details = match live {
            None => vec!["command missing".to_owned()],
            Some(Err(e)) => vec![format!("unreadable: {e}")],
            Some(Ok(live)) => compare(want, &live),
        };
        if !details.is_empty() {
            drift.push(Drift {
                command: want.id.replace(':', " "),
                details,
            });
        }
    }
    let check = if drift.is_empty() {
        Check::Pass(format!(
            "{} commands match the manifest",
            manifest.commands.len()
        ))
    } else {
        Check::Fail(format!(
            "{} command(s) drifted; only those are disabled",
            drift.len()
        ))
    };
    (check, drift)
}

fn compare(want: &CommandSpec, live: &CommandSpec) -> Vec<String> {
    let mut out = Vec::new();
    for f in &want.flags {
        match live.flag(&f.name) {
            None => out.push(format!("--{} missing", f.name)),
            Some(l) => {
                if l.kind != f.kind {
                    out.push(format!("--{} type {} → {}", f.name, f.kind, l.kind));
                }
                if l.char != f.char {
                    out.push(format!("--{} char {:?} → {:?}", f.name, f.char, l.char));
                }
                if l.required != f.required {
                    out.push(format!(
                        "--{} required {} → {}",
                        f.name, f.required, l.required
                    ));
                }
                if l.options != f.options {
                    out.push(format!(
                        "--{} options {:?} → {:?}",
                        f.name, f.options, l.options
                    ));
                }
            }
        }
    }
    out
}

/// Runs D1–D4. `version`, `plugins` and `commands` run in parallel (§4.6:
/// process startup dominates, AC-03 wants a green Doctor within 8 s).
pub fn run(runner: &SfRunner, manifest: &Manifest) -> DoctorReport {
    let bare = |cmd: ReadOnlyCommand| -> (Result<Value, String>, Option<RunOutput>) {
        let out = match runner.run(&cmd, manifest, None, None) {
            Ok(o) => o,
            Err(e) => return (Err(e.to_string()), None),
        };
        let exit = out.exit_code;
        let parsed = if out.timed_out {
            Err("timed out".to_owned())
        } else {
            match envelope::parse(&out.stdout) {
                Ok(SfOutput::Bare(v)) if exit == Some(0) => Ok(v),
                Ok(SfOutput::Success { result, .. }) => Ok(result),
                Ok(other) => Err(format!("unexpected output (exit {exit:?}): {other:?}")),
                Err(e) => Err(format!("exit {exit:?}: {e}")),
            }
        };
        (parsed, Some(out))
    };

    let ((version, r1), (plugins, r2), (commands, r3)) = std::thread::scope(|s| {
        let p = s.spawn(|| bare(ReadOnlyCommand::Plugins));
        let c = s.spawn(|| bare(ReadOnlyCommand::Commands));
        let v = bare(ReadOnlyCommand::Version);
        let failed = |e| (Err(format!("worker panicked: {e:?}")), None);
        (
            v,
            p.join().unwrap_or_else(failed),
            c.join().unwrap_or_else(failed),
        )
    });
    let runs: Vec<RunOutput> = [r1, r2, r3].into_iter().flatten().collect();

    let skipped = || Check::Skipped("sf is not runnable".into());
    let (d1, d2, found) = match version {
        Ok(v) => {
            let (d2, found) = check_version(&v);
            (Check::Pass("sf version --json ran".into()), d2, found)
        }
        Err(e) => (Check::Fail(e), skipped(), None),
    };
    if !d1.is_pass() {
        return DoctorReport {
            d1_runnable: d1,
            d2_version: d2,
            found_version: found,
            d3_packaging_plugin: skipped(),
            d4_contract: skipped(),
            drift: vec![],
            runs,
        };
    }
    let d3 = match plugins {
        Ok(v) => check_packaging_plugin(&v),
        Err(e) => Check::Fail(e),
    };
    let (d4, drift) = match commands {
        Ok(v) => check_contract(&v, manifest),
        Err(e) => (Check::Fail(e), vec![]),
    };
    DoctorReport {
        d1_runnable: d1,
        d2_version: d2,
        found_version: found,
        d3_packaging_plugin: d3,
        d4_contract: d4,
        drift,
        runs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture(name: &str) -> Value {
        let b = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/sf-2.150.6")
                .join(name),
        )
        .unwrap();
        serde_json::from_slice(&b).unwrap()
    }

    #[test]
    fn version_gate() {
        assert!(check_version(&fixture("version.json")).0.is_pass());
        let (c, found) = check_version(&json!({"cliVersion": "@salesforce/cli/2.152.14"}));
        assert!(!c.is_pass());
        assert_eq!(found.as_deref(), Some("@salesforce/cli/2.152.14"));
    }

    #[test]
    fn packaging_plugin() {
        assert!(check_packaging_plugin(&fixture("plugins.json")).is_pass());
        let user = json!([{"name": PACKAGING_PLUGIN_NAME, "version": "3.1.0", "type": "user"}]);
        assert!(
            matches!(check_packaging_plugin(&user), Check::Fail(m) if m.contains("PackagingPluginOverridden"))
        );
        let link = json!([{"name": PACKAGING_PLUGIN_NAME, "version": "3.0.6", "type": "link"}]);
        assert!(!check_packaging_plugin(&link).is_pass());
    }

    #[test]
    fn contract_drift_only_for_changed_command() {
        let m = Manifest::embedded().unwrap();
        // Rebuild a "live" commands array from the manifest itself.
        let mut live: Vec<Value> = m
            .commands
            .iter()
            .map(|c| {
                let flags: serde_json::Map<String, Value> = c
                    .flags
                    .iter()
                    .map(|f| {
                        (
                            f.name.clone(),
                            json!({"name": f.name, "type": f.kind, "char": f.char,
                                   "required": f.required, "options": f.options}),
                        )
                    })
                    .collect();
                json!({"id": c.id, "flags": flags})
            })
            .collect();
        let (c, d) = check_contract(&Value::Array(live.clone()), &m);
        assert!(c.is_pass(), "{d:?}");

        let pvl = live
            .iter_mut()
            .find(|c| c["id"] == "package:version:list")
            .unwrap();
        pvl["flags"]["order-by"]["char"] = json!("z");
        pvl["flags"].as_object_mut().unwrap().remove("branch");
        let (c, d) = check_contract(&Value::Array(live), &m);
        assert!(!c.is_pass());
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].command, "package version list");
        assert_eq!(d[0].details.len(), 2);
    }
}
