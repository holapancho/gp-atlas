//! Rendering. Every frame records clickable regions in `app.hits`.

use std::time::Instant;

use gp_atlas_core::classify::CapState;
use gp_atlas_core::doctor::Check;
use gp_atlas_core::probes::Capability;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Cell as TCell, Clear, Paragraph, Row, Table, TableState, Wrap,
};

use crate::app::{App, Cell, Gate, Load, Scroll, Tab, cell_text, failure_lines};

const SPINNER: [&str; 4] = ["◐", "◓", "◑", "◒"];

fn spinner() -> &'static str {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    SPINNER[(ms / 150 % 4) as usize]
}

fn dim() -> Style {
    Style::new().fg(Color::DarkGray)
}

pub fn draw(f: &mut Frame, app: &mut App) {
    app.hits = Default::default();
    let [header, tabs, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(2),
    ])
    .areas(f.area());
    draw_header(f, app, header);
    draw_tabs(f, app, tabs);
    match app.tab {
        Tab::Doctor => draw_doctor(f, app, body),
        Tab::Orgs => draw_orgs(f, app, body),
        Tab::Access => draw_access(f, app, body),
        Tab::Packages | Tab::Versions | Tab::Installed | Tab::Pkg1 => draw_panel(f, app, body),
        Tab::Log => draw_log(f, app, body),
    }
    draw_footer(f, app, footer);
    if app.popup.is_some() {
        draw_popup(f, app);
    }
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let mut spans = vec![
        Span::styled(" GP Atlas ", Style::new().bold().reversed()),
        " ".into(),
    ];
    match &app.gate {
        Gate::Checking => spans.push(Span::styled(format!("{} checking sf…", spinner()), dim())),
        Gate::Ok { version, newer } => {
            spans.push(Span::styled(
                format!("sf {version} ✓"),
                Style::new().fg(Color::Green),
            ));
            if *newer {
                spans.push(Span::styled(" (newer than baseline)", dim()));
            }
        }
        Gate::Blocked(_) => spans.push(Span::styled(
            "sf ⛔ see Doctor",
            Style::new().fg(Color::Red).bold(),
        )),
    }
    let label = |o: &Option<gp_atlas_core::orgs::Org>| {
        o.as_ref()
            .map(|o| o.target().to_owned())
            .unwrap_or_else(|| "—".into())
    };
    spans.push(Span::styled("  │  Dev Hub: ", dim()));
    spans.push(Span::styled(
        label(&app.hub),
        Style::new().fg(Color::Cyan).bold(),
    ));
    spans.push(Span::styled("  │  Org: ", dim()));
    spans.push(Span::styled(
        label(&app.org),
        Style::new().fg(Color::Cyan).bold(),
    ));
    if app.any_loading() {
        spans.push(Span::styled(
            format!("  {} working", spinner()),
            Style::new().fg(Color::Yellow),
        ));
    }
    spans.push(Span::styled("  │  read-only", dim()));
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_tabs(f: &mut Frame, app: &mut App, area: Rect) {
    let mut x = area.x;
    let mut spans = Vec::new();
    for (i, t) in Tab::ALL.iter().enumerate() {
        let text = format!(" {} {} ", i + 1, t.title());
        let w = text.chars().count() as u16;
        let style = if *t == app.tab {
            Style::new().fg(Color::Black).bg(Color::Cyan).bold()
        } else {
            Style::new().fg(Color::Gray)
        };
        if x < area.right() {
            app.hits
                .tabs
                .push((Rect::new(x, area.y, w.min(area.right() - x), 1), *t));
        }
        spans.push(Span::styled(text, style));
        spans.push(" ".into());
        x += w + 1;
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let hints = match app.tab {
        Tab::Doctor => "r re-run checks",
        Tab::Orgs => "h use as Dev Hub · o use as org · Enter select · r refresh",
        Tab::Access => {
            "p probe org · a probe all · t try 2GP on non-hubs · ←→ column · Enter details · Esc cancel"
        }
        Tab::Packages => "Enter versions · c copy 0Ho · r refresh",
        Tab::Versions => {
            "R released · L latest · V verbose · x all pkgs · c 04t · i install link · y report cmd · Enter details"
        }
        Tab::Installed | Tab::Pkg1 => "c copy 04t · Enter details · r refresh",
        Tab::Log => "Enter details · c copy command",
    };
    let lines = vec![
        Line::from(vec![
            Span::styled(" ", dim()),
            Span::styled(hints, dim()),
            Span::styled("   ? help · q quit", dim()),
        ]),
        Line::from(Span::styled(
            format!(" {}", app.status),
            Style::new().fg(Color::Yellow),
        )),
    ];
    f.render_widget(Paragraph::new(lines), area);
}

fn check_line(name: &str, c: &Check) -> Line<'static> {
    let (icon, color, text) = match c {
        Check::Pass(t) => ("✅", Color::Green, t.clone()),
        Check::Fail(t) => ("⛔", Color::Red, t.clone()),
        Check::Skipped(t) => ("– ", Color::DarkGray, t.clone()),
    };
    Line::from(vec![
        Span::raw(format!(" {icon} ")),
        Span::styled(format!("{name:<30}"), Style::new().bold()),
        Span::styled(text, Style::new().fg(color)),
    ])
}

