//! TUI state and input handling. Rendering is in `ui.rs`.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use gp_atlas_core::classify::CapState;
use gp_atlas_core::command::{PkgVersionListArgs, ReadOnlyCommand};
use gp_atlas_core::doctor::DoctorReport;
use gp_atlas_core::graph::{self as dag, Graph};
use gp_atlas_core::ids::{Id0Ho, Id04t, PackageRef};
use gp_atlas_core::manifest::Manifest;
use gp_atlas_core::orgs::{self, Org};
use gp_atlas_core::probes::{Capability, org_ref};
use gp_atlas_core::versions;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::{Position, Rect};
use serde_json::Value;

use crate::clip;
use crate::worker::{Done, Failure, Job, LogEntry, Outcome, Pool, Request};

/// Production install link prefix (F18). Same as the CLI's `InstallUrl`.
const INSTALL_URL_BASE: &str = "https://login.salesforce.com/packaging/installPackage.apexp?p0=";

/// Max History entries kept (§8).
const LOG_CAP: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Doctor,
    Orgs,
    Access,
    Packages,
    Versions,
    Installed,
    Pkg1,
    Log,
}

impl Tab {
    /// Visible tabs. `Versions` is not a tab: it is the right pane of `Packages`.
    pub const ALL: [Tab; 7] = [
        Tab::Doctor,
        Tab::Orgs,
        Tab::Access,
        Tab::Packages,
        Tab::Installed,
        Tab::Pkg1,
        Tab::Log,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Doctor => "Doctor",
            Tab::Orgs => "Orgs",
            Tab::Access => "Access",
            Tab::Packages => "2GP Packages & Versions",
            Tab::Versions => "2GP Versions",
            Tab::Installed => "Installed",
            Tab::Pkg1 => "1GP Versions",
            Tab::Log => "History",
        }
    }
}

pub enum Load<T> {
    Idle,
    Loading {
        id: u64,
        since: Instant,
        cancel: Arc<AtomicBool>,
    },
    Ready(T),
    Failed(Failure),
}

impl<T> Load<T> {
    pub fn is_loading(&self) -> bool {
        matches!(self, Load::Loading { .. })
    }
    fn loading_id(&self) -> Option<u64> {
        match self {
            Load::Loading { id, .. } => Some(*id),
            _ => None,
        }
    }
    fn cancel(&self) {
        if let Load::Loading { cancel, .. } = self {
            cancel.store(true, Ordering::Relaxed);
        }
    }
}

/// Selection and scroll position of a table (only visible rows are built).
#[derive(Default, Clone, Copy)]
pub struct Scroll {
    pub selected: usize,
    pub offset: usize,
}

impl Scroll {
    pub fn move_by(&mut self, delta: isize, len: usize) {
        if len == 0 {
            *self = Self::default();
            return;
        }
        let s = self.selected as isize + delta;
        self.selected = s.clamp(0, len as isize - 1) as usize;
    }

    /// Keeps the selection inside `[offset, offset + height)`.
    pub fn fit(&mut self, height: usize, len: usize) {
        if len == 0 {
            *self = Self::default();
            return;
        }
        self.selected = self.selected.min(len - 1);
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if height > 0 && self.selected >= self.offset + height {
            self.offset = self.selected + 1 - height;
        }
        self.offset = self.offset.min(len.saturating_sub(height.max(1)));
    }
}

pub struct ListData {
    pub rows: Vec<Value>,
    pub warnings: Vec<String>,
}

#[derive(Default)]
pub struct Panel {
    pub load: Option<Load<ListData>>,
    pub scroll: Scroll,
    /// The org the loaded data belongs to.
    pub for_org: Option<String>,
}

impl Panel {
    pub fn load(&self) -> &Load<ListData> {
        static IDLE: Load<ListData> = Load::Idle;
        self.load.as_ref().unwrap_or(&IDLE)
    }
}

pub enum Gate {
    Checking,
    Ok { version: String, newer: bool },
    Blocked(String),
}

#[derive(Clone)]
pub enum Cell {
    NotRun,
    NotApplicable(&'static str),
    Pending,
    Done {
        state: CapState,
        failure: Option<Failure>,
        command: String,
    },
}

#[derive(Default)]
pub struct VersionFilter {
    pub package: Option<(Id0Ho, String)>,
    pub released: bool,
    pub latest: bool,
    pub verbose: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphKind {
    Ancestry,
    Deps,
}

/// The Ancestry / Dependencies overlay on the 2GP tab.
pub struct GraphView {
    pub kind: GraphKind,
    pub title: String,
    pub loading: Option<(u64, Instant)>,
    pub graph: Option<Graph>,
    pub failure: Option<Failure>,
    /// 04t to highlight (the version the user started from).
    pub focus: Option<String>,
    pub notes: Vec<String>,
    /// Dependencies: false = install order, true = tree.
    pub tree_mode: bool,
    pub scroll: Scroll,
    /// The command behind the view, for "copy command" (copy-only).
    pub command: Option<ReadOnlyCommand>,
}

/// One displayed line of a graph view.
pub struct GraphRow {
    pub prefix: String,
    pub id: String,
    pub label: String,
    pub focus: bool,
    pub on_path: bool,
    pub highlighted: bool,
    pub children: usize,
}

impl GraphView {
    pub fn rows(&self) -> Vec<GraphRow> {
        let Some(g) = &self.graph else {
            return vec![];
        };
        let path: Vec<String> = self
            .focus
            .as_deref()
            .map(|f| g.path_to_root(f))
            .unwrap_or_default();
        let row = |prefix: String, id: &str| {
            let n = g.node(id);
            GraphRow {
                prefix,
                id: id.to_owned(),
                label: n.map(|n| n.label.clone()).unwrap_or_else(|| id.to_owned()),
                focus: self.focus.as_deref() == Some(id),
                on_path: path.iter().any(|p| p == id),
                highlighted: n.is_some_and(|n| n.highlighted),
                children: g.child_count(id),
            }
        };
        match (self.kind, self.tree_mode) {
            (GraphKind::Ancestry, _) => g
                .forest(false)
                .into_iter()
                .map(|l| row(l.prefix, &l.id))
                .collect(),
            (GraphKind::Deps, true) => g
                .forest(true)
                .into_iter()
                .map(|l| row(l.prefix, &l.id))
                .collect(),
            (GraphKind::Deps, false) => match g.install_order() {
                Ok(order) => order
                    .iter()
                    .enumerate()
                    .map(|(i, n)| row(format!("{:>2}. ", i + 1), &n.id))
                    .collect(),
                Err(_) => g
                    .forest(true)
                    .into_iter()
                    .map(|l| row(l.prefix, &l.id))
                    .collect(),
            },
        }
    }
}

pub struct Popup {
    pub title: String,
    pub lines: Vec<String>,
    pub scroll: u16,
    pub loading_id: Option<u64>,
}

/// Screen regions recorded while rendering, for mouse hit-testing.
#[derive(Default)]
pub struct Hits {
    pub tabs: Vec<(Rect, Tab)>,
    /// Rows areas of the visible tables (below their headers), with the pane
    /// they belong to (`Packages`/`Versions` on the 2GP tab, else the tab).
    pub rows: Vec<(Rect, Tab)>,
    /// Column x-ranges of the Access matrix capability columns.
    pub access_cols: Vec<(u16, u16)>,
    pub popup: Option<Rect>,
    /// Graph overlay and its rows area.
    pub graph: Option<Rect>,
    pub graph_rows: Option<Rect>,
}

pub struct App {
    pub manifest: Manifest,
    pool: Pool,
    next_id: u64,
    pub quit: bool,
    pub tab: Tab,
    pub gate: Gate,
    pub doctor: Load<Box<DoctorReport>>,
    pub drifted: Vec<String>,
    pub orgs: Load<Vec<Org>>,
    orgs_full_loaded: bool,
    pub orgs_scroll: Scroll,
    pub hub: Option<Org>,
    pub org: Option<Org>,
    pub cells: HashMap<(String, Capability), Cell>,
    pub probe_pending: usize,
    probe_cancel: Arc<AtomicBool>,
    pub try_anyway: bool,
    pub access_scroll: Scroll,
    pub access_col: usize,
    pub packages: Panel,
    pub versions: Panel,
    pub vfilter: VersionFilter,
    /// 2GP tab: true when the packages (left) pane has focus.
    pub pkg_focus: bool,
    /// 2GP tab: when the package selection last changed (debounced load).
    pkg_changed: Option<Instant>,
    pub installed: Panel,
    pub pkg1: Panel,
    pub log: Vec<LogEntry>,
    pub log_scroll: Scroll,
    pub popup: Option<Popup>,
    pub graph: Option<GraphView>,
    pub status: String,
    pub hits: Hits,
    pub allow_cli_version: Option<String>,
}

fn new_cancel() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

impl App {
    pub fn new(pool: Pool, manifest: Manifest, allow_cli_version: Option<String>) -> Self {
        Self {
            manifest,
            pool,
            next_id: 1,
            quit: false,
            tab: Tab::Doctor,
            gate: Gate::Checking,
            doctor: Load::Idle,
            drifted: Vec::new(),
            orgs: Load::Idle,
            orgs_full_loaded: false,
            orgs_scroll: Scroll::default(),
            hub: None,
            org: None,
            cells: HashMap::new(),
            probe_pending: 0,
            probe_cancel: new_cancel(),
            try_anyway: false,
            access_scroll: Scroll::default(),
            access_col: 0,
            packages: Panel::default(),
            versions: Panel::default(),
            vfilter: VersionFilter::default(),
            pkg_focus: true,
            pkg_changed: None,
            installed: Panel::default(),
            pkg1: Panel::default(),
            log: Vec::new(),
            log_scroll: Scroll::default(),
            popup: None,
            graph: None,
            status: "Checking the Salesforce CLI…".into(),
            hits: Hits::default(),
            allow_cli_version,
        }
    }

