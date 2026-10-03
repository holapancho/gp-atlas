//! `gp-atlas-beta`: command-line debug frontend for GP Atlas.
//!
//! Runs only the read-only commands of `gp_atlas_core::command::ReadOnlyCommand`
//! through the same core the desktop app will use, and can capture sanitized
//! fixtures from real orgs (`capture`).

/// `println!` that exits quietly when stdout is closed (e.g. piped to `head`).
#[macro_export]
macro_rules! out {
    () => {{
        use std::io::Write as _;
        if writeln!(std::io::stdout()).is_err() {
            std::process::exit(0);
        }
    }};
    ($($t:tt)*) => {{
        use std::io::Write as _;
        if writeln!(std::io::stdout(), $($t)*).is_err() {
            std::process::exit(0);
        }
    }};
}

mod capture;
mod sanitize;
mod table;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Mutex;
use std::time::Duration;

use clap::{Parser, Subcommand};
use gp_atlas_core::classify::{CapState, Classified, classify};
use gp_atlas_core::command::{
    CreateStatus, OrderBy, PkgVersionListArgs, ReadOnlyCommand, parse_packages,
};
use gp_atlas_core::doctor::{self, Check};
use gp_atlas_core::envelope::{self, SfOutput};
use gp_atlas_core::ids::{Branch, Id04t, Id033, OrgRef, PackageAlias, PackageRef};
use gp_atlas_core::manifest::Manifest;
use gp_atlas_core::orgs::{self, Org};
use gp_atlas_core::probes::{Capability, org_ref};
use gp_atlas_core::runner::{RunOutput, RunnerConfig, SfRunner, resolve_sf_bin};
use gp_atlas_core::{REQUIRED_CLI_VERSION_STRING, versions};
use serde_json::Value;

use crate::table::Table;

