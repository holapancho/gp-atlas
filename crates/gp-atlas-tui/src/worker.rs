//! Background execution of `sf` commands (SPEC §1 rule 6: UI never blocks).
//!
//! A pool of 3 worker threads (§4.6: max 3 concurrent `sf`) pulls jobs from a
//! two-level queue: interactive requests first, Access Matrix probes after.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

use gp_atlas_core::classify::{CapState, Classified, UnknownReason, classify};
use gp_atlas_core::command::{PkgVersionListArgs, ReadOnlyCommand};
use gp_atlas_core::doctor::{self, DoctorReport};
use gp_atlas_core::envelope::{self, SfOutput};
use gp_atlas_core::ids::{Id04t, OrgRef, PackageRef};
use gp_atlas_core::manifest::Manifest;
use gp_atlas_core::probes::Capability;
use gp_atlas_core::runner::{RunOutput, SfRunner};
use serde_json::Value;

/// Work the UI can ask for.
#[derive(Debug, Clone)]
pub enum Job {
    Doctor,
    Orgs {
        fast: bool,
    },
    Probe {
        username: String,
        cap: Capability,
        target: OrgRef,
    },
    Packages {
        hub: OrgRef,
    },
    Versions {
        args: Box<PkgVersionListArgs>,
    },
    Report {
        hub: OrgRef,
        version: Id04t,
    },
    Installed {
        org: OrgRef,
    },
    Pkg1 {
        org: OrgRef,
    },
}

impl Job {
    pub fn command(&self) -> Option<ReadOnlyCommand> {
        Some(match self {
            Job::Doctor => return None,
            Job::Orgs { fast } => ReadOnlyCommand::OrgList {
                skip_connection_status: *fast,
                all: false,
            },
            Job::Probe { cap, target, .. } => cap.probe(target.clone()),
            Job::Packages { hub } => ReadOnlyCommand::PkgList {
                hub: hub.clone(),
                verbose: false,
                api_version: None,
            },
            Job::Versions { args } => ReadOnlyCommand::PkgVersionList((**args).clone()),
            Job::Report { hub, version } => ReadOnlyCommand::PkgVersionReport {
                hub: hub.clone(),
                package: PackageRef::Id(version.clone()),
                verbose: true,
            },
            Job::Installed { org } => ReadOnlyCommand::PkgInstalledList { org: org.clone() },
            Job::Pkg1 { org } => ReadOnlyCommand::Pkg1VersionList {
                org: org.clone(),
                package_id: None,
            },
        })
    }
}

/// A failed run, with everything needed to explain it (§5.4 drawer).
#[derive(Debug, Clone)]
pub struct Failure {
    pub state: CapState,
    pub rule: &'static str,
    pub name: String,
    pub message: String,
    pub stderr: String,
}

impl Failure {
    fn from_run(c: &Classified, out: &RunOutput) -> Self {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        Self {
            state: c.state.clone(),
            rule: c.rule,
            name: c.error.as_ref().map(|e| e.name.clone()).unwrap_or_default(),
            message: c
                .error
                .as_ref()
                .map(|e| e.message.trim().to_owned())
                .unwrap_or_else(|| stderr.lines().next().unwrap_or("").to_owned()),
            stderr,
        }
    }

    pub fn simple(msg: impl Into<String>) -> Self {
        Self {
            state: CapState::Unknown(UnknownReason::Unclassified),
            rule: "",
            name: String::new(),
            message: msg.into(),
            stderr: String::new(),
        }
    }
}

/// One executed `sf` call (History / Log tab, §8).
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub at: SystemTime,
    pub command: String,
    pub exit: Option<i32>,
    pub duration: Duration,
    pub state: String,
    pub rule: &'static str,
    pub stderr: String,
}

#[derive(Debug)]
pub enum Outcome {
    Doctor(Box<DoctorReport>),
    Ok { value: Value, warnings: Vec<String> },
    Failed(Failure),
    Cancelled,
}

#[derive(Debug)]
pub struct Done {
    pub id: u64,
    pub job: Job,
    pub outcome: Outcome,
    pub log: Vec<LogEntry>,
}

pub struct Request {
    pub id: u64,
    pub job: Job,
    pub cancel: Arc<AtomicBool>,
    pub low_priority: bool,
}

#[derive(Default)]
struct Queues {
    high: VecDeque<Request>,
    low: VecDeque<Request>,
}

/// Handle used by the UI to submit jobs.
#[derive(Clone)]
pub struct Pool {
    queues: Arc<(Mutex<Queues>, Condvar)>,
}

impl Pool {
    pub fn start(runner: SfRunner, manifest: Manifest, done: Sender<Done>) -> Self {
        let queues: Arc<(Mutex<Queues>, Condvar)> = Arc::default();
        let shared = Arc::new((runner, manifest));
        for _ in 0..3 {
            let queues = queues.clone();
            let shared = shared.clone();
            let done = done.clone();
            thread::spawn(move || {
                loop {
                    let req = {
                        let (lock, cv) = &*queues;
                        let mut q = lock.lock().unwrap();
                        loop {
                            if let Some(r) = q.high.pop_front().or_else(|| q.low.pop_front()) {
                                break r;
                            }
                            q = cv.wait(q).unwrap();
                        }
                    };
                    let d = execute(&shared.0, &shared.1, req);
                    if done.send(d).is_err() {
                        return;
                    }
                }
            });
        }
        Self { queues }
    }

    pub fn submit(&self, req: Request) {
        let (lock, cv) = &*self.queues;
        let mut q = lock.lock().unwrap();
        if req.low_priority {
            q.low.push_back(req);
        } else {
            q.high.push_back(req);
        }
        cv.notify_one();
    }
}

fn log_entry(out: &RunOutput, c: &Classified) -> LogEntry {
    LogEntry {
        at: SystemTime::now(),
        command: format!("sf {}", out.argv.join(" ")),
        exit: out.exit_code,
        duration: out.duration,
        state: c.state.short(),
        rule: c.rule,
        stderr: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
    }
}

fn execute(runner: &SfRunner, manifest: &Manifest, req: Request) -> Done {
    let Request {
        id, job, cancel, ..
    } = req;
    if cancel.load(Ordering::Relaxed) {
        return Done {
            id,
            job,
            outcome: Outcome::Cancelled,
            log: vec![],
        };
    }
    let Some(cmd) = job.command() else {
        let report = doctor::run(runner, manifest);
        let log = report
            .runs
            .iter()
            .map(|o| log_entry(o, &classify(o)))
            .collect();
        return Done {
            id,
            job,
            outcome: Outcome::Doctor(Box::new(report)),
            log,
        };
    };
    let out = match runner.run(&cmd, manifest, None, Some(&cancel)) {
        Ok(o) => o,
        Err(e) => {
            return Done {
                id,
                job,
                outcome: Outcome::Failed(Failure::simple(e.to_string())),
                log: vec![],
            };
        }
    };
    let c = classify(&out);
    let log = vec![log_entry(&out, &c)];
    let outcome = if out.cancelled {
        Outcome::Cancelled
    } else {
        match envelope::parse(&out.stdout) {
            Ok(SfOutput::Success { result, warnings }) => Outcome::Ok {
                value: result,
                warnings,
            },
            Ok(SfOutput::Bare(v)) if out.exit_code == Some(0) => Outcome::Ok {
                value: v,
                warnings: vec![],
            },
            _ => Outcome::Failed(Failure::from_run(&c, &out)),
        }
    };
    Done {
        id,
        job,
        outcome,
        log,
    }
}