    pub fn start(&mut self) {
        let (id, cancel) = self.enqueue(Job::Doctor, false, new_cancel());
        self.doctor = Load::Loading {
            id,
            since: Instant::now(),
            cancel,
        };
    }

    fn enqueue(&mut self, job: Job, low: bool, cancel: Arc<AtomicBool>) -> (u64, Arc<AtomicBool>) {
        let id = self.next_id;
        self.next_id += 1;
        self.pool.submit(Request {
            id,
            job,
            cancel: cancel.clone(),
            low_priority: low,
        });
        (id, cancel)
    }

    /// Submits an sf job if the version gate and contract allow it.
    fn submit(
        &mut self,
        job: Job,
        low: bool,
        cancel: Arc<AtomicBool>,
    ) -> Result<(u64, Arc<AtomicBool>), Failure> {
        if !matches!(self.gate, Gate::Ok { .. }) {
            return Err(Failure::simple(
                "The Salesforce CLI check has not passed — see the Doctor tab.",
            ));
        }
        if let Some(cmd) = job.command() {
            let words = cmd.id().replace(':', " ");
            if self.drifted.contains(&words) {
                return Err(Failure {
                    state: CapState::ContractDrift(words.clone()),
                    rule: "D4",
                    name: "ContractDrift".into(),
                    message: format!(
                        "`sf {words}` changed in this CLI version and is disabled (see Doctor)."
                    ),
                    stderr: String::new(),
                });
            }
            if let Err(e) = cmd.argv(&self.manifest) {
                return Err(Failure::simple(e.to_string()));
            }
        }
        Ok(self.enqueue(job, low, cancel))
    }

    pub fn any_loading(&self) -> bool {
        self.doctor.is_loading()
            || self.orgs.is_loading()
            || self.probe_pending > 0
            || [&self.packages, &self.versions, &self.installed, &self.pkg1]
                .iter()
                .any(|p| p.load().is_loading())
            || self.popup.as_ref().is_some_and(|p| p.loading_id.is_some())
    }

    // ----- results ---------------------------------------------------------

    pub fn on_done(&mut self, done: Done) {
        for e in done.log {
            self.log.push(e);
        }
        if self.log.len() > LOG_CAP {
            let extra = self.log.len() - LOG_CAP;
            self.log.drain(..extra);
        }
        let id = done.id;
        match (done.job, done.outcome) {
            (Job::Doctor, Outcome::Doctor(report)) => self.on_doctor(report),
            (Job::Doctor, _) => {}
            (Job::Orgs { fast }, outcome) => self.on_orgs(id, fast, outcome),
            (Job::Probe { username, cap, .. }, outcome) => {
                self.probe_pending = self.probe_pending.saturating_sub(1);
                let cell = match outcome {
                    Outcome::Ok { .. } => Cell::Done {
                        state: CapState::Allowed,
                        failure: None,
                        command: self.last_command(),
                    },
                    Outcome::Failed(f) => Cell::Done {
                        state: f.state.clone(),
                        failure: Some(f),
                        command: self.last_command(),
                    },
                    Outcome::Cancelled | Outcome::Doctor(_) => Cell::NotRun,
                };
                self.cells.insert((username, cap), cell);
                if self.probe_pending == 0 {
                    self.status = "Access Matrix probes finished.".into();
                }
            }
            (Job::Packages { .. }, o) => finish(&mut self.packages, id, o),
            (Job::Versions { .. }, o) => finish(&mut self.versions, id, o),
            (Job::Installed { .. }, o) => finish(&mut self.installed, id, o),
            (Job::Pkg1 { .. }, o) => finish(&mut self.pkg1, id, o),
            (Job::Ancestry { .. } | Job::Deps { .. }, o) => self.on_graph(id, o),
            (Job::Report { version, .. }, o) => {
                if let Some(p) = &mut self.popup
                    && p.loading_id == Some(id)
                {
                    p.loading_id = None;
                    p.lines = match o {
                        Outcome::Ok { value, .. } => kv_lines(&value),
                        Outcome::Failed(f) => failure_lines(&f, ""),
                        _ => vec!["Cancelled.".into()],
                    };
                    p.title = format!("Version {version}");
                }
            }
        }
    }

    fn last_command(&self) -> String {
        self.log
            .last()
            .map(|e| e.command.clone())
            .unwrap_or_default()
    }