#[derive(Parser)]
#[command(
    name = "gp-atlas-beta",
    version,
    about = "GP Atlas beta: browse 1GP/2GP package versions through your installed sf (read-only).",
    long_about = "GP Atlas beta (debug build). Read-only companion to the Salesforce CLI.\n\
                  Not affiliated with or endorsed by Salesforce.\n\
                  Requires @salesforce/cli 2.150.6 exactly."
)]
struct Cli {
    /// Path to the sf binary (default: $GP_ATLAS_SF_BIN, then `sf` on PATH).
    #[arg(long, global = true)]
    sf: Option<PathBuf>,
    /// Per-command timeout in seconds.
    #[arg(long, global = true, default_value_t = 120)]
    timeout: u64,
    /// Print every sf invocation (argv, exit code, duration, stderr) to stderr.
    #[arg(long, short = 'd', global = true)]
    debug: bool,
    /// Print the (secret-scrubbed) JSON result instead of a table.
    #[arg(long, global = true)]
    raw: bool,
    /// Developer override: allow this CLI version (e.g. 2.152.14). Results may be wrong.
    #[arg(long, global = true, env = "GP_ATLAS_ALLOW_CLI_VERSION")]
    allow_cli_version: Option<String>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// D1–D4: sf found, exact version, bundled packaging plugin, command contract.
    Doctor,
    /// List orgs known to sf (alias — username, type, connection status).
    Orgs {
        /// Skip the live connection check (faster, less accurate).
        #[arg(long)]
        fast: bool,
    },
    /// Access Matrix: probe what each org lets you read.
    Probe {
        /// Org alias or username (repeatable). Default: all orgs.
        #[arg(long = "org", short = 'o')]
        orgs: Vec<String>,
        /// Also run 2GP probes on orgs not flagged as Dev Hub.
        #[arg(long)]
        try_anyway: bool,
        /// Show the raw error name/message and matched rule for each failure.
        #[arg(long)]
        details: bool,
    },
    /// 2GP packages in a Dev Hub.
    Packages {
        #[arg(long, short = 'v')]
        hub: String,
    },
    /// 2GP package versions in a Dev Hub.
    Versions {
        #[arg(long, short = 'v')]
        hub: String,
        /// Comma-separated 0Ho ids (or project aliases, with --project).
        #[arg(long, short = 'p')]
        packages: Option<String>,
        #[arg(long, short = 'r')]
        released: bool,
        #[arg(long, short = 'b')]
        branch: Option<String>,
        #[arg(long)]
        created_last_days: Option<u32>,
        #[arg(long)]
        modified_last_days: Option<u32>,
        /// e.g. "CreatedDate DESC,Name" (allow-listed fields only).
        #[arg(long)]
        order_by: Option<String>,
        #[arg(long)]
        verbose: bool,
        #[arg(long)]
        conversions_only: bool,
        /// Only the latest released version per package (computed by GP Atlas).
        #[arg(long)]
        latest: bool,
        /// sfdx project directory: run there so package aliases resolve.
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Details of one package version (`package version report --verbose`).
    Report {
        #[arg(long, short = 'v')]
        hub: String,
        /// 04t id or package alias.
        #[arg(long, short = 'p')]
        package: String,
    },
    /// Version-creation requests (read-only monitoring).
    Builds {
        #[arg(long, short = 'v')]
        hub: String,
        #[arg(long)]
        created_last_days: Option<u32>,
        /// Queued | InProgress | Success | Error
        #[arg(long)]
        status: Option<String>,
    },
    /// Packages installed in an org.
    Installed {
        #[arg(long, short = 'o')]
        org: String,
    },
    /// 1GP package versions in a packaging org.
    Pkg1 {
        #[arg(long, short = 'o')]
        org: String,
        /// 033 package id (18 characters).
        #[arg(long, short = 'i')]
        package_id: Option<String>,
    },
    /// Print the exact sf command for a probe/browse action without running it.
    Show {
        #[command(subcommand)]
        what: ShowCmd,
    },
    /// Run every read-only command against your orgs and save SANITIZED output
    /// for the GP Atlas fixtures. Review the folder before sharing it.
    Capture {
        /// Output folder (must not exist yet).
        #[arg(long)]
        out: PathBuf,
        /// Skip the per-org probes (much faster).
        #[arg(long)]
        no_probes: bool,
        /// Limit how many orgs are probed (0 = all).
        #[arg(long, default_value_t = 0)]
        max_orgs: usize,
    },
}

#[derive(Subcommand)]
enum ShowCmd {
    /// The probe commands for one org.
    Probes {
        #[arg(long, short = 'o')]
        org: String,
    },
}

pub struct Ctx {
    pub runner: SfRunner,
    pub manifest: Manifest,
    pub debug: bool,
    pub raw: bool,
    pub allow: Option<String>,
    log: Mutex<()>,
}

impl Ctx {
    /// Runs one command, printing debug info if requested.
    pub fn run(
        &self,
        cmd: &ReadOnlyCommand,
        cwd: Option<&std::path::Path>,
    ) -> Result<RunOutput, String> {
        let out = self
            .runner
            .run(cmd, &self.manifest, cwd, None)
            .map_err(|e| e.to_string())?;
        if self.debug {
            let _g = self.log.lock();
            eprintln!("→ sf {}", out.argv.join(" "));
            eprintln!(
                "← exit {} in {:.1}s, stdout {} B, stderr {} B{}{}",
                out.exit_code.map_or("killed".to_owned(), |c| c.to_string()),
                out.duration.as_secs_f64(),
                out.stdout.len(),
                out.stderr.len(),
                if out.timed_out { ", TIMED OUT" } else { "" },
                if out.truncated { ", TRUNCATED" } else { "" },
            );
            let err = String::from_utf8_lossy(&out.stderr);
            for line in err.lines().filter(|l| !l.trim().is_empty()).take(20) {
                eprintln!("  stderr: {line}");
            }
        }
        Ok(out)
    }