fn draw_doctor(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::bordered().title(" Doctor — Salesforce CLI checks ");
    let lines: Vec<Line> = match &app.doctor {
        Load::Idle => vec![Line::from(" Press r to run the checks.")],
        Load::Loading { since, .. } => vec![Line::from(format!(
            " {} Running sf version / plugins / commands… {:.0}s",
            spinner(),
            since.elapsed().as_secs_f64()
        ))],
        Load::Failed(fl) => failure_lines(fl, "").into_iter().map(Line::from).collect(),
        Load::Ready(r) => {
            let mut l = vec![
                Line::from(""),
                check_line("D1 sf runnable", &r.d1_runnable),
                check_line(
                    &format!("D2 CLI version ≥ {}", gp_atlas_core::MIN_CLI_VERSION),
                    &r.d2_version,
                ),
                check_line("D3 bundled packaging plugin", &r.d3_packaging_plugin),
                check_line("D4 command contract", &r.d4_contract),
                Line::from(""),
            ];
            for d in &r.drift {
                l.push(Line::styled(
                    format!("   ⚠ `sf {}` disabled: {}", d.command, d.details.join("; ")),
                    Style::new().fg(Color::Yellow),
                ));
            }
            match &app.gate {
                Gate::Blocked(m) => {
                    l.push(Line::styled(
                        format!(" {m}"),
                        Style::new().fg(Color::Red).bold(),
                    ));
                    l.push(Line::from(format!(
                        " Fix: {}",
                        gp_atlas_core::CLI_INSTALL_COMMAND
                    )));
                }
                Gate::Ok { .. } => l.push(Line::from(
                    " CLI usable. Go to Orgs (2) to pick a Dev Hub and an org.",
                )),
                Gate::Checking => {}
            }
            l
        }
    };
    f.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );
}