    fn on_doctor(&mut self, report: Box<DoctorReport>) {
        let found = report.found_version.clone().unwrap_or_default();
        self.drifted = report.drift.iter().map(|d| d.command.clone()).collect();
        self.gate = if !report.d1_runnable.is_pass() {
            Gate::Blocked("sf is not runnable — see Doctor.".into())
        } else if report.version_allowed(self.allow_cli_version.as_deref()) {
            Gate::Ok {
                newer: found != gp_atlas_core::BASELINE_CLI_VERSION_STRING,
                version: found.rsplit('/').next().unwrap_or(&found).to_owned(),
            }
        } else {
            Gate::Blocked(format!(
                "sf {found} is older than {} — all sf features are disabled.",
                gp_atlas_core::MIN_CLI_VERSION
            ))
        };
        let green = report.all_green();
        self.doctor = Load::Ready(report);
        match &self.gate {
            Gate::Ok { .. } => {
                self.status = if green {
                    "CLI OK. Loading orgs…".into()
                } else {
                    "CLI usable with warnings — see Doctor. Loading orgs…".into()
                };
                self.load_orgs(true);
                if green && self.tab == Tab::Doctor {
                    self.tab = Tab::Orgs;
                }
            }
            Gate::Blocked(m) => self.status = m.clone(),
            Gate::Checking => {}
        }
    }

    fn load_orgs(&mut self, fast: bool) {
        match self.submit(Job::Orgs { fast }, false, new_cancel()) {
            Ok((id, cancel)) => {
                if fast || !matches!(self.orgs, Load::Ready(_)) {
                    self.orgs = Load::Loading {
                        id,
                        since: Instant::now(),
                        cancel,
                    };
                }
            }
            Err(f) => self.orgs = Load::Failed(f),
        }
    }

    fn on_orgs(&mut self, id: u64, fast: bool, outcome: Outcome) {
        let current = self.orgs.loading_id();
        // A full (slow) refresh may arrive while Ready; accept it then too.
        if current.is_some() && current != Some(id) && fast {
            return;
        }
        match outcome {
            Outcome::Ok { value, .. } => {
                let list = orgs::from_org_list(&value);
                if self.hub.is_none() {
                    self.hub = list.iter().find(|o| o.is_default_dev_hub).cloned();
                }
                if self.org.is_none() {
                    self.org = list.iter().find(|o| o.is_default_org).cloned();
                }
                // Refresh the selected orgs' details (connection status).
                for sel in [&mut self.hub, &mut self.org] {
                    if let Some(s) = sel.as_ref()
                        && let Some(n) = list.iter().find(|o| o.username == s.username)
                    {
                        *sel = Some(n.clone());
                    }
                }
                let n = list.len();
                self.orgs = Load::Ready(list);
                if fast {
                    self.status =
                        format!("{n} orgs. Checking connection status in the background…");
                    self.load_orgs(false);
                } else {
                    self.orgs_full_loaded = true;
                    self.status = format!("{n} orgs (connection status updated).");
                }
            }
            Outcome::Failed(f) => {
                self.status = format!("Loading orgs failed: {}", f.state.short());
                if !matches!(self.orgs, Load::Ready(_)) || fast {
                    self.orgs = Load::Failed(f);
                } else {
                    self.status = format!("Connection-status refresh failed: {}", f.message);
                }
            }
            Outcome::Cancelled => {
                if self.orgs.is_loading() {
                    self.orgs = Load::Idle;
                }
            }
            Outcome::Doctor(_) => {}
        }
    }

    // ----- loading tabs ----------------------------------------------------

    fn panel_mut(&mut self, tab: Tab) -> Option<&mut Panel> {
        Some(match tab {
            Tab::Packages => &mut self.packages,
            Tab::Versions => &mut self.versions,
            Tab::Installed => &mut self.installed,
            Tab::Pkg1 => &mut self.pkg1,
            _ => return None,
        })
    }

    /// Target org for a tab's data (hub or org).
    pub fn tab_target(&self, tab: Tab) -> Option<&Org> {
        match tab {
            Tab::Packages | Tab::Versions => self.hub.as_ref(),
            Tab::Installed | Tab::Pkg1 => self.org.as_ref(),
            _ => None,
        }
    }

    fn job_for(&self, tab: Tab) -> Result<Job, String> {
        let target = self.tab_target(tab).ok_or_else(|| match tab {
            Tab::Packages | Tab::Versions => {
                "No Dev Hub selected — go to Orgs and press h on a Dev Hub.".to_owned()
            }
            _ => "No org selected — go to Orgs and press o on an org.".to_owned(),
        })?;
        let r = org_ref(target).map_err(|e| e.to_string())?;
        Ok(match tab {
            Tab::Packages => Job::Packages { hub: r },
            Tab::Versions => {
                let mut a = PkgVersionListArgs::new(r);
                if let Some((id, _)) = &self.vfilter.package {
                    a.packages = vec![PackageRef::Id(id.clone())];
                }
                a.released = self.vfilter.released || self.vfilter.latest;
                a.verbose = self.vfilter.verbose;
                Job::Versions { args: Box::new(a) }
            }
            Tab::Installed => Job::Installed { org: r },
            Tab::Pkg1 => Job::Pkg1 { org: r },
            _ => return Err("nothing to load".into()),
        })
    }

    /// Loads the current tab if needed (or always, with `force`).
    pub fn load_tab(&mut self, force: bool) {
        let tab = self.tab;
        match tab {
            Tab::Packages => {
                self.load_panel(Tab::Packages, force);
                self.load_panel(Tab::Versions, force);
                return;
            }
            Tab::Doctor => {
                if force {
                    self.gate = Gate::Checking;
                    self.start();
                }
                return;
            }
            Tab::Orgs | Tab::Access => {
                if force {
                    self.load_orgs(true);
                }
                return;
            }
            Tab::Log => return,
            _ => {}
        }
        self.load_panel(tab, force);
    }

    /// Loads one data panel if needed (or always, with `force`).
    fn load_panel(&mut self, tab: Tab, force: bool) {
        if !matches!(self.gate, Gate::Ok { .. }) {
            return;
        }
        let target = self.tab_target(tab).map(|o| o.username.clone());
        if target.is_none() {
            // Nothing selected yet: the tab shows how to pick a Dev Hub / org.
            let panel = self.panel_mut(tab).expect("data tab");
            panel.load().cancel();
            panel.load = None;
            panel.for_org = None;
            return;
        }
        let panel = self.panel_mut(tab).expect("data tab");
        let stale = panel.for_org != target;
        let idle = matches!(panel.load(), Load::Idle);
        if !(force || stale || idle) || panel.load().is_loading() && !force {
            return;
        }
        panel.load().cancel();
        let result = self
            .job_for(tab)
            .map_err(Failure::simple)
            .and_then(|job| self.submit(job, false, new_cancel()));
        let panel = self.panel_mut(tab).expect("data tab");
        panel.for_org = target;
        panel.scroll = Scroll::default();
        panel.load = Some(match result {
            Ok((id, cancel)) => Load::Loading {
                id,
                since: Instant::now(),
                cancel,
            },
            Err(f) => Load::Failed(f),
        });
    }

    // ----- Access Matrix ---------------------------------------------------