    /// Runs a command and returns its `result`, or prints the classified error.
    fn result(
        &self,
        cmd: &ReadOnlyCommand,
        cwd: Option<&std::path::Path>,
    ) -> Result<(Value, Vec<String>), ExitCode> {
        let out = self.run(cmd, cwd).map_err(|e| {
            eprintln!("error: {e}");
            ExitCode::from(2)
        })?;
        match envelope::parse(&out.stdout) {
            Ok(SfOutput::Success { result, warnings }) => Ok((result, warnings)),
            Ok(SfOutput::Bare(v)) if out.exit_code == Some(0) => Ok((v, vec![])),
            _ => {
                let c = classify(&out);
                print_failure(&c, cmd.target().map(OrgRef::as_str).unwrap_or(""));
                if !self.debug {
                    eprintln!("(re-run with --debug to see the sf command and stderr)");
                }
                Err(ExitCode::from(1))
            }
        }
    }
}

fn print_failure(c: &Classified, org: &str) {
    eprintln!("✗ {}", c.state.short());
    if let Some(e) = &c.error {
        eprintln!("  {}: {}", e.name, e.message.trim());
        for a in &e.actions {
            eprintln!("  action: {a}");
        }
    }
    eprintln!("  rule: {}", c.rule);
    if let Some(h) = c.state.hint(org) {
        eprintln!("  fix (copy, not run by GP Atlas): {h}");
    }
}

fn org(s: &str) -> Result<OrgRef, ExitCode> {
    OrgRef::new(s).map_err(|e| {
        eprintln!("error: {e}");
        ExitCode::from(2)
    })
}

/// Version gate (§4.2). Returns false if sf features must stay disabled.
fn gate(ctx: &Ctx) -> Result<(), ExitCode> {
    let out = ctx.run(&ReadOnlyCommand::Version, None).map_err(|e| {
        eprintln!("✗ CliMissing / not runnable: {e}");
        eprintln!("  fix: {}", gp_atlas_core::CLI_INSTALL_COMMAND);
        ExitCode::from(3)
    })?;
    let v = match envelope::parse(&out.stdout) {
        Ok(SfOutput::Bare(v)) => v,
        _ => {
            eprintln!(
                "✗ `sf version --json` did not return JSON (run `gp-atlas-beta doctor --debug`)"
            );
            return Err(ExitCode::from(3));
        }
    };
    let (check, found) = doctor::check_version(&v);
    if check.is_pass() {
        return Ok(());
    }
    let found = found.unwrap_or_default();
    if let Some(allow) = &ctx.allow
        && (found == *allow || found.ends_with(&format!("/{allow}")))
    {
        eprintln!("⚠ Unsupported CLI version {found} — results may be wrong (override active).");
        return Ok(());
    }
    eprintln!("✗ CliVersionMismatch: found {found}, required {REQUIRED_CLI_VERSION_STRING}.");
    eprintln!(
        "  All sf features are disabled. Fix: {}",
        gp_atlas_core::CLI_INSTALL_COMMAND
    );
    Err(ExitCode::from(3))
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let (bin, _) = match resolve_sf_bin(cli.sf.as_deref()) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("✗ {e}");
            return ExitCode::from(3);
        }
    };
    let manifest = match Manifest::embedded() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("✗ embedded manifest is invalid: {e}");
            return ExitCode::from(4);
        }
    };
    let mut cfg = RunnerConfig::new(bin);
    cfg.timeout = Duration::from_secs(cli.timeout.max(1));
    let ctx = Ctx {
        runner: SfRunner::new(cfg),
        manifest,
        debug: cli.debug,
        raw: cli.raw,
        allow: cli.allow_cli_version.clone(),
        log: Mutex::new(()),
    };
    if cli.debug {
        eprintln!("sf binary: {}", ctx.runner.cfg.bin.display());
    }
    let r = match cli.cmd {
        Cmd::Doctor => cmd_doctor(&ctx),
        Cmd::Show { what } => cmd_show(&ctx, what),
        other => gate(&ctx).and_then(|()| dispatch(&ctx, other)),
    };
    r.err().unwrap_or(ExitCode::SUCCESS)
}