/// Draws a table, building only the visible rows. Records the rows area for clicks.
#[allow(clippy::too_many_arguments)]
fn draw_rows(
    f: &mut Frame,
    hits_rows: &mut Option<Rect>,
    area: Rect,
    title: String,
    header: &[&str],
    widths: &[Constraint],
    len: usize,
    scroll: &mut Scroll,
    row: impl Fn(usize) -> Vec<TCell<'static>>,
) {
    let block = Block::bordered().title(title);
    let inner = block.inner(area);
    let height = inner.height.saturating_sub(1) as usize; // minus header row
    scroll.fit(height, len);
    let end = (scroll.offset + height).min(len);
    let rows: Vec<Row> = (scroll.offset..end).map(|i| Row::new(row(i))).collect();
    let mut state = TableState::default();
    if len > 0 {
        state.select(Some(scroll.selected - scroll.offset));
    }
    let table = Table::new(rows, widths.to_vec())
        .header(
            Row::new(header.iter().map(|h| TCell::from(h.to_string())))
                .style(Style::new().bold().underlined()),
        )
        .row_highlight_style(
            Style::new()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ")
        .block(block);
    f.render_stateful_widget(table, area, &mut state);
    *hits_rows = Some(Rect::new(
        inner.x,
        inner.y + 1,
        inner.width,
        inner.height.saturating_sub(1),
    ));
}

fn message(f: &mut Frame, area: Rect, title: String, lines: Vec<Line<'static>>) {
    f.render_widget(
        Paragraph::new(lines)
            .block(Block::bordered().title(title))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn loading_line(since: &Instant, what: &str) -> Line<'static> {
    Line::from(format!(
        " {} {what}… {:.0}s  (Esc cancels)",
        spinner(),
        since.elapsed().as_secs_f64()
    ))
}

fn draw_orgs(f: &mut Frame, app: &mut App, area: Rect) {
    let title = " Orgs — h: use as Dev Hub, o: use as target org ".to_owned();
    let list = match &app.orgs {
        Load::Ready(l) => l.clone(),
        Load::Loading { since, .. } => {
            return message(f, area, title, vec![loading_line(since, "Loading orgs")]);
        }
        Load::Failed(fl) => {
            return message(
                f,
                area,
                title,
                failure_lines(fl, "").into_iter().map(Line::from).collect(),
            );
        }
        Load::Idle => {
            return message(
                f,
                area,
                title,
                vec![Line::from(" Waiting for the CLI check…")],
            );
        }
    };
    if list.is_empty() {
        return message(
            f,
            area,
            title,
            vec![
                Line::from(" sf knows no orgs yet. Log in in another terminal, then press r:"),
                Line::from(""),
                Line::from("   sf org login web --alias my-devhub --set-default-dev-hub"),
                Line::from(""),
                Line::styled(" (GP Atlas never logs in for you; it only reads.)", dim()),
            ],
        );
    }
    let hub = app.hub.as_ref().map(|o| o.username.clone());
    let org = app.org.as_ref().map(|o| o.username.clone());
    draw_rows(
        f,
        &mut app.hits.rows,
        area,
        format!("{title}· {} orgs ", list.len()),
        &["Use", "Alias — username", "Type", "Dev Hub", "Status"],
        &[
            Constraint::Length(9),
            Constraint::Percentage(45),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Fill(1),
        ],
        list.len(),
        &mut app.orgs_scroll,
        |i| {
            let o = &list[i];
            let mut using = String::new();
            if hub.as_deref() == Some(&o.username) {
                using.push_str("HUB ");
            }
            if org.as_deref() == Some(&o.username) {
                using.push_str("ORG");
            }
            let status = o
                .connected_status
                .clone()
                .or_else(|| o.status.clone())
                .unwrap_or_default();
            let status_style = match o.is_connected() {
                Some(true) => Style::new().fg(Color::Green),
                Some(false) => Style::new().fg(Color::Red),
                None => dim(),
            };
            let mut label = o.label();
            if o.is_default_dev_hub {
                label.push_str(" (D)");
            }
            if o.is_default_org {
                label.push_str(" (U)");
            }
            vec![
                TCell::from(using).style(Style::new().fg(Color::Cyan).bold()),
                TCell::from(label),
                TCell::from(o.kind.badge()),
                TCell::from(if o.is_dev_hub { "yes" } else { "" }),
                TCell::from(status.replace('\n', " ")).style(status_style),
            ]
        },
    );
}

fn cell_view(c: Option<&Cell>) -> (String, Style) {
    match c {
        None | Some(Cell::NotRun) => ("·".into(), dim()),
        Some(Cell::NotApplicable(_)) => ("n/a".into(), dim()),
        Some(Cell::Pending) => (format!("{} …", spinner()), Style::new().fg(Color::Yellow)),
        Some(Cell::Done { state, .. }) => match state {
            CapState::Allowed => ("✅ allowed".into(), Style::new().fg(Color::Green)),
            CapState::Denied(r) => (format!("⛔ {r:?}"), Style::new().fg(Color::Red)),
            CapState::Unreachable(r) => (format!("🔌 {r:?}"), Style::new().fg(Color::Magenta)),
            CapState::Unknown(r) => (format!("? {r:?}"), Style::new().fg(Color::Yellow)),
            CapState::NotApplicable(_) => ("n/a".into(), dim()),
            CapState::ContractDrift(_) => ("⚠ drift".into(), Style::new().fg(Color::Yellow)),
        },
    }
}

fn draw_access(f: &mut Frame, app: &mut App, area: Rect) {
    let title = format!(
        " Access Matrix — p: probe org · a: all · t: try 2GP on non-hubs [{}] ",
        if app.try_anyway { "on" } else { "off" }
    );
    let list = match &app.orgs {
        Load::Ready(l) => l.clone(),
        _ => {
            return message(
                f,
                area,
                title,
                vec![Line::from(" Orgs are not loaded yet (see Orgs tab).")],
            );
        }
    };
    let first = 32u16;
    let colw = 18u16;
    let mut widths = vec![Constraint::Length(first)];
    widths.extend(Capability::ALL.iter().map(|_| Constraint::Length(colw)));
    let mut header = vec!["Org"];
    header.extend(Capability::ALL.iter().map(|c| c.short()));
    // Column hitboxes: border(1) + highlight symbol(2) + first column + spacing(1).
    let mut x = area.x + 1 + 2 + first + 1;
    for _ in Capability::ALL {
        app.hits.access_cols.push((x, x + colw));
        x += colw + 1;
    }
    let col = app.access_col;
    let cells = &app.cells;
    draw_rows(
        f,
        &mut app.hits.rows,
        area,
        title,
        &header,
        &widths,
        list.len(),
        &mut app.access_scroll,
        |i| {
            let o = &list[i];
            let mut v = vec![TCell::from(o.target().to_owned())];
            for (j, cap) in Capability::ALL.iter().enumerate() {
                let (t, mut s) = cell_view(cells.get(&(o.username.clone(), *cap)));
                if j == col {
                    s = s.add_modifier(Modifier::UNDERLINED);
                }
                v.push(TCell::from(t).style(s));
            }
            v
        },
    );
}

fn draw_panel(f: &mut Frame, app: &mut App, area: Rect) {
    let tab = app.tab;
    let target = app.tab_target(tab).map(|o| o.target().to_owned());
    let mut title = match (tab, &target) {
        (Tab::Packages, Some(t)) => format!(" 2GP packages in {t} "),
        (Tab::Versions, Some(t)) => format!(" 2GP versions in {t} "),
        (Tab::Installed, Some(t)) => format!(" Installed in {t} "),
        (Tab::Pkg1, Some(t)) => format!(" 1GP versions in {t} "),
        _ => format!(" {} ", tab.title()),
    };
    if tab == Tab::Versions {
        let v = &app.vfilter;
        let mut flags = Vec::new();
        if let Some((_, name)) = &v.package {
            flags.push(format!("package: {name}"));
        }
        if v.released {
            flags.push("released".into());
        }
        if v.latest {
            flags.push("latest per package (computed by GP Atlas)".into());
        }
        if v.verbose {
            flags.push("verbose".into());
        }
        if !flags.is_empty() {
            title.push_str(&format!("[{}] ", flags.join(", ")));
        }
    }
    let panel = match tab {
        Tab::Packages => &app.packages,
        Tab::Versions => &app.versions,
        Tab::Installed => &app.installed,
        _ => &app.pkg1,
    };
    match panel.load() {
        Load::Idle => {
            let msg = if target.is_none() {
                match tab {
                    Tab::Packages | Tab::Versions => {
                        " No Dev Hub selected. Go to Orgs (2), select a Dev Hub, press h."
                    }
                    _ => " No org selected. Go to Orgs (2), select an org, press o.",
                }
            } else {
                " Press r to load."
            };
            return message(f, area, title, vec![Line::from(msg)]);
        }
        Load::Loading { since, .. } => {
            return message(f, area, title, vec![loading_line(since, "Running sf")]);
        }
        Load::Failed(fl) => {
            let mut l: Vec<Line> = failure_lines(fl, target.as_deref().unwrap_or(""))
                .into_iter()
                .map(Line::from)
                .collect();
            l.push(Line::from(""));
            l.push(Line::styled(
                " r retry · History (8) shows the exact sf command",
                dim(),
            ));
            return message(f, area, title, l);
        }
        Load::Ready(_) => {}
    }
    let (header, widths, keys): (Vec<&str>, Vec<Constraint>, Vec<&str>) = match tab {
        Tab::Packages => (
            vec!["Name", "Namespace", "Type", "Org-dep.", "0Ho", "033"],
            vec![
                Constraint::Fill(2),
                Constraint::Length(12),
                Constraint::Length(9),
                Constraint::Length(8),
                Constraint::Length(19),
                Constraint::Length(19),
            ],
            vec![
                "Name",
                "NamespacePrefix",
                "ContainerOptions",
                "IsOrgDependent",
                "Id",
                "SubscriberPackageId",
            ],
        ),
        Tab::Versions => {
            let mut h = vec![
                "Package", "Version", "04t", "Released", "Branch", "Ancestor", "Created",
            ];
            let mut w = vec![
                Constraint::Fill(2),
                Constraint::Length(12),
                Constraint::Length(19),
                Constraint::Length(8),
                Constraint::Length(12),
                Constraint::Length(10),
                Constraint::Length(16),
            ];
            let mut k = vec![
                "Package2Name",
                "Version",
                "SubscriberPackageVersionId",
                "IsReleased",
                "Branch",
                "AncestorVersion",
                "CreatedDate",
            ];
            if app.vfilter.verbose {
                h.push("Coverage");
                w.push(Constraint::Length(9));
                k.push("CodeCoverage");
            }
            (h, w, k)
        }
        Tab::Installed => (
            vec!["Package", "Namespace", "Version", "04t", "Version settings"],
            vec![
                Constraint::Fill(2),
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(19),
                Constraint::Length(16),
            ],
            vec![
                "SubscriberPackageName",
                "SubscriberPackageNamespace",
                "SubscriberPackageVersionNumber",
                "SubscriberPackageVersionId",
                "VersionSettings",
            ],
        ),
        _ => (
            vec!["Name", "Release state", "Version", "Build", "04t", "033"],
            vec![
                Constraint::Fill(2),
                Constraint::Length(13),
                Constraint::Length(10),
                Constraint::Length(6),
                Constraint::Length(19),
                Constraint::Length(19),
            ],
            vec![
                "Name",
                "ReleaseState",
                "Version",
                "BuildNumber",
                "MetadataPackageVersionId",
                "MetadataPackageId",
            ],
        ),
    };
    let rows: Vec<serde_json::Value> = if tab == Tab::Versions {
        app.version_rows().into_iter().cloned().collect()
    } else {
        match panel.load() {
            Load::Ready(d) => d.rows.clone(),
            _ => vec![],
        }
    };
    let warnings = match panel.load() {
        Load::Ready(d) => d.warnings.clone(),
        _ => vec![],
    };
    if rows.is_empty() {
        let mut l = vec![Line::from(" No results.")];
        l.extend(
            warnings
                .iter()
                .map(|w| Line::styled(format!(" warning: {w}"), dim())),
        );
        return message(f, area, title, l);
    }
    title.push_str(&format!("· {} rows ", rows.len()));
    let scroll = match tab {
        Tab::Packages => &mut app.packages.scroll,
        Tab::Versions => &mut app.versions.scroll,
        Tab::Installed => &mut app.installed.scroll,
        _ => &mut app.pkg1.scroll,
    };
    draw_rows(
        f,
        &mut app.hits.rows,
        area,
        title,
        &header,
        &widths,
        rows.len(),
        scroll,
        |i| {
            keys.iter()
                .map(|k| {
                    let t = cell_text(&rows[i], k);
                    let style = if *k == "IsReleased" && t == "true" {
                        Style::new().fg(Color::Green)
                    } else {
                        Style::new()
                    };
                    TCell::from(t).style(style)
                })
                .collect()
        },
    );
}

fn draw_log(f: &mut Frame, app: &mut App, area: Rect) {
    let entries: Vec<_> = app.log.iter().rev().cloned().collect();
    if entries.is_empty() {
        return message(
            f,
            area,
            " History ".into(),
            vec![Line::from(" No sf calls yet.")],
        );
    }
    draw_rows(
        f,
        &mut app.hits.rows,
        area,
        format!(" History — last {} sf calls (newest first) ", entries.len()),
        &["Time UTC", "Exit", "Secs", "Result", "Command"],
        &[
            Constraint::Length(9),
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Length(26),
            Constraint::Fill(1),
        ],
        entries.len(),
        &mut app.log_scroll,
        |i| {
            let e = &entries[i];
            let secs =
                e.at.duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
            let (h, m, s) = ((secs / 3600) % 24, (secs / 60) % 60, secs % 60);
            let ok = e.state == "Allowed";
            vec![
                TCell::from(format!("{h:02}:{m:02}:{s:02}")),
                TCell::from(e.exit.map_or("kill".into(), |c| c.to_string())),
                TCell::from(format!("{:.1}", e.duration.as_secs_f64())),
                TCell::from(e.state.clone()).style(if ok {
                    Style::new().fg(Color::Green)
                } else {
                    Style::new().fg(Color::Yellow)
                }),
                TCell::from(e.command.clone()),
            ]
        },
    );
}

fn draw_popup(f: &mut Frame, app: &mut App) {
    let Some(p) = &app.popup else { return };
    let area = f.area();
    let w = area.width.saturating_sub(8).min(110);
    let h = area
        .height
        .saturating_sub(4)
        .min(p.lines.len() as u16 + 4)
        .max(6)
        .min(area.height);
    let rect = Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    );
    let mut title = format!(" {} ", p.title);
    if p.loading_id.is_some() {
        title.push_str(&format!("{} ", spinner()));
    }
    let lines: Vec<Line> = p.lines.iter().map(|l| Line::from(l.clone())).collect();
    f.render_widget(Clear, rect);
    f.render_widget(
        Paragraph::new(lines)
            .block(
                Block::new()
                    .borders(Borders::ALL)
                    .title(title)
                    .title_bottom(Line::from(" Esc close · ↑↓ scroll · c copy ").right_aligned())
                    .style(Style::new().bg(Color::Black)),
            )
            .wrap(Wrap { trim: false })
            .scroll((p.scroll, 0)),
        rect,
    );
    app.hits.popup = Some(rect);
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::mpsc;

    use gp_atlas_core::doctor::{Check, DoctorReport};
    use gp_atlas_core::envelope::{SfOutput, parse};
    use gp_atlas_core::manifest::Manifest;
    use gp_atlas_core::orgs;
    use gp_atlas_core::runner::{RunnerConfig, SfRunner};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::app::{App, ListData, Popup};
    use crate::worker::Pool;

    fn fixture(name: &str) -> serde_json::Value {
        let b = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/sf-2.150.6")
                .join(name),
        )
        .unwrap();
        match parse(&b).unwrap() {
            SfOutput::Success { result, .. } => result,
            other => panic!("{other:?}"),
        }
    }

    fn app() -> App {
        let (tx, _rx) = mpsc::channel();
        let runner = SfRunner::new(RunnerConfig::new(PathBuf::from("/nonexistent/sf")));
        let m = Manifest::embedded().unwrap();
        let mut app = App::new(Pool::start(runner, m.clone(), tx), m, None);
        app.doctor = Load::Ready(Box::new(DoctorReport {
            d1_runnable: Check::Pass("ok".into()),
            d2_version: Check::Pass("@salesforce/cli/2.150.6".into()),
            found_version: Some("@salesforce/cli/2.150.6".into()),
            d3_packaging_plugin: Check::Pass("ok".into()),
            d4_contract: Check::Pass("ok".into()),
            drift: vec![],
            runs: vec![],
        }));
        app.gate = Gate::Ok {
            version: "2.150.6".into(),
            newer: false,
        };
        let list = orgs::from_org_list(&fixture("org-list.real.json"));
        app.hub = list.iter().find(|o| o.is_default_dev_hub).cloned();
        app.org = list.iter().find(|o| o.is_default_org).cloned();
        app.orgs = Load::Ready(list);
        let rows = fixture("package-list.real.json")
            .as_array()
            .unwrap()
            .clone();
        app.packages.load = Some(Load::Ready(ListData {
            rows,
            warnings: vec![],
        }));
        app
    }

    #[test]
    fn every_tab_renders_at_any_size() {
        let mut app = app();
        for (w, h) in [(150, 40), (80, 24), (40, 10), (12, 5), (1, 1)] {
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            for tab in Tab::ALL {
                app.tab = tab;
                for popup in [false, true] {
                    app.popup = popup.then(|| Popup {
                        title: "t".into(),
                        lines: vec!["x".repeat(300); 80],
                        scroll: 0,
                        loading_id: None,
                    });
                    term.draw(|f| draw(f, &mut app)).unwrap();
                }
            }
        }
    }

    #[test]
    fn content_and_click_targets() {
        let mut app = app();
        app.tab = Tab::Packages;
        let mut term = Terminal::new(TestBackend::new(150, 30)).unwrap();
        term.draw(|f| draw(f, &mut app)).unwrap();
        let text: String = term
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("2GP packages in fake0009"), "{text}");
        assert!(text.contains("0Ho00000000000jCAA"));
        assert_eq!(app.hits.tabs.len(), Tab::ALL.len());
        assert!(app.hits.rows.is_some());
    }
}