    fn probe_orgs(&mut self, only: Option<String>) {
        let Load::Ready(list) = &self.orgs else {
            self.status = "Orgs are not loaded yet.".into();
            return;
        };
        let list: Vec<Org> = list
            .iter()
            .filter(|o| only.as_ref().is_none_or(|u| &o.username == u))
            .cloned()
            .collect();
        self.probe_cancel = new_cancel();
        let low = only.is_none();
        let mut queued = 0;
        for o in &list {
            for cap in Capability::ALL {
                let key = (o.username.clone(), cap);
                let applies = match cap.applies_to(o) {
                    Ok(()) => Ok(()),
                    Err(r) if self.try_anyway && r == "not flagged as Dev Hub" => Ok(()),
                    Err(r) => Err(r),
                };
                if let Err(r) = applies {
                    self.cells.insert(key, Cell::NotApplicable(r));
                    continue;
                }
                let Ok(target) = org_ref(o) else {
                    self.cells
                        .insert(key, Cell::NotApplicable("unsupported alias"));
                    continue;
                };
                let job = Job::Probe {
                    username: o.username.clone(),
                    cap,
                    target,
                };
                match self.submit(job, low, self.probe_cancel.clone()) {
                    Ok(_) => {
                        self.cells.insert(key, Cell::Pending);
                        queued += 1;
                    }
                    Err(f) => {
                        self.cells.insert(
                            key,
                            Cell::Done {
                                state: f.state.clone(),
                                failure: Some(f),
                                command: String::new(),
                            },
                        );
                    }
                }
            }
        }
        self.probe_pending += queued;
        self.status =
            format!("Probing: {queued} checks queued (max 3 sf processes at once). Esc cancels.");
    }

    // ----- input -----------------------------------------------------------

    /// The pane keys and selection apply to: on the 2GP tab, packages (left)
    /// or versions (right); otherwise the tab itself.
    pub fn pane(&self) -> Tab {
        if self.tab == Tab::Packages && !self.pkg_focus {
            Tab::Versions
        } else {
            self.tab
        }
    }

    /// The package selected in the 2GP left pane (row 0 = all packages).
    pub fn selected_package(&self) -> Option<(Id0Ho, String)> {
        let sel = self.packages.scroll.selected.checked_sub(1)?;
        let Load::Ready(d) = self.packages.load() else {
            return None;
        };
        let row = d.rows.get(sel)?;
        let id = str_of(row, "Id").and_then(|i| Id0Ho::new(&i).ok())?;
        Some((id, str_of(row, "Name").unwrap_or_default()))
    }

    /// Loads versions for the selected package now.
    fn apply_package_selection(&mut self) {
        self.pkg_changed = None;
        let want = self.selected_package();
        let same = want.as_ref().map(|p| &p.0) == self.vfilter.package.as_ref().map(|p| &p.0);
        if same && !matches!(self.versions.load(), Load::Idle | Load::Failed(_)) {
            return;
        }
        self.vfilter.package = want;
        self.load_panel(Tab::Versions, true);
    }

    /// Called every frame: applies a debounced package selection.
    pub fn tick(&mut self) {
        if let Some(t) = self.pkg_changed
            && t.elapsed().as_millis() >= 350
        {
            self.apply_package_selection();
        }
    }

    fn table_len(&self) -> usize {
        match self.pane() {
            Tab::Orgs | Tab::Access => match &self.orgs {
                Load::Ready(l) => l.len(),
                _ => 0,
            },
            Tab::Versions => self.version_rows().len(),
            Tab::Packages => match self.packages.load() {
                Load::Ready(d) => d.rows.len() + 1, // + "All packages"
                _ => 0,
            },
            Tab::Installed | Tab::Pkg1 => match self.panel_ref(self.pane()).map(Panel::load) {
                Some(Load::Ready(d)) => d.rows.len(),
                _ => 0,
            },
            Tab::Log => self.log.len(),
            Tab::Doctor => 0,
        }
    }

    fn panel_ref(&self, tab: Tab) -> Option<&Panel> {
        Some(match tab {
            Tab::Packages => &self.packages,
            Tab::Versions => &self.versions,
            Tab::Installed => &self.installed,
            Tab::Pkg1 => &self.pkg1,
            _ => return None,
        })
    }

    pub fn scroll_mut(&mut self) -> Option<&mut Scroll> {
        Some(match self.pane() {
            Tab::Orgs => &mut self.orgs_scroll,
            Tab::Access => &mut self.access_scroll,
            Tab::Log => &mut self.log_scroll,
            Tab::Doctor => return None,
            t => &mut self.panel_mut(t)?.scroll,
        })
    }

    /// Version rows as displayed (client-side "latest released" applied).
    pub fn version_rows(&self) -> Vec<&Value> {
        match self.versions.load() {
            Load::Ready(d) if self.vfilter.latest => versions::latest_released(&d.rows),
            Load::Ready(d) => d.rows.iter().collect(),
            _ => vec![],
        }
    }

    fn selected_row(&self) -> Option<Value> {
        let pane = self.pane();
        let sel = self.panel_ref(pane)?.scroll.selected;
        match pane {
            Tab::Versions => self.version_rows().get(sel).map(|v| (*v).clone()),
            Tab::Packages => match self.packages.load() {
                Load::Ready(d) => d.rows.get(sel.checked_sub(1)?).cloned(),
                _ => None,
            },
            _ => match self.panel_ref(pane)?.load() {
                Load::Ready(d) => d.rows.get(sel).cloned(),
                _ => None,
            },
        }
    }

    fn selected_org(&self, scroll: Scroll) -> Option<Org> {
        match &self.orgs {
            Load::Ready(l) => l.get(scroll.selected).cloned(),
            _ => None,
        }
    }

    fn switch_tab(&mut self, tab: Tab) {
        self.tab = tab;
        self.load_tab(false);
    }

    fn move_sel(&mut self, delta: isize) {
        let len = self.table_len();
        let before = self.scroll_mut().map(|s| s.selected);
        if let Some(s) = self.scroll_mut() {
            s.move_by(delta, len);
        }
        if self.pane() == Tab::Packages && self.scroll_mut().map(|s| s.selected) != before {
            self.pkg_changed = Some(Instant::now());
        }
    }