fn dispatch(ctx: &Ctx, cmd: Cmd) -> Result<(), ExitCode> {
    match cmd {
        Cmd::Orgs { fast } => cmd_orgs(ctx, fast),
        Cmd::Probe {
            orgs,
            try_anyway,
            details,
        } => cmd_probe(ctx, &orgs, try_anyway, details),
        Cmd::Packages { hub } => cmd_packages(ctx, &hub),
        Cmd::Versions {
            hub,
            packages,
            released,
            branch,
            created_last_days,
            modified_last_days,
            order_by,
            verbose,
            conversions_only,
            latest,
            project,
        } => {
            let mut a = PkgVersionListArgs::new(org(&hub)?);
            if let Some(p) = packages {
                a.packages = parse_packages(&p).map_err(bad)?;
            }
            a.released = released || latest;
            a.branch = branch
                .as_deref()
                .map(Branch::new)
                .transpose()
                .map_err(bad)?;
            a.created_last_days = created_last_days;
            a.modified_last_days = modified_last_days;
            if let Some(o) = order_by {
                a.order_by = o
                    .split(',')
                    .map(|t| OrderBy::parse(t.trim()))
                    .collect::<Result<_, _>>()
                    .map_err(bad)?;
            }
            a.verbose = verbose;
            a.show_conversions_only = conversions_only;
            cmd_versions(ctx, a, latest, project)
        }
        Cmd::Report { hub, package } => {
            let package = match Id04t::new(&package) {
                Ok(id) => PackageRef::Id(id),
                Err(_) => PackageRef::Alias(PackageAlias::new(&package).map_err(bad)?),
            };
            let cmd = ReadOnlyCommand::PkgVersionReport {
                hub: org(&hub)?,
                package,
                verbose: true,
            };
            let (v, _) = ctx.result(&cmd, None)?;
            print_kv(ctx, &v);
            Ok(())
        }
        Cmd::Builds {
            hub,
            created_last_days,
            status,
        } => {
            let status = match status.as_deref() {
                None => None,
                Some("Queued") => Some(CreateStatus::Queued),
                Some("InProgress") => Some(CreateStatus::InProgress),
                Some("Success") => Some(CreateStatus::Success),
                Some("Error") => Some(CreateStatus::Error),
                Some(other) => {
                    eprintln!(
                        "error: --status must be Queued, InProgress, Success or Error (got {other:?})"
                    );
                    return Err(ExitCode::from(2));
                }
            };
            let cmd = ReadOnlyCommand::PkgCreateList {
                hub: org(&hub)?,
                created_last_days,
                status,
                show_conversions_only: false,
                verbose: false,
            };
            let (v, w) = ctx.result(&cmd, None)?;
            print_rows(
                ctx,
                &v,
                &w,
                &[
                    ("Id", "Request"),
                    ("Status", "Status"),
                    ("Package2Name", "Package"),
                    ("VersionNumber", "Version"),
                    ("SubscriberPackageVersionId", "04t"),
                    ("Branch", "Branch"),
                    ("CreatedDate", "Created"),
                ],
            );
            Ok(())
        }
        Cmd::Installed { org: o } => {
            let cmd = ReadOnlyCommand::PkgInstalledList { org: org(&o)? };
            let (v, w) = ctx.result(&cmd, None)?;
            print_rows(
                ctx,
                &v,
                &w,
                &[
                    ("SubscriberPackageName", "Package"),
                    ("SubscriberPackageNamespace", "Namespace"),
                    ("SubscriberPackageVersionNumber", "Version"),
                    ("SubscriberPackageVersionId", "04t"),
                    ("VersionSettings", "Version Settings"),
                ],
            );
            Ok(())
        }
        Cmd::Pkg1 { org: o, package_id } => {
            let package_id = package_id
                .as_deref()
                .map(Id033::new)
                .transpose()
                .map_err(bad)?;
            let cmd = ReadOnlyCommand::Pkg1VersionList {
                org: org(&o)?,
                package_id,
            };
            let (v, w) = ctx.result(&cmd, None)?;
            print_rows(
                ctx,
                &v,
                &w,
                &[
                    ("Name", "Name"),
                    ("ReleaseState", "Release State"),
                    ("Version", "Version"),
                    ("BuildNumber", "Build"),
                    ("MetadataPackageVersionId", "04t"),
                    ("MetadataPackageId", "033"),
                ],
            );
            Ok(())
        }
        Cmd::Capture {
            out,
            no_probes,
            max_orgs,
        } => capture::run(ctx, &out, !no_probes, max_orgs),
        Cmd::Doctor | Cmd::Show { .. } => unreachable!(),
    }
}

fn bad(e: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {e}");
    ExitCode::from(2)
}

