//! `capture`: runs the read-only command set against the user's orgs and saves
//! sanitized output (fixtures for SPEC §4.7 / docs/FIXTURE_CAPTURE.md).

use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;

use gp_atlas_core::classify::{CapState, classify};
use gp_atlas_core::command::{PkgVersionListArgs, ReadOnlyCommand};
use gp_atlas_core::orgs::{Org, OrgKind};
use gp_atlas_core::probes::{Capability, org_ref};
use gp_atlas_core::runner::RunOutput;
use serde_json::{Value, json};

use crate::sanitize::Sanitizer;
use crate::{Ctx, ProbeResult, load_orgs, probe_orgs, run_parallel};

const README: &str = "\
GP Atlas capture
================

Output of read-only `sf` commands, captured by `gp-atlas-beta capture`.

Sanitized automatically: usernames/emails, aliases, org and record IDs,
instance hosts, package/org names, namespaces, descriptions, branches, tags.
Secrets (access tokens, passwords, refresh tokens, client secrets, auth URLs,
installation keys) are removed. Stack traces are dropped.

PLEASE REVIEW before sharing: search the folder for your company name, your
own name, and any domain you recognize. Delete any file you are unsure about.

Files per run (NNN = run number):
  NNN-<command>__<org>.json         stdout (sanitized JSON)
  NNN-<command>__<org>.stdout.txt   stdout when it was not JSON
  NNN-<command>__<org>.stderr.txt   stderr (sanitized)
  NNN-<command>__<org>.meta.json    argv, exit code, duration, classification
summary.md                          overview and Access Matrix
";

struct Run {
    label: String,
    org: Option<usize>,
    out: Result<RunOutput, String>,
}