    pub fn on_key(&mut self, k: KeyEvent) {
        if k.kind != KeyEventKind::Press {
            return;
        }
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && k.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if let Some(p) = &mut self.popup {
            match k.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => self.popup = None,
                KeyCode::Down | KeyCode::Char('j') => p.scroll = p.scroll.saturating_add(1),
                KeyCode::Up | KeyCode::Char('k') => p.scroll = p.scroll.saturating_sub(1),
                KeyCode::PageDown => p.scroll = p.scroll.saturating_add(10),
                KeyCode::PageUp => p.scroll = p.scroll.saturating_sub(10),
                KeyCode::Char('c') => {
                    let text = p.lines.join("\n");
                    self.copy(&text, "details");
                }
                _ => {}
            }
            return;
        }
        if self.graph.is_some() {
            self.graph_key(k.code);
            return;
        }
        match k.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.show_help(),
            KeyCode::Esc => self.cancel_current(),
            KeyCode::Tab | KeyCode::Right
                if k.code == KeyCode::Tab || !matches!(self.tab, Tab::Access | Tab::Packages) =>
            {
                let i = Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0);
                self.switch_tab(Tab::ALL[(i + 1) % Tab::ALL.len()]);
            }
            KeyCode::BackTab | KeyCode::Left
                if k.code == KeyCode::BackTab
                    || !matches!(self.tab, Tab::Access | Tab::Packages) =>
            {
                let i = Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0);
                self.switch_tab(Tab::ALL[(i + Tab::ALL.len() - 1) % Tab::ALL.len()]);
            }
            KeyCode::Left if self.tab == Tab::Packages => self.pkg_focus = true,
            KeyCode::Right if self.tab == Tab::Packages => self.pkg_focus = false,
            KeyCode::Left => self.access_col = self.access_col.saturating_sub(1),
            KeyCode::Right => {
                self.access_col = (self.access_col + 1).min(Capability::ALL.len() - 1)
            }
            KeyCode::Char(c @ '1'..='7') => {
                let i = c as usize - '1' as usize;
                self.switch_tab(Tab::ALL[i]);
            }
            KeyCode::Down | KeyCode::Char('j') => self.move_sel(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_sel(-1),
            KeyCode::PageDown => self.move_sel(20),
            KeyCode::PageUp => self.move_sel(-20),
            KeyCode::Home | KeyCode::Char('g') => self.move_sel(isize::MIN / 2),
            KeyCode::End | KeyCode::Char('G') => self.move_sel(isize::MAX / 2),
            KeyCode::Char('r') => {
                self.load_tab(true);
                self.status = format!("Refreshing {}…", self.tab.title());
            }
            KeyCode::Enter => self.activate(),
            KeyCode::Char(c) => self.tab_key(c),
            _ => {}
        }
    }

    fn tab_key(&mut self, c: char) {
        if self.tab == Tab::Packages && matches!(c, 'a' | 'A' | 'd') {
            self.open_graph_for_selection(c);
            return;
        }
        let pane = if self.tab == Tab::Packages && "RLVxiy".contains(c) {
            Tab::Versions
        } else {
            self.pane()
        };
        match (pane, c) {
            (Tab::Orgs, 'h') => self.pick_org(true),
            (Tab::Orgs, 'o') => self.pick_org(false),
            (Tab::Access, 'p') => {
                let u = self.selected_org(self.access_scroll).map(|o| o.username);
                if u.is_some() {
                    self.probe_orgs(u);
                }
            }
            (Tab::Access, 'a') => self.probe_orgs(None),
            (Tab::Access, 't') => {
                self.try_anyway = !self.try_anyway;
                self.status = format!(
                    "Try 2GP on non-Dev-Hub orgs: {}",
                    if self.try_anyway { "on" } else { "off" }
                );
            }
            (Tab::Versions, 'R') => self.toggle_filter(|f| f.released = !f.released),
            (Tab::Versions, 'L') => {
                self.vfilter.latest = !self.vfilter.latest;
                if self.vfilter.latest && !self.vfilter.released {
                    self.load_panel(Tab::Versions, true);
                } else {
                    self.versions.scroll = Scroll::default();
                }
            }
            (Tab::Versions, 'V') => self.toggle_filter(|f| f.verbose = !f.verbose),
            (Tab::Versions, 'x') => {
                self.packages.scroll = Scroll::default();
                self.apply_package_selection();
            }
            (Tab::Versions, 'i') => {
                if let Some(u) = self.selected_row().and_then(|r| str_of(&r, "InstallUrl")) {
                    self.copy(&u, "install link");
                }
            }
            (Tab::Versions, 'y') => self.copy_report_command(),
            (Tab::Packages, 'c') => self.copy_field("Id", "0Ho id"),
            (Tab::Versions, 'c') | (Tab::Installed, 'c') => {
                self.copy_field("SubscriberPackageVersionId", "04t id")
            }
            (Tab::Pkg1, 'c') => self.copy_field("MetadataPackageVersionId", "04t id"),
            (Tab::Log, 'c') => {
                if let Some(e) = self.log.iter().rev().nth(self.log_scroll.selected) {
                    let cmd = e.command.clone();
                    self.copy(&cmd, "command");
                }
            }
            _ => {}
        }
    }

    fn toggle_filter(&mut self, f: impl FnOnce(&mut VersionFilter)) {
        f(&mut self.vfilter);
        self.load_panel(Tab::Versions, true);
    }

    fn pick_org(&mut self, as_hub: bool) {
        let Some(o) = self.selected_org(self.orgs_scroll) else {
            return;
        };
        if as_hub {
            if !o.is_dev_hub {
                self.status = format!(
                    "{} is not flagged as a Dev Hub; using it anyway.",
                    o.label()
                );
            } else {
                self.status = format!("Dev Hub: {}", o.label());
            }
            self.hub = Some(o);
            self.vfilter.package = None;
        } else {
            self.status = format!("Org: {}", o.label());
            self.org = Some(o);
        }
    }

    fn activate(&mut self) {
        match self.pane() {
            Tab::Orgs => {
                let is_hub = self
                    .selected_org(self.orgs_scroll)
                    .is_some_and(|o| o.is_dev_hub);
                self.pick_org(is_hub);
            }
            Tab::Access => self.cell_details(),
            Tab::Packages => {
                // Show this package's versions now and move focus to them.
                self.apply_package_selection();
                self.pkg_focus = false;
            }
            Tab::Versions => self.version_details(),
            Tab::Installed | Tab::Pkg1 => {
                if let Some(row) = self.selected_row() {
                    self.popup = Some(Popup {
                        title: "Details".into(),
                        lines: kv_lines(&row),
                        scroll: 0,
                        loading_id: None,
                    });
                }
            }
            Tab::Log => {
                if let Some(e) = self.log.iter().rev().nth(self.log_scroll.selected) {
                    let mut lines = vec![
                        e.command.clone(),
                        String::new(),
                        format!(
                            "exit code: {}",
                            e.exit.map_or("killed".into(), |c| c.to_string())
                        ),
                        format!("duration:  {:.1}s", e.duration.as_secs_f64()),
                        format!("result:    {}", e.state),
                        format!("rule:      {}", e.rule),
                    ];
                    if !e.stderr.is_empty() {
                        lines.push(String::new());
                        lines.push("stderr:".into());
                        lines.extend(e.stderr.lines().map(str::to_owned));
                    }
                    self.popup = Some(Popup {
                        title: "sf call".into(),
                        lines,
                        scroll: 0,
                        loading_id: None,
                    });
                }
            }
            Tab::Doctor => {}
        }
    }

    fn cell_details(&mut self) {
        let Some(o) = self.selected_org(self.access_scroll) else {
            return;
        };
        let cap = Capability::ALL[self.access_col];
        let cell = self
            .cells
            .get(&(o.username.clone(), cap))
            .cloned()
            .unwrap_or(Cell::NotRun);
        let mut lines = vec![
            format!("Org:        {}", o.label()),
            format!("Capability: {}", cap.name()),
        ];
        match cell {
            Cell::NotRun => {
                lines.push("Not tested yet. Press p (this org) or a (all orgs).".into())
            }
            Cell::Pending => lines.push("Running…".into()),
            Cell::NotApplicable(r) => {
                lines.push(format!("Not applicable: {r}."));
                if r == "not flagged as Dev Hub" {
                    lines.push("Press t to enable \"try anyway\", then p.".into());
                }
            }
            Cell::Done {
                state,
                failure,
                command,
            } => {
                lines.push(format!("State:      {}", state.short()));
                if !command.is_empty() {
                    lines.push(format!("Command:    {command}"));
                }
                if let Some(f) = failure {
                    lines.push(String::new());
                    lines.extend(failure_lines(&f, o.target()));
                }
            }
        }
        self.popup = Some(Popup {
            title: "Access check".into(),
            lines,
            scroll: 0,
            loading_id: None,
        });
    }

    fn version_details(&mut self) {
        let Some(row) = self.selected_row() else {
            return;
        };
        let Some(v) = str_of(&row, "SubscriberPackageVersionId").and_then(|s| Id04t::new(&s).ok())
        else {
            return;
        };
        self.open_version_details(v, kv_lines(&row));
    }

    /// Popup with `package version report --verbose` for a version.
    fn open_version_details(&mut self, v: Id04t, mut lines: Vec<String>) {
        let Some(hub) = self.hub.as_ref().and_then(|h| org_ref(h).ok()) else {
            self.status = "No Dev Hub selected.".into();
            return;
        };
        lines.insert(0, "Loading `package version report --verbose`…".into());
        lines.insert(1, String::new());
        let job = Job::Report {
            hub,
            version: v.clone(),
        };
        let (lines, loading_id) = match self.submit(job, false, new_cancel()) {
            Ok((id, _)) => (lines, Some(id)),
            Err(f) => (failure_lines(&f, ""), None),
        };
        self.popup = Some(Popup {
            title: format!("Version {v}"),
            lines,
            scroll: 0,
            loading_id,
        });
    }

    // ----- Ancestry & Dependencies (docs/ideas/ANCESTRY_AND_DEPENDENCIES.md) --

    fn selected_version_row(&self) -> Option<Value> {
        self.version_rows()
            .get(self.versions.scroll.selected)
            .map(|v| (*v).clone())
    }

    /// `ContainerOptions` (Managed / Unlocked) of a package from the loaded list.
    fn package_type(&self, id: &str) -> Option<String> {
        let Load::Ready(d) = self.packages.load() else {
            return None;
        };
        let id15 = &id[..id.len().min(15)];
        d.rows
            .iter()
            .find(|r| str_of(r, "Id").is_some_and(|x| x.starts_with(id15)))
            .and_then(|r| str_of(r, "ContainerOptions"))
    }

    fn package_name(&self, id: &str) -> String {
        let Load::Ready(d) = self.packages.load() else {
            return id.to_owned();
        };
        d.rows
            .iter()
            .find(|r| str_of(r, "Id").as_deref() == Some(id))
            .and_then(|r| str_of(r, "Name"))
            .unwrap_or_else(|| id.to_owned())
    }

    /// `a`: ancestry (from a version: highlighted path; from the package pane:
    /// whole tree), `A`: whole package, `d`: dependencies of the selected version.
    fn open_graph_for_selection(&mut self, c: char) {
        if c == 'd' {
            let Some(row) = self.selected_version_row() else {
                self.status = "Select a version (right pane) first.".into();
                return;
            };
            let Some(v) =
                str_of(&row, "SubscriberPackageVersionId").and_then(|s| Id04t::new(&s).ok())
            else {
                return;
            };
            if let Some(t) = str_of(&row, "Package2Id").and_then(|p| self.package_type(&p))
                && t != "Managed"
                && t != "Unlocked"
            {
                self.status =
                    format!("Dependencies are for unlocked or 2GP managed packages, not {t}.");
                return;
            }
            let label = format!(
                "{}@{}",
                str_of(&row, "Package2Name").unwrap_or_default(),
                str_of(&row, "Version").unwrap_or_default()
            );
            self.open_deps(v, label);
            return;
        }
        let version = if self.pkg_focus {
            None
        } else {
            self.selected_version_row()
        };
        let (pkg, focus, mut notes) = match (&version, c) {
            (Some(row), 'a') => {
                let mut notes = Vec::new();
                if !versions::is_released(row) {
                    notes.push(format!(
                        "This version is not released: the CLI's ancestry tree has released versions \
                         only. Its ancestor per the version list: {}.",
                        str_of(row, "AncestorVersion").unwrap_or_else(|| "none".into())
                    ));
                }
                (
                    str_of(row, "Package2Id"),
                    str_of(row, "SubscriberPackageVersionId"),
                    notes,
                )
            }
            (Some(row), _) => (str_of(row, "Package2Id"), None, vec![]),
            (None, _) => (
                self.selected_package().map(|p| p.0.as_str().to_owned()),
                None,
                vec![],
            ),
        };
        let Some(pkg) = pkg.and_then(|p| Id0Ho::new(&p).ok()) else {
            self.status = "Select a package (left) or a version (right) first.".into();
            return;
        };
        if let Some(t) = self.package_type(pkg.as_str())
            && t != "Managed"
        {
            self.status =
                format!("Ancestry exists only for 2GP managed packages; this one is {t}.");
            return;
        }
        notes.insert(
            0,
            "Released versions only (CLI rule). From `displayancestry --dot-code`, which includes \
             every root."
                .into(),
        );
        let Some(hub) = self.hub.as_ref().and_then(|h| org_ref(h).ok()) else {
            self.status = "No Dev Hub selected.".into();
            return;
        };
        let name = self.package_name(pkg.as_str());
        let job = Job::Ancestry { hub, package: pkg };
        self.start_graph(
            GraphKind::Ancestry,
            format!("Ancestry — {name}"),
            job,
            focus,
            notes,
        );
    }

    fn open_deps(&mut self, v: Id04t, label: String) {
        let Some(hub) = self.hub.as_ref().and_then(|h| org_ref(h).ok()) else {
            self.status = "No Dev Hub selected.".into();
            return;
        };
        let mut notes = vec![
            "Install from top to bottom (root-last order).".to_owned(),
            "Needs a version built with \"calculateTransitiveDependencies\": true.".to_owned(),
        ];
        match self.org.as_ref().map(|o| o.target().to_owned()) {
            Some(o) => {
                notes.push(format!("\"In org\" checks {o} (its installed packages)."));
                self.load_panel(Tab::Installed, false);
            }
            None => {
                notes.push("Select an org (Orgs tab, o) to compare with what is installed.".into())
            }
        }
        let job = Job::Deps {
            hub,
            version: v.clone(),
        };
        self.start_graph(
            GraphKind::Deps,
            format!("Dependencies — {label}"),
            job,
            Some(v.as_str().to_owned()),
            notes,
        );
    }

    fn start_graph(
        &mut self,
        kind: GraphKind,
        title: String,
        job: Job,
        focus: Option<String>,
        notes: Vec<String>,
    ) {
        let command = job.command();
        let (loading, failure) = match self.submit(job, false, new_cancel()) {
            Ok((id, _)) => (Some((id, Instant::now())), None),
            Err(f) => (None, Some(f)),
        };
        self.graph = Some(GraphView {
            kind,
            title,
            loading,
            graph: None,
            failure,
            focus,
            notes,
            tree_mode: false,
            scroll: Scroll::default(),
            command,
        });
    }

    fn on_graph(&mut self, id: u64, outcome: Outcome) {
        let Some(gv) = &mut self.graph else { return };
        if gv.loading.map(|l| l.0) != Some(id) {
            return; // closed or replaced meanwhile
        }
        gv.loading = None;
        match outcome {
            Outcome::Ok {
                value: Value::String(dot),
                ..
            } => match dag::parse_dot(&dot) {
                Ok(g) => {
                    if gv.kind == GraphKind::Ancestry
                        && gv.focus.as_ref().is_some_and(|f| g.node(f).is_none())
                    {
                        gv.focus = None;
                    }
                    if gv.kind == GraphKind::Deps && g.nodes.len() <= 1 {
                        gv.notes
                            .push("No dependencies: this version installs on its own.".into());
                    }
                    gv.graph = Some(g);
                    if let Some(f) = gv.focus.clone()
                        && let Some(i) = gv.rows().iter().position(|r| r.id == f)
                    {
                        gv.scroll.selected = i;
                    }
                }
                Err(e) => gv.failure = Some(Failure::simple(e.to_string())),
            },
            Outcome::Ok { value, .. } => {
                gv.failure = Some(Failure::simple(format!(
                    "Unexpected output (not DOT): {value}"
                )));
            }
            Outcome::Failed(f) => {
                if f.message.contains("calculateTransitiveDependencies")
                    || f.message.contains("CalcTransitiveDependencies")
                {
                    gv.notes.push(
                        "This version was built without transitive dependencies. To see them, add \
                         \"calculateTransitiveDependencies\": true to its package directory in \
                         sfdx-project.json and create a new version (GP Atlas never edits the file)."
                            .into(),
                    );
                }
                gv.failure = Some(f);
            }
            Outcome::Cancelled | Outcome::Doctor(_) => {
                gv.failure = Some(Failure::simple("Cancelled."));
            }
        }
    }

    fn graph_key(&mut self, code: KeyCode) {
        let Some(gv) = &mut self.graph else { return };
        let len = gv.rows().len();
        match code {
            KeyCode::Esc | KeyCode::Char('q') => self.graph = None,
            KeyCode::Down | KeyCode::Char('j') => gv.scroll.move_by(1, len),
            KeyCode::Up | KeyCode::Char('k') => gv.scroll.move_by(-1, len),
            KeyCode::PageDown => gv.scroll.move_by(20, len),
            KeyCode::PageUp => gv.scroll.move_by(-20, len),
            KeyCode::Home | KeyCode::Char('g') => gv.scroll.move_by(isize::MIN / 2, len),
            KeyCode::End | KeyCode::Char('G') => gv.scroll.move_by(isize::MAX / 2, len),
            KeyCode::Char('t') if gv.kind == GraphKind::Deps => {
                gv.tree_mode = !gv.tree_mode;
                gv.scroll = Scroll::default();
            }
            KeyCode::Char('y') => {
                let cmd = gv.command.clone();
                if let Some(text) = cmd.and_then(|c| clip::command_for_copy(&c, &self.manifest)) {
                    self.copy(&text, "command");
                }
            }
            KeyCode::Char('c') => {
                if let Some(id) = self.graph_selected_id() {
                    self.copy(&id, "04t id");
                }
            }
            KeyCode::Char('i') => {
                if let Some(id) = self.graph_selected_id() {
                    self.copy(&format!("{INSTALL_URL_BASE}{id}"), "install link");
                }
            }
            KeyCode::Enter => self.graph_activate(),
            _ => {}
        }
    }

    fn graph_selected_id(&self) -> Option<String> {
        let gv = self.graph.as_ref()?;
        let rows = gv.rows();
        let id = rows.get(gv.scroll.selected)?.id.clone();
        id.starts_with("04t").then_some(id)
    }

    fn graph_activate(&mut self) {
        let Some(v) = self.graph_selected_id().and_then(|s| Id04t::new(&s).ok()) else {
            return;
        };
        self.open_version_details(v, vec![]);
    }

    /// Install state of a dependency in the selected org, as short text.
    pub fn install_text(&self, id: &str) -> (String, &'static str) {
        let (Some(org), Some(g)) = (
            &self.org,
            self.graph.as_ref().and_then(|g| g.graph.as_ref()),
        ) else {
            return ("—".into(), "dim");
        };
        let Some(node) = g.node(id) else {
            return (String::new(), "dim");
        };
        if node.id == "VERSION_BEING_BUILT" {
            return ("being built".into(), "dim");
        }
        if self.installed.for_org.as_deref() != Some(org.username.as_str()) {
            return ("…".into(), "dim");
        }
        match self.installed.load() {
            Load::Ready(d) => match dag::install_state(node, &d.rows) {
                dag::InstallState::Same => ("✅ installed".into(), "ok"),
                dag::InstallState::Newer(v) => (format!("✅ newer {v}"), "ok"),
                dag::InstallState::Older(v) => (format!("⚠ older {v}"), "warn"),
                dag::InstallState::Missing => ("⛔ missing".into(), "bad"),
                dag::InstallState::Unknown => ("?".into(), "dim"),
            },
            Load::Loading { .. } => ("…".into(), "dim"),
            Load::Failed(_) => ("? (installed list failed)".into(), "warn"),
            Load::Idle => ("—".into(), "dim"),
        }
    }

    fn copy_field(&mut self, key: &str, what: &str) {
        if let Some(v) = self.selected_row().and_then(|r| str_of(&r, key)) {
            self.copy(&v, what);
        }
    }

    fn copy_report_command(&mut self) {
        let Some(v) = self
            .selected_row()
            .and_then(|r| str_of(&r, "SubscriberPackageVersionId"))
            .and_then(|s| Id04t::new(&s).ok())
        else {
            return;
        };
        let Some(hub) = self.hub.as_ref().and_then(|h| org_ref(h).ok()) else {
            return;
        };
        let cmd = ReadOnlyCommand::PkgVersionReport {
            hub,
            package: PackageRef::Id(v),
            verbose: true,
        };
        if let Some(text) = clip::command_for_copy(&cmd, &self.manifest) {
            self.copy(&text, "report command");
        }
    }

    fn copy(&mut self, text: &str, what: &str) {
        self.status = match clip::copy(text) {
            Ok(()) => format!("Copied {what}: {text}"),
            Err(e) => format!("Clipboard unavailable ({e}). {what}: {text}"),
        };
    }

    fn cancel_current(&mut self) {
        match self.tab {
            Tab::Access if self.probe_pending > 0 => {
                self.probe_cancel.store(true, Ordering::Relaxed);
                self.status = "Cancelling probes…".into();
            }
            Tab::Orgs | Tab::Access => self.orgs.cancel(),
            Tab::Doctor => self.doctor.cancel(),
            t => {
                if let Some(p) = self.panel_mut(t) {
                    p.load().cancel();
                }
                self.status = "Cancelled.".into();
            }
        }
    }

    fn show_help(&mut self) {
        let lines = [
            "Mouse: click a tab or a row; click the selected row again to open it;",
            "       wheel scrolls. (Hold Shift/Option to select text in the terminal.)",
            "",
            "Keys (all tabs)",
            "  1-7 / Tab / Shift-Tab   switch tab        ↑↓ j k PgUp PgDn g G   move",
            "  Enter                   open / select     r   refresh this tab",
            "  Esc                     cancel / close    ?   help      q  quit",
            "",
            "Orgs           h  use as Dev Hub      o  use as target org   Enter  either",
            "Access         p  probe this org      a  probe all orgs      t  try 2GP on non-hubs",
            "               ←→ choose column       Enter  details of the selected cell",
            "2GP            left: packages · right: versions of the selected package",
            "               ←/→ or click: switch pane   ↑↓/click a package: show its versions",
            "               R released  L latest per package  V verbose  x  all packages",
            "               c copy 0Ho (left) / 04t (right)  i copy install link  y copy report cmd",
            "               Enter on a version: details (package version report)",
            "               a  ancestry (version: highlighted path; package: whole tree)",
            "               A  whole package ancestry    d  dependencies (install order)",
            "Graph view     c copy 04t  i install link  t order/tree (deps)  y copy command",
            "               Enter version details      Esc close",
            "Installed/1GP  c  copy 04t            Enter  details",
            "History        c  copy command        Enter  details (exit code, stderr)",
            "",
            "GP Atlas only runs read-only sf commands. Copied commands are never run.",
        ];
        self.popup = Some(Popup {
            title: "Help".into(),
            lines: lines.iter().map(|s| (*s).to_owned()).collect(),
            scroll: 0,
            loading_id: None,
        });
    }

    pub fn on_mouse(&mut self, m: MouseEvent) {
        let pos = Position::new(m.column, m.row);
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(r) = self.hits.popup {
                    if !r.contains(pos) {
                        self.popup = None;
                    }
                    return;
                }
                if self.graph.is_some() {
                    match (self.hits.graph, self.hits.graph_rows) {
                        (Some(area), _) if !area.contains(pos) => self.graph = None,
                        (_, Some(rows)) if rows.contains(pos) => {
                            let gv = self.graph.as_mut().expect("graph");
                            let idx = gv.scroll.offset + (m.row - rows.y) as usize;
                            if idx < gv.rows().len() {
                                let again = gv.scroll.selected == idx;
                                gv.scroll.selected = idx;
                                if again {
                                    self.graph_activate();
                                }
                            }
                        }
                        _ => {}
                    }
                    return;
                }
                if let Some(tab) = self
                    .hits
                    .tabs
                    .iter()
                    .find(|(r, _)| r.contains(pos))
                    .map(|(_, t)| *t)
                {
                    self.switch_tab(tab);
                    return;
                }
                let Some((rows, pane)) = self
                    .hits
                    .rows
                    .iter()
                    .copied()
                    .find(|(r, _)| r.contains(pos))
                else {
                    return;
                };
                if self.tab == Tab::Packages {
                    self.pkg_focus = pane == Tab::Packages;
                }
                if self.tab == Tab::Access
                    && let Some(i) = self
                        .hits
                        .access_cols
                        .iter()
                        .position(|(x0, x1)| m.column >= *x0 && m.column < *x1)
                {
                    self.access_col = i;
                }
                let len = self.table_len();
                let Some(s) = self.scroll_mut() else { return };
                let idx = s.offset + (m.row - rows.y) as usize;
                if idx >= len {
                    return;
                }
                let again = s.selected == idx;
                s.selected = idx;
                if pane == Tab::Packages {
                    // A click on a package shows its versions right away;
                    // clicking it again moves focus to the versions pane.
                    self.apply_package_selection();
                    if again {
                        self.pkg_focus = false;
                    }
                } else if again {
                    self.activate();
                }
            }
            MouseEventKind::ScrollDown if self.popup.is_none() && self.graph.is_some() => {
                self.graph_key(KeyCode::PageDown);
            }
            MouseEventKind::ScrollUp if self.popup.is_none() && self.graph.is_some() => {
                self.graph_key(KeyCode::PageUp);
            }
            MouseEventKind::ScrollDown => match &mut self.popup {
                Some(p) => p.scroll = p.scroll.saturating_add(3),
                None => self.move_sel(3),
            },
            MouseEventKind::ScrollUp => match &mut self.popup {
                Some(p) => p.scroll = p.scroll.saturating_sub(3),
                None => self.move_sel(-3),
            },
            _ => {}
        }
    }
}