fn check_line(name: &str, c: &Check) {
    let (icon, text) = match c {
        Check::Pass(t) => ("✅", t),
        Check::Fail(t) => ("⛔", t),
        Check::Skipped(t) => ("–", t),
    };
    out!("{icon} {name:<28} {text}");
}

fn cmd_doctor(ctx: &Ctx) -> Result<(), ExitCode> {
    out!("sf binary: {}", ctx.runner.cfg.bin.display());
    let r = doctor::run(&ctx.runner, &ctx.manifest);
    if ctx.debug {
        for out in &r.runs {
            eprintln!(
                "→ sf {}  ← exit {:?} in {:.1}s",
                out.argv.join(" "),
                out.exit_code,
                out.duration.as_secs_f64()
            );
        }
    }
    check_line("D1 sf runnable", &r.d1_runnable);
    check_line("D2 exact CLI version", &r.d2_version);
    check_line("D3 bundled packaging plugin", &r.d3_packaging_plugin);
    check_line("D4 command contract", &r.d4_contract);
    for d in &r.drift {
        out!("   drift in `sf {}`: {}", d.command, d.details.join("; "));
    }
    if r.all_green() {
        out!("\nAll checks green.");
        Ok(())
    } else {
        Err(ExitCode::from(1))
    }
}

fn cmd_show(ctx: &Ctx, what: ShowCmd) -> Result<(), ExitCode> {
    match what {
        ShowCmd::Probes { org: o } => {
            let o = org(&o)?;
            for c in Capability::ALL {
                out!(
                    "{:<20} {}",
                    c.name(),
                    c.probe(o.clone()).display(&ctx.manifest)
                );
            }
        }
    }
    Ok(())
}

/// Loads the org inventory.
pub fn load_orgs(ctx: &Ctx, fast: bool) -> Result<Vec<Org>, ExitCode> {
    let cmd = ReadOnlyCommand::OrgList {
        skip_connection_status: fast,
        all: false,
    };
    let (v, _) = ctx.result(&cmd, None)?;
    Ok(orgs::from_org_list(&v))
}

fn cmd_orgs(ctx: &Ctx, fast: bool) -> Result<(), ExitCode> {
    let orgs = load_orgs(ctx, fast)?;
    let mut t = Table::new(&["", "Org", "Type", "Dev Hub", "Status"]);
    for o in &orgs {
        let marker = match (o.is_default_dev_hub, o.is_default_org) {
            (true, true) => "(D)(U)",
            (true, false) => "(D)",
            (false, true) => "(U)",
            _ => "",
        };
        let status = o
            .connected_status
            .clone()
            .or_else(|| o.status.clone())
            .unwrap_or_else(|| if fast { "(skipped)".into() } else { "".into() });
        t.row(vec![
            marker.into(),
            o.label(),
            o.kind.badge().into(),
            if o.is_dev_hub {
                "yes".into()
            } else {
                "".into()
            },
            status,
        ]);
    }
    t.print();
    out!("{} orgs. (D)=default Dev Hub, (U)=default org", orgs.len());
    Ok(())
}

/// Runs jobs with at most 3 concurrent sf processes (§4.6).
pub fn run_parallel<J: Sync, R: Send>(jobs: &[J], f: impl Fn(&J) -> R + Sync) -> Vec<R> {
    let next = Mutex::new(0usize);
    let results: Mutex<Vec<Option<R>>> = Mutex::new((0..jobs.len()).map(|_| None).collect());
    std::thread::scope(|s| {
        for _ in 0..3 {
            s.spawn(|| {
                loop {
                    let i = {
                        let mut n = next.lock().unwrap();
                        let i = *n;
                        *n += 1;
                        i
                    };
                    if i >= jobs.len() {
                        break;
                    }
                    let r = f(&jobs[i]);
                    results.lock().unwrap()[i] = Some(r);
                }
            });
        }
    });
    results
        .into_inner()
        .unwrap()
        .into_iter()
        .map(|r| r.expect("job finished"))
        .collect()
}

/// One probe outcome.
pub struct ProbeResult {
    pub org: usize,
    pub cap: Capability,
    pub state: CapState,
    pub classified: Option<Classified>,
    pub out: Option<RunOutput>,
}