pub fn run(ctx: &Ctx, dir: &Path, probes: bool, max_orgs: usize) -> Result<(), ExitCode> {
    if dir.exists() {
        eprintln!(
            "error: {} already exists; choose a new folder",
            dir.display()
        );
        return Err(ExitCode::from(2));
    }
    std::fs::create_dir_all(dir).map_err(|e| {
        eprintln!("error: cannot create {}: {e}", dir.display());
        ExitCode::from(2)
    })?;

    eprintln!("Loading orgs…");
    let orgs = load_orgs(ctx, false)?;
    let probe_orgs_list: Vec<Org> = if max_orgs == 0 {
        orgs.clone()
    } else {
        orgs.iter().take(max_orgs).cloned().collect()
    };

    // 1. Inventory and Dev Hub listings.
    let mut steps: Vec<(String, ReadOnlyCommand, Option<usize>)> = vec![
        ("version".into(), ReadOnlyCommand::Version, None),
        ("plugins".into(), ReadOnlyCommand::Plugins, None),
        (
            "org-list".into(),
            ReadOnlyCommand::OrgList {
                skip_connection_status: false,
                all: false,
            },
            None,
        ),
        (
            "org-list-skip".into(),
            ReadOnlyCommand::OrgList {
                skip_connection_status: true,
                all: false,
            },
            None,
        ),
        ("alias-list".into(), ReadOnlyCommand::AliasList, None),
        ("config-get".into(), ReadOnlyCommand::ConfigGet, None),
    ];
    for (i, o) in orgs.iter().enumerate() {
        if o.kind != OrgKind::DevHub {
            continue;
        }
        let Ok(hub) = org_ref(o) else { continue };
        let mut verbose = PkgVersionListArgs::new(hub.clone());
        verbose.verbose = true;
        steps.push((
            "version-list".into(),
            ReadOnlyCommand::PkgVersionList(PkgVersionListArgs::new(hub.clone())),
            Some(i),
        ));
        steps.push((
            "version-list-verbose".into(),
            ReadOnlyCommand::PkgVersionList(verbose),
            Some(i),
        ));
        steps.push((
            "create-list".into(),
            ReadOnlyCommand::PkgCreateList {
                hub: hub.clone(),
                created_last_days: None,
                status: None,
                show_conversions_only: false,
                verbose: false,
            },
            Some(i),
        ));
        steps.push((
            "installed-list".into(),
            ReadOnlyCommand::PkgInstalledList { org: hub },
            Some(i),
        ));
    }
    eprintln!("Running {} inventory/listing commands…", steps.len());
    let outs = run_parallel(&steps, |(_, cmd, _)| ctx.run(cmd, None));
    let mut runs: Vec<Run> = steps
        .into_iter()
        .zip(outs)
        .map(|((label, _, org), out)| Run { label, org, out })
        .collect();

    // 2. Probes (Access Matrix). 2GP is also tried on non-hubs: that is how
    //    the "not a Dev Hub" error (U5) and 1GP behaviour (U7) get captured.
    let mut matrix: Vec<ProbeResult> = Vec::new();
    if probes {
        eprintln!("Probing {} org(s)…", probe_orgs_list.len());
        matrix = probe_orgs(ctx, &probe_orgs_list, true);
        for r in &mut matrix {
            if let Some(out) = r.out.take() {
                runs.push(Run {
                    label: format!("probe-{}", r.cap.name().replace('.', "-").to_lowercase()),
                    org: Some(r.org),
                    out: Ok(out.clone()),
                });
                r.out = Some(out);
            }
        }
    }

    // 3. Sanitize and write.
    let mut s = Sanitizer::new();
    s.seed_orgs(&orgs);
    let org_label = |s: &mut Sanitizer, i: usize| s.text(orgs[i].target());
    let mut written = 0usize;
    for (n, run) in runs.iter().enumerate() {
        let org = run.org.map(|i| org_label(&mut s, i));
        let stem = format!(
            "{:03}-{}{}",
            n + 1,
            run.label,
            org.as_ref().map(|o| format!("__{o}")).unwrap_or_default()
        );
        let out = match &run.out {
            Ok(o) => o,
            Err(e) => {
                write(
                    dir,
                    &format!("{stem}.meta.json"),
                    &pretty(&json!({"run_error": s.text(e)})),
                )?;
                continue;
            }
        };
        match serde_json::from_slice::<Value>(&out.stdout) {
            Ok(mut v) => {
                if run.label == "plugins" {
                    v = trim_plugins(v);
                }
                s.value(&mut v);
                write(dir, &format!("{stem}.json"), &pretty(&v))?;
            }
            Err(_) if !out.stdout.is_empty() => {
                let t = s.text(&String::from_utf8_lossy(&out.stdout));
                write(dir, &format!("{stem}.stdout.txt"), &t)?;
            }
            Err(_) => {}
        }
        if !out.stderr.is_empty() {
            let t = s.text(&String::from_utf8_lossy(&out.stderr));
            write(dir, &format!("{stem}.stderr.txt"), &t)?;
        }
        let c = classify(out);
        let argv: Vec<String> = out.argv.iter().map(|a| s.text(a)).collect();
        let meta = json!({
            "argv": argv,
            "exit_code": out.exit_code,
            "duration_ms": out.duration.as_millis() as u64,
            "timed_out": out.timed_out,
            "truncated": out.truncated,
            "state": c.state.short(),
            "rule": c.rule,
            "error_name": c.error.as_ref().map(|e| e.name.clone()),
            "error_code": c.error.as_ref().and_then(|e| e.code.clone()),
        });
        write(dir, &format!("{stem}.meta.json"), &pretty(&meta))?;
        written += 1;
    }

    // 4. Summary.
    let mut md = String::new();
    let _ = writeln!(md, "# GP Atlas capture summary\n");
    let _ = writeln!(md, "- gp-atlas-beta {}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(
        md,
        "- OS: {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let _ = writeln!(
        md,
        "- Orgs: {} ({} Dev Hubs, {} sandboxes, {} scratch); probed: {}",
        orgs.len(),
        orgs.iter().filter(|o| o.kind == OrgKind::DevHub).count(),
        orgs.iter().filter(|o| o.kind == OrgKind::Sandbox).count(),
        orgs.iter().filter(|o| o.kind == OrgKind::Scratch).count(),
        if probes { probe_orgs_list.len() } else { 0 }
    );
    let _ = writeln!(md, "- Runs saved: {written}\n");
    if probes {
        let _ = writeln!(md, "## Access Matrix\n");
        let _ = write!(md, "| Org | Type | Connected |");
        for c in Capability::ALL {
            let _ = write!(md, " {} |", c.name());
        }
        let _ = writeln!(md);
        let _ = writeln!(md, "|---|---|---|{}", "---|".repeat(Capability::ALL.len()));
        for (i, o) in probe_orgs_list.iter().enumerate() {
            let conn = o
                .connected_status
                .as_deref()
                .map(|c| s.text(c))
                .unwrap_or_default();
            let _ = write!(
                md,
                "| {} | {} | {} |",
                s.text(o.target()),
                o.kind.badge(),
                conn.replace('\n', " ")
            );
            for c in Capability::ALL {
                let st = matrix
                    .iter()
                    .find(|r| r.org == i && r.cap == c)
                    .map(|r| r.state.short())
                    .unwrap_or_default();
                let _ = write!(md, " {st} |");
            }
            let _ = writeln!(md);
        }
        let _ = writeln!(md, "\n## Distinct failures (for U5/U7)\n");
        let mut seen = std::collections::BTreeSet::new();
        for r in &matrix {
            if matches!(r.state, CapState::Allowed | CapState::NotApplicable(_)) {
                continue;
            }
            if let Some(c) = &r.classified {
                let (name, msg) = match (&c.error, &r.out) {
                    (Some(e), _) => (e.name.clone(), s.text(e.message.trim())),
                    // No error envelope: show exit code and the first stderr line.
                    (None, Some(out)) => {
                        let err = String::from_utf8_lossy(&out.stderr);
                        let first = err.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
                        (format!("exit {:?}", out.exit_code), s.text(first.trim()))
                    }
                    (None, None) => Default::default(),
                };
                let key = format!(
                    "{} | {} | {} | {}",
                    r.cap.name(),
                    r.state.short(),
                    name,
                    msg.replace('\n', " ")
                );
                if seen.insert(key.clone()) {
                    let _ = writeln!(md, "- {key}  (rule: {})", c.rule);
                }
            }
        }
    }
    write(dir, "summary.md", &md)?;
    write(dir, "README.txt", README)?;
    crate::out!("Saved {written} runs to {}", dir.display());
    crate::out!("Review the folder (see README.txt), then zip it and send it.");
    Ok(())
}

fn trim_plugins(v: Value) -> Value {
    match v {
        Value::Array(a) => Value::Array(
            a.into_iter()
                .map(|p| json!({"name": p.get("name"), "version": p.get("version"), "type": p.get("type")}))
                .collect(),
        ),
        other => other,
    }
}

fn pretty(v: &Value) -> String {
    let mut s = serde_json::to_string_pretty(v).unwrap_or_default();
    s.push('\n');
    s
}

fn write(dir: &Path, name: &str, contents: &str) -> Result<(), ExitCode> {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "-_.@".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    std::fs::write(dir.join(safe), contents).map_err(|e| {
        eprintln!("error: cannot write {name}: {e}");
        ExitCode::from(2)
    })
}
