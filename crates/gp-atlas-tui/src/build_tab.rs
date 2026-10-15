//! Command Builder tab (SPEC §9): pick a read-only command, fill a form
//! generated from the manifest, see the exact command for your shell, copy
//! it, or run it in GP Atlas. Everything goes through `ReadOnlyCommand`.

use std::collections::BTreeMap;

use gp_atlas_core::builder::{
    self, BUILDER_COMMANDS, FieldError, FieldKind, Form, RenderOptions, Values,
};
use gp_atlas_core::command::ReadOnlyCommand;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::Value;

use crate::app::{App, Load, Popup, Scroll, cell_text, failure_lines, kv_lines, str_of};
use crate::worker::{Job, Outcome};

#[derive(Default)]
pub struct BuilderState {
    /// Index into `BUILDER_COMMANDS`.
    pub cmd: usize,
    pub cmd_scroll: Scroll,
    pub field_scroll: Scroll,
    /// Focus on the form (right) rather than the command list (left).
    pub in_form: bool,
    /// Values per command id, kept while switching commands.
    pub values: BTreeMap<String, Values>,
    /// Text being edited for the selected field.
    pub editing: Option<String>,
    pub opts: RenderOptions,
    /// Job id of a "Run in GP Atlas" in progress.
    pub running: Option<u64>,
}

/// What the preview shows for the current form.
pub struct Preview {
    pub form: Option<Form>,
    pub command: Result<ReadOnlyCommand, Vec<FieldError>>,
    pub text: Result<String, String>,
    pub notes: Vec<String>,
}