/// Probes the given orgs (§5.2 L2). Not-applicable cells are not run unless `try_anyway`.
pub fn probe_orgs(ctx: &Ctx, orgs: &[Org], try_anyway: bool) -> Vec<ProbeResult> {
    let mut jobs = Vec::new();
    let mut skipped = Vec::new();
    for (i, o) in orgs.iter().enumerate() {
        for cap in Capability::ALL {
            match cap.applies_to(o) {
                Ok(()) => jobs.push((i, cap)),
                Err(r)
                    if try_anyway
                        && cap == Capability::Pkg2ListPackages
                        && r == "not flagged as Dev Hub" =>
                {
                    jobs.push((i, cap))
                }
                Err(r) => skipped.push(ProbeResult {
                    org: i,
                    cap,
                    state: CapState::NotApplicable(r.into()),
                    classified: None,
                    out: None,
                }),
            }
        }
    }
    let done = Mutex::new(0usize);
    let total = jobs.len();
    let mut results = run_parallel(&jobs, |(i, cap)| {
        let o = &orgs[*i];
        let r = match org_ref(o) {
            Err(e) => ProbeResult {
                org: *i,
                cap: *cap,
                state: CapState::NotApplicable(format!("unsupported alias/username: {e}")),
                classified: None,
                out: None,
            },
            Ok(target) => match ctx.run(&cap.probe(target), None) {
                Ok(out) => {
                    let c = classify(&out);
                    ProbeResult {
                        org: *i,
                        cap: *cap,
                        state: c.state.clone(),
                        classified: Some(c),
                        out: Some(out),
                    }
                }
                Err(e) => {
                    eprintln!("run error: {e}");
                    ProbeResult {
                        org: *i,
                        cap: *cap,
                        state: CapState::Unknown(
                            gp_atlas_core::classify::UnknownReason::Unclassified,
                        ),
                        classified: None,
                        out: None,
                    }
                }
            },
        };
        let mut d = done.lock().unwrap();
        *d += 1;
        if !ctx.debug {
            eprint!("\rprobing… {}/{}   ", *d, total);
        }
        r
    });
    if !ctx.debug && total > 0 {
        eprintln!();
    }
    results.extend(skipped);
    results.sort_by_key(|r| (r.org, r.cap));
    results
}

fn cell(state: &CapState) -> String {
    match state {
        CapState::Allowed => "✅".into(),
        CapState::Denied(r) => format!("⛔ {r:?}"),
        CapState::NotApplicable(_) => "·".into(),
        CapState::Unreachable(r) => format!("🔌 {r:?}"),
        CapState::Unknown(r) => format!("? {r:?}"),
        CapState::ContractDrift(_) => "⚠ drift".into(),
    }
}

fn cmd_probe(ctx: &Ctx, only: &[String], try_anyway: bool, details: bool) -> Result<(), ExitCode> {
    let mut orgs = load_orgs(ctx, true)?;
    if !only.is_empty() {
        orgs.retain(|o| {
            only.iter()
                .any(|x| Some(x) == o.alias.as_ref() || *x == o.username)
        });
        if orgs.is_empty() {
            eprintln!("error: none of {only:?} is in `sf org list`");
            return Err(ExitCode::from(2));
        }
    }
    let results = probe_orgs(ctx, &orgs, try_anyway);
    let mut header = vec!["Org", "Type"];
    header.extend(Capability::ALL.iter().map(|c| c.short()));
    let mut t = Table::new(&header);
    for (i, o) in orgs.iter().enumerate() {
        let mut row = vec![o.label(), o.kind.badge().to_owned()];
        for cap in Capability::ALL {
            let r = results.iter().find(|r| r.org == i && r.cap == cap);
            row.push(r.map(|r| cell(&r.state)).unwrap_or_default());
        }
        t.row(row);
    }
    t.print();
    out!(
        "✅ allowed  ⛔ denied  🔌 unreachable  ? unknown  · not applicable (use --try-anyway for 2GP on non-hubs)"
    );
    if details {
        for r in results
            .iter()
            .filter(|r| !matches!(r.state, CapState::Allowed | CapState::NotApplicable(_)))
        {
            let o = &orgs[r.org];
            out!("\n{} / {}: {}", o.label(), r.cap.name(), r.state.short());
            if let Some(out) = &r.out {
                out!(
                    "  ran: sf {}  (exit {:?}, {:.1}s)",
                    out.argv.join(" "),
                    out.exit_code,
                    out.duration.as_secs_f64()
                );
            }
            if let Some(c) = &r.classified {
                if let Some(e) = &c.error {
                    out!("  {}: {}", e.name, e.message.trim().replace('\n', " "));
                }
                out!("  rule: {}", c.rule);
            }
            if let Some(h) = r.state.hint(o.target()) {
                out!("  fix (copy only): {h}");
            }
        }
    }
    Ok(())
}