fn finish(panel: &mut Panel, id: u64, outcome: Outcome) {
    if panel.load().loading_id() != Some(id) {
        return; // stale result (the user refreshed or switched org)
    }
    panel.load = Some(match outcome {
        Outcome::Ok { value, warnings } => Load::Ready(ListData {
            rows: match value {
                Value::Array(a) => a,
                Value::Null => vec![],
                other => vec![other],
            },
            warnings,
        }),
        Outcome::Failed(f) => Load::Failed(f),
        Outcome::Cancelled | Outcome::Doctor(_) => Load::Idle,
    });
}

pub fn str_of(v: &Value, key: &str) -> Option<String> {
    match v.get(key)? {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

pub fn cell_text(v: &Value, key: &str) -> String {
    match v.get(key) {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

pub fn kv_lines(v: &Value) -> Vec<String> {
    let obj = v.as_array().and_then(|a| a.first()).unwrap_or(v);
    match obj.as_object() {
        Some(o) => {
            let w = o.keys().map(String::len).max().unwrap_or(0);
            o.iter()
                .map(|(k, val)| {
                    let s = match val {
                        Value::Object(_) | Value::Array(_) => val.to_string(),
                        _ => cell_text(obj, k),
                    };
                    format!("{k:<w$}  {s}")
                })
                .collect()
        }
        None => v.to_string().lines().map(str::to_owned).collect(),
    }
}

pub fn failure_lines(f: &Failure, org: &str) -> Vec<String> {
    let mut lines = vec![format!("Result: {}", f.state.short())];
    if !f.name.is_empty() {
        lines.push(format!("Error:  {}", f.name));
    }
    lines.extend(f.message.lines().map(|l| format!("        {l}")));
    if !f.rule.is_empty() {
        lines.push(format!("Rule:   {}", f.rule));
    }
    if let Some(h) = f.state.hint(org) {
        lines.push(format!("Fix (copy, not run by GP Atlas): {h}"));
    }
    if !f.stderr.is_empty() && f.stderr != f.message {
        lines.push(String::new());
        lines.push("stderr:".into());
        lines.extend(f.stderr.lines().take(30).map(str::to_owned));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_fit_and_move() {
        let mut s = Scroll::default();
        s.move_by(15, 10);
        assert_eq!(s.selected, 9);
        s.fit(4, 10);
        assert_eq!(s.offset, 6);
        s.move_by(-100, 10);
        s.fit(4, 10);
        assert_eq!((s.selected, s.offset), (0, 0));
        s.move_by(1, 0);
        assert_eq!((s.selected, s.offset), (0, 0));
    }
}