impl App {
    pub fn builder_id(&self) -> &'static str {
        BUILDER_COMMANDS[self.builder.cmd.min(BUILDER_COMMANDS.len() - 1)]
    }

    pub fn builder_form(&self) -> Option<Form> {
        builder::form(&self.manifest, self.builder_id()).ok()
    }

    fn builder_values(&self) -> Values {
        self.builder
            .values
            .get(self.builder_id())
            .cloned()
            .unwrap_or_default()
    }

    fn set_value(&mut self, flag: &str, value: String) {
        let id = self.builder_id().to_owned();
        let v = self.builder.values.entry(id).or_default();
        if value.is_empty() {
            v.remove(flag);
        } else {
            v.insert(flag.to_owned(), value);
        }
    }

    /// Fills empty fields from what is selected elsewhere (Dev Hub, org,
    /// 2GP package/version). Never overwrites what the user typed.
    pub fn builder_prefill(&mut self) {
        let Some(form) = self.builder_form() else {
            return;
        };
        let current = self.builder_values();
        let hub = self.hub.as_ref().map(|o| o.target().to_owned());
        let org = self.org.as_ref().map(|o| o.target().to_owned());
        let version = self
            .version_rows()
            .get(self.versions.scroll.selected)
            .and_then(|r| str_of(r, "SubscriberPackageVersionId"));
        let package = self
            .selected_package()
            .map(|(id, _)| id.as_str().to_owned());
        let id = self.builder_id();
        for f in form.fields {
            if current.contains_key(&f.flag) {
                continue;
            }
            let value = match (f.kind, f.flag.as_str(), id) {
                (FieldKind::Hub, ..) => hub.clone(),
                (FieldKind::Org, ..) => org.clone(),
                (_, "package", "package:version:displayancestry") => {
                    package.clone().or_else(|| version.clone())
                }
                (_, "package", _) => version.clone(),
                (_, "packages", _) => package.clone(),
                _ => None,
            };
            if let Some(v) = value {
                self.set_value(&f.flag, v);
            }
        }
    }

    pub fn builder_preview(&self) -> Preview {
        let form = self.builder_form();
        let values = self.builder_values();
        let command = builder::build(self.builder_id(), &values);
        let mut notes = Vec::new();
        let mut opts = self.builder.opts.clone();
        if let Ok(cmd) = &command
            && cmd.uses_project_alias()
        {
            match self.project.project() {
                Some(p) => {
                    let dir = p.dir().display().to_string();
                    notes.push(format!(
                        "A project alias resolves only inside the project: `cd {dir}` is added."
                    ));
                    opts.cd = Some(dir);
                }
                None => notes.push(
                    "A project alias resolves only inside an SFDX project, and no project is \
                     loaded: run it from the project folder, or use the ID (A swaps)."
                        .into(),
                ),
            }
        }
        if let Some(f) = &form
            && !f.not_offered.is_empty()
        {
            notes.push(format!(
                "Not offered here: {}",
                f.not_offered
                    .iter()
                    .map(|n| format!("--{n}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        let text = match &command {
            Ok(cmd) => builder::render(cmd, &self.manifest, &opts).map_err(|e| e.to_string()),
            Err(errs) => Err(format!(
                "Fix {} field{} to see the command.",
                errs.len(),
                if errs.len() == 1 { "" } else { "s" }
            )),
        };
        Preview {
            form,
            command,
            text,
            notes,
        }
    }

    fn builder_field(&self) -> Option<builder::Field> {
        self.builder_form()?
            .fields
            .get(self.builder.field_scroll.selected)
            .cloned()
    }

    /// Keys while a field is being edited. Returns true when handled.
    pub fn builder_edit_key(&mut self, k: KeyEvent) -> bool {
        let Some(buf) = self.builder.editing.as_mut() else {
            return false;
        };
        match k.code {
            KeyCode::Esc => self.builder.editing = None,
            KeyCode::Enter => {
                let value = buf.trim().to_owned();
                self.builder.editing = None;
                if let Some(f) = self.builder_field() {
                    self.set_value(&f.flag, value);
                }
            }
            KeyCode::Backspace => {
                buf.pop();
            }
            KeyCode::Char('u') if k.modifiers.contains(KeyModifiers::CONTROL) => buf.clear(),
            // Printable characters only; validation happens on build.
            KeyCode::Char(c) if !c.is_control() && buf.chars().count() < 1000 => buf.push(c),
            _ => {}
        }
        true
    }

    /// Enter / Space on the form: edit text, toggle a switch, cycle a choice.
    pub fn builder_activate(&mut self) {
        if !self.builder.in_form {
            self.builder.in_form = true;
            self.builder_prefill();
            return;
        }
        let Some(f) = self.builder_field() else {
            return;
        };
        let current = self.builder_values().get(&f.flag).cloned();
        match f.kind {
            FieldKind::Bool => {
                let on = current.as_deref() == Some("true");
                self.set_value(&f.flag, if on { String::new() } else { "true".into() });
            }
            FieldKind::Choice(opts) => {
                // (none) → first → … → last → (none)
                let next = match current.and_then(|c| opts.iter().position(|o| *o == c)) {
                    None => opts.first().cloned(),
                    Some(i) => opts.get(i + 1).cloned(),
                };
                self.set_value(&f.flag, next.unwrap_or_default());
            }
            _ => self.builder.editing = Some(current.unwrap_or_default()),
        }
    }

    pub fn builder_select_command(&mut self, i: usize) {
        if i < BUILDER_COMMANDS.len() {
            self.builder.cmd = i;
            self.builder.cmd_scroll.selected = i;
            self.builder.field_scroll = Scroll::default();
            self.builder.editing = None;
            self.builder_prefill();
        }
    }

    /// Builder-only keys (not editing). Returns true when handled.
    pub fn builder_key(&mut self, c: char) -> bool {
        let o = &mut self.builder.opts;
        match c {
            's' => o.shell = o.shell.next(),
            'J' => o.json = !o.json,
            'F' => o.short_flags = !o.short_flags,
            'M' => o.multiline = !o.multiline,
            'U' => self.builder_swap_org(),
            'A' => self.builder_swap_package(),
            'c' => self.builder_copy(),
            'R' => self.builder_run(),
            'x' => {
                let id = self.builder_id().to_owned();
                self.builder.values.remove(&id);
                self.status = "Form cleared.".into();
            }
            'f' => {
                self.builder_prefill();
                self.status =
                    "Empty fields filled from the selected Dev Hub, org and 2GP rows.".into();
            }
            _ => return false,
        }
        true
    }

    /// Delete / Backspace on a field clears it.
    pub fn builder_clear_field(&mut self) {
        if let Some(f) = self.builder_field() {
            self.set_value(&f.flag, String::new());
        }
    }

    /// `U`: alias ↔ username for the Dev Hub / org field (SPEC §6.1).
    fn builder_swap_org(&mut self) {
        let Some(f) = self
            .builder_field()
            .filter(|f| matches!(f.kind, FieldKind::Hub | FieldKind::Org))
        else {
            self.status = "Select the Dev Hub / org field, then press U.".into();
            return;
        };
        let Some(cur) = self.builder_values().get(&f.flag).cloned() else {
            return;
        };
        let Load::Ready(list) = &self.orgs else {
            return;
        };
        let next = list.iter().find_map(|o| {
            if o.alias.as_deref() == Some(cur.as_str()) {
                Some(o.username.clone())
            } else if o.username == cur {
                o.alias.clone()
            } else {
                None
            }
        });
        match next {
            Some(n) => self.set_value(&f.flag, n),
            None => self.status = format!("{cur}: no alias/username pair in org list."),
        }
    }

    /// `A`: package ID ↔ project alias (SPEC §6.4).
    fn builder_swap_package(&mut self) {
        let Some(p) = self.project.project() else {
            self.status =
                "No project loaded (start inside an SFDX project or use --project).".into();
            return;
        };
        let Some(f) = self
            .builder_field()
            .filter(|f| matches!(f.flag.as_str(), "package" | "packages"))
        else {
            self.status = "Select a package field, then press A.".into();
            return;
        };
        let Some(cur) = self.builder_values().get(&f.flag).cloned() else {
            return;
        };
        let swap = |item: &str| -> String {
            if let Some(a) = p.aliases_for(item).first() {
                return (*a).to_owned();
            }
            p.entries
                .iter()
                .find(|e| e.alias == item)
                .map_or_else(|| item.to_owned(), |e| e.id.clone())
        };
        let next: Vec<String> = cur.split(',').map(|s| swap(s.trim())).collect();
        self.set_value(&f.flag, next.join(","));
    }

    fn builder_copy(&mut self) {
        match self.builder_preview().text {
            Ok(t) => self.copy_text(&t, "command"),
            Err(e) => self.status = e,
        }
    }

    /// `R`: runs the built command (always read-only, always `--json`).
    fn builder_run(&mut self) {
        let p = self.builder_preview();
        let Ok(cmd) = p.command else {
            self.status = "Fix the highlighted fields first.".into();
            return;
        };
        let cwd = if cmd.uses_project_alias() {
            match self.project.project() {
                Some(pr) => Some(pr.dir().to_path_buf()),
                None => {
                    self.status =
                        "A project alias needs a loaded project to run; use the ID (A).".into();
                    return;
                }
            }
        } else {
            None
        };
        let title = format!("sf {}", cmd.id().replace(':', " "));
        match self.submit_job(Job::Run {
            cmd: Box::new(cmd),
            cwd,
        }) {
            Ok(id) => {
                self.builder.running = Some(id);
                self.popup = Some(Popup {
                    title,
                    lines: vec![
                        "Running… (Esc closes this; the result also goes to History)".into(),
                    ],
                    scroll: 0,
                    loading_id: Some(id),
                });
            }
            Err(f) => {
                self.popup = Some(Popup {
                    title,
                    lines: failure_lines(&f, ""),
                    scroll: 0,
                    loading_id: None,
                });
            }
        }
    }

    /// Left click in the command list or the form. Returns true when handled.
    pub fn builder_click(&mut self, pos: ratatui::layout::Position) -> bool {
        if let Some(r) = self.hits.builder_cmds
            && r.contains(pos)
        {
            let i = self.builder.cmd_scroll.offset + (pos.y - r.y) as usize;
            self.builder.in_form = false;
            self.builder_select_command(i);
            return true;
        }
        if let Some(r) = self.hits.builder_fields
            && r.contains(pos)
        {
            let i = self.builder.field_scroll.offset + (pos.y - r.y) as usize;
            let n = self.builder_form().map_or(0, |f| f.fields.len());
            if i < n {
                let again = self.builder.in_form && self.builder.field_scroll.selected == i;
                self.builder.in_form = true;
                self.builder.editing = None;
                self.builder.field_scroll.selected = i;
                if again {
                    self.builder_activate();
                }
            }
            return true;
        }
        false
    }

    pub fn on_builder_run(&mut self, id: u64, outcome: Outcome) {
        if self.builder.running == Some(id) {
            self.builder.running = None;
        }
        let Some(p) = &mut self.popup else { return };
        if p.loading_id != Some(id) {
            return;
        }
        p.loading_id = None;
        p.lines = match outcome {
            Outcome::Ok { value, warnings } => {
                let mut l = result_lines(&value);
                l.extend(warnings.into_iter().map(|w| format!("warning: {w}")));
                l
            }
            Outcome::Failed(f) => failure_lines(&f, ""),
            _ => vec!["Cancelled.".into()],
        };
    }
}

/// Readable lines for any result: rows of an array, key/values of an object,
/// or the text (DOT).
fn result_lines(v: &Value) -> Vec<String> {
    match v {
        Value::Array(rows) if rows.is_empty() => vec!["No results.".into()],
        Value::Array(rows) => {
            let mut l = vec![
                format!("{} rows (c copies this text)", rows.len()),
                String::new(),
            ];
            for (i, r) in rows.iter().enumerate().take(500) {
                match r.as_object() {
                    Some(o) => {
                        l.push(format!("#{}", i + 1));
                        l.extend(
                            o.keys()
                                .map(|k| format!("  {k}: {}", cell_text(r, k)))
                                .filter(|s| !s.ends_with(": ")),
                        );
                    }
                    None => l.push(r.to_string()),
                }
            }
            if rows.len() > 500 {
                l.push(format!(
                    "… {} more rows (copy the command to see all)",
                    rows.len() - 500
                ));
            }
            l
        }
        Value::String(s) => s.lines().map(str::to_owned).collect(),
        other => kv_lines(other),
    }
}