fn cmd_packages(ctx: &Ctx, hub: &str) -> Result<(), ExitCode> {
    let cmd = ReadOnlyCommand::PkgList {
        hub: org(hub)?,
        verbose: false,
        api_version: None,
    };
    let (v, w) = ctx.result(&cmd, None)?;
    print_rows(
        ctx,
        &v,
        &w,
        &[
            ("Name", "Name"),
            ("NamespacePrefix", "Namespace"),
            ("ContainerOptions", "Type"),
            ("IsOrgDependent", "Org-dependent"),
            ("Id", "0Ho"),
            ("SubscriberPackageId", "033"),
        ],
    );
    Ok(())
}

fn cmd_versions(
    ctx: &Ctx,
    a: PkgVersionListArgs,
    latest: bool,
    project: Option<PathBuf>,
) -> Result<(), ExitCode> {
    let cmd = ReadOnlyCommand::PkgVersionList(a);
    if cmd.uses_project_alias() && project.is_none() {
        eprintln!(
            "error: package aliases resolve only inside the sfdx project; pass --project <dir> or use 0Ho ids"
        );
        return Err(ExitCode::from(2));
    }
    let (v, w) = ctx.result(&cmd, project.as_deref())?;
    let rows: Vec<Value> = v.as_array().cloned().unwrap_or_default();
    let shown: Value = if latest {
        out!("Latest released per package (computed by GP Atlas):");
        Value::Array(
            versions::latest_released(&rows)
                .into_iter()
                .cloned()
                .collect(),
        )
    } else {
        Value::Array(rows)
    };
    let mut cols = vec![
        ("Package2Name", "Package"),
        ("Version", "Version"),
        ("SubscriberPackageVersionId", "04t"),
        ("IsReleased", "Released"),
        ("Branch", "Branch"),
        ("AncestorVersion", "Ancestor"),
        ("CreatedDate", "Created"),
    ];
    if matches!(&cmd, ReadOnlyCommand::PkgVersionList(a) if a.verbose) {
        cols.push(("CodeCoverage", "Coverage"));
    }
    print_rows(ctx, &shown, &w, &cols);
    Ok(())
}

fn fmt_value(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

fn print_rows(ctx: &Ctx, v: &Value, warnings: &[String], cols: &[(&str, &str)]) {
    if ctx.raw {
        out!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
        return;
    }
    let rows = v.as_array().cloned().unwrap_or_default();
    if rows.is_empty() {
        out!("(no results)");
    } else {
        let header: Vec<&str> = cols.iter().map(|(_, h)| *h).collect();
        let mut t = Table::new(&header);
        for r in &rows {
            t.row(cols.iter().map(|(k, _)| fmt_value(r.get(*k))).collect());
        }
        t.print();
        out!("{} row(s)", rows.len());
    }
    for w in warnings {
        eprintln!("warning: {w}");
    }
}

fn print_kv(ctx: &Ctx, v: &Value) {
    if ctx.raw {
        out!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
        return;
    }
    let obj = v.as_array().and_then(|a| a.first()).unwrap_or(v);
    if let Some(o) = obj.as_object() {
        for (k, val) in o {
            let s = match val {
                Value::Object(_) | Value::Array(_) => val.to_string(),
                _ => fmt_value(Some(val)),
            };
            out!("{k:<32} {s}");
        }
    } else {
        out!("{v}");
    }
}
