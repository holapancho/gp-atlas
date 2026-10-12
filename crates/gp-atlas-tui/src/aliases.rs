//! Project package aliases in the TUI (SPEC §6): alias column, warnings,
//! `packageAliases` snippets. `sfdx-project.json` is only ever read.

use std::path::PathBuf;

use gp_atlas_core::project::{
    AliasKind, Project, ProjectError, id15, printable, snippet, version_alias,
};
use serde_json::Value;

use crate::app::str_of;

/// Where the project comes from (command line).
pub enum ProjectSource {
    /// Search the start folder and its parents, like the CLI (F32).
    Auto(PathBuf),
    /// `--project PATH` (a folder or the file itself).
    Path(PathBuf),
    /// `--no-project`.
    Off,
}

pub enum ProjectState {
    Off,
    NotFound(PathBuf),
    Loaded(Project),
    Failed(String),
}

impl ProjectState {
    pub fn load(src: &ProjectSource) -> Self {
        let (start, explicit) = match src {
            ProjectSource::Off => return Self::Off,
            ProjectSource::Auto(d) => (d, false),
            ProjectSource::Path(p) => (p, true),
        };
        match Project::discover(start) {
            Ok(p) => Self::Loaded(p),
            Err(ProjectError::NotFound(d)) if !explicit => Self::NotFound(d),
            Err(e) => Self::Failed(printable(&e.to_string())),
        }
    }

    pub fn project(&self) -> Option<&Project> {
        match self {
            Self::Loaded(p) => Some(p),
            _ => None,
        }
    }

    /// Header chip.
    pub fn chip(&self) -> String {
        match self {
            Self::Off => "off".into(),
            Self::NotFound(_) => "none".into(),
            Self::Failed(_) => "error (P)".into(),
            Self::Loaded(p) => {
                let name = p
                    .dir()
                    .file_name()
                    .map(|n| printable(&n.to_string_lossy()))
                    .unwrap_or_else(|| printable(&p.dir().display().to_string()));
                format!("{name} · {} aliases", p.entries.len())
            }
        }
    }
}

/// What the Alias column shows for a row.
pub struct AliasCell {
    pub text: String,
    /// The CLI's own `Alias` disagrees with the project, or the ID has several
    /// aliases.
    pub warn: bool,
}

/// Alias column for a row whose ID is in `id_key` (primary alias, `+N` more).
pub fn alias_cell(p: &Project, row: &Value, id_key: &str) -> AliasCell {
    let id = str_of(row, id_key).unwrap_or_default();
    let ours = p.aliases_for(&id);
    // F12/F13: the CLI fills `Alias` (CSV) only inside a project.
    let cli: Vec<String> = str_of(row, "Alias")
        .map(|s| {
            s.split(',')
                .map(|a| a.trim().to_owned())
                .filter(|a| !a.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let mismatch = cli.iter().any(|a| !ours.contains(&a.as_str()));
    let mut text = match ours.as_slice() {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [first, rest @ ..] => format!("{first} +{}", rest.len()),
    };
    if mismatch {
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(&format!("≠ sf: {}", printable(&cli.join(","))));
    }
    AliasCell {
        text,
        warn: mismatch || ours.len() > 1,
    }
}

/// What the loaded versions cover, for the 04t orphan check.
pub enum VersionScope<'a> {
    /// Every version in the Dev Hub (all packages, released or not).
    All(&'a [Value]),
    /// A subset (one package, or released only): orphans can't be told apart.
    Partial,
    NotLoaded,
}

/// Number of warnings for the header chip: orphan packages, duplicates and
/// skipped entries (04t orphans only when every version is loaded).
pub fn warning_count(p: &Project, packages: Option<&[Value]>, versions: &VersionScope) -> usize {
    let mut n = p.duplicates().len() + p.skipped.len();
    if let Some(rows) = packages {
        n += p.orphans(AliasKind::Package, ids(rows, "Id")).len();
    }
    if let VersionScope::All(rows) = versions {
        n += p
            .orphans(AliasKind::Version, ids(rows, "SubscriberPackageVersionId"))
            .len();
    }
    n
}

fn ids<'a>(rows: &'a [Value], key: &'a str) -> impl Iterator<Item = &'a str> {
    rows.iter().filter_map(move |r| r.get(key)?.as_str())
}

/// The `P` popup: file, counts and every warning.
pub fn report_lines(
    state: &ProjectState,
    hub: Option<&str>,
    packages: Option<&[Value]>,
    versions: &VersionScope,
) -> Vec<String> {
    let p = match state {
        ProjectState::Off => {
            return vec![
                "Project aliases are off (--no-project).".into(),
                String::new(),
                "Start without --no-project inside an SFDX project folder, or pass".into(),
                "--project <folder> to show aliases from its sfdx-project.json.".into(),
            ];
        }
        ProjectState::NotFound(d) => {
            return vec![
                format!(
                    "No sfdx-project.json in {} or its parent folders.",
                    printable(&d.display().to_string())
                ),
                String::new(),
                "Start gp-atlas-tui inside an SFDX project folder, or pass".into(),
                "--project <folder>, to show the project's package aliases.".into(),
            ];
        }
        ProjectState::Failed(e) => {
            return vec![
                format!("Could not read the project: {e}"),
                String::new(),
                "Fix the file (GP Atlas never edits it), then press P again to reload.".into(),
            ];
        }
        ProjectState::Loaded(p) => p,
    };
    let mut l = vec![
        format!("File     {}", printable(&p.file.display().to_string())),
        format!(
            "Aliases  {} packages (0Ho), {} versions (04t)",
            p.count(AliasKind::Package),
            p.count(AliasKind::Version)
        ),
        "Read-only: GP Atlas never edits sfdx-project.json. P reloads it.".into(),
        String::new(),
    ];
    let mut ok = true;

    match packages {
        Some(rows) => {
            let orphans = p.orphans(AliasKind::Package, ids(rows, "Id"));
            if !orphans.is_empty() {
                ok = false;
                l.push(format!(
                    "⚠ Orphan package aliases ({}): the 0Ho is not in {} (another Dev Hub, or deleted)",
                    orphans.len(),
                    hub.unwrap_or("this Dev Hub")
                ));
                l.extend(
                    orphans
                        .iter()
                        .map(|e| format!("    {}  →  {}", e.alias, e.id)),
                );
                l.push(String::new());
            }
            let unaliased: Vec<String> = rows
                .iter()
                .filter(|r| str_of(r, "Id").is_some_and(|id| p.aliases_for(&id).is_empty()))
                .map(|r| {
                    format!(
                        "    {}  {}",
                        str_of(r, "Name").unwrap_or_default(),
                        str_of(r, "Id").unwrap_or_default()
                    )
                })
                .collect();
            if !unaliased.is_empty() {
                ok = false;
                l.push(format!(
                    "⚠ Packages without an alias ({}) (s / S on the 2GP tab copies a snippet)",
                    unaliased.len()
                ));
                l.extend(unaliased);
                l.push(String::new());
            }
        }
        None => l.push("Package checks run once the 2GP package list is loaded.".into()),
    }

    match versions {
        VersionScope::All(rows) => {
            let orphans = p.orphans(AliasKind::Version, ids(rows, "SubscriberPackageVersionId"));
            if !orphans.is_empty() {
                ok = false;
                l.push(format!(
                    "⚠ Orphan version aliases ({}): the 04t is not in {} (another Dev Hub, or deleted)",
                    orphans.len(),
                    hub.unwrap_or("this Dev Hub")
                ));
                l.extend(
                    orphans
                        .iter()
                        .map(|e| format!("    {}  →  {}", e.alias, e.id)),
                );
                l.push(String::new());
            }
            let unaliased = rows
                .iter()
                .filter(|r| {
                    str_of(r, "SubscriberPackageVersionId")
                        .is_some_and(|id| p.aliases_for(&id).is_empty())
                })
                .count();
            if unaliased > 0 {
                l.push(format!(
                    "  {unaliased} of {} versions have no alias (normal for old builds; S copies a snippet)",
                    rows.len()
                ));
                l.push(String::new());
            }
        }
        VersionScope::Partial => l.push(
            "Version (04t) orphan check: select ‹ All packages › with R (released) off.".into(),
        ),
        VersionScope::NotLoaded => {
            l.push("Version (04t) checks run once all versions are loaded.".into())
        }
    }

    let dups = p.duplicates();
    if !dups.is_empty() {
        ok = false;
        l.push(format!("⚠ Several aliases for one ID ({})", dups.len()));
        for (id, aliases) in dups {
            l.push(format!("    {id}  ←  {}", aliases.join(", ")));
        }
        l.push(String::new());
    }
    if !p.skipped.is_empty() {
        ok = false;
        l.push(format!("⚠ Entries GP Atlas ignores ({})", p.skipped.len()));
        for s in &p.skipped {
            l.push(format!("    {}: {}", s.alias, s.reason));
        }
        l.push(String::new());
    }
    if ok {
        l.push("✓ No alias warnings.".into());
    }
    l
}

/// A `packageAliases` line for a package row: its aliases, or its name.
pub fn package_entry(p: Option<&Project>, row: &Value) -> Option<(String, String)> {
    let id = str_of(row, "Id")?;
    let alias = p
        .and_then(|p| p.aliases_for(&id).first().map(|a| (*a).to_owned()))
        .or_else(|| str_of(row, "Name"))?;
    Some((alias, id))
}

/// A `packageAliases` line for a version row: its first alias, or the alias
/// the CLI would write (`<package alias or name>@M.m.p-build[-branch]`).
pub fn version_entry(p: Option<&Project>, row: &Value) -> Option<(String, String)> {
    let id = str_of(row, "SubscriberPackageVersionId")?;
    if let Some(a) = p.and_then(|p| p.aliases_for(&id).first().map(|a| (*a).to_owned())) {
        return Some((a, id));
    }
    let package = str_of(row, "Package2Id")
        .and_then(|pid| p.and_then(|p| p.aliases_for(&pid).first().map(|a| (*a).to_owned())))
        .or_else(|| str_of(row, "Package2Name"))?;
    let version = str_of(row, "Version")?;
    let branch = str_of(row, "Branch");
    Some((version_alias(&package, &version, branch.as_deref()), id))
}

/// `package_entry` or `version_entry`.
pub type EntryFn = fn(Option<&Project>, &Value) -> Option<(String, String)>;

/// Snippet for rows without an alias (all rows when there is no project).
pub fn unaliased_snippet(
    p: Option<&Project>,
    rows: &[&Value],
    id_key: &str,
    entry: EntryFn,
) -> (String, usize) {
    let mut seen = std::collections::BTreeSet::new();
    let entries: Vec<(String, String)> = rows
        .iter()
        .filter(|r| {
            str_of(r, id_key).is_some_and(|id| p.is_none_or(|p| p.aliases_for(&id).is_empty()))
        })
        .filter_map(|r| entry(p, r))
        .filter(|(_, id)| seen.insert(id15(id).to_owned()))
        .collect();
    (snippet(&entries), entries.len())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::json;

    use super::*;

    const PKG: &str = "0Ho000000000001CAA";
    const PKG2: &str = "0Ho000000000002CAA";
    const V1: &str = "04t000000000001AAA";
    const V2: &str = "04t000000000002AAA";

    fn project() -> Project {
        let s = format!(
            r#"{{"packageAliases": {{"MyApp": "{PKG}", "Gone": "0Ho000000000009CAA",
                "MyApp@1.0.0-1": "{V1}", "first": "{V1}"}}}}"#
        );
        Project::parse(Path::new("/w/proj/sfdx-project.json"), s.as_bytes()).unwrap()
    }

    fn pkg_rows() -> Vec<Value> {
        vec![
            json!({"Id": PKG, "Name": "My App"}),
            json!({"Id": PKG2, "Name": "Other"}),
        ]
    }

    fn version_rows() -> Vec<Value> {
        vec![
            json!({"SubscriberPackageVersionId": V1, "Package2Id": PKG, "Package2Name": "My App", "Version": "1.0.0.1", "Alias": ""}),
            json!({"SubscriberPackageVersionId": V2, "Package2Id": PKG, "Package2Name": "My App", "Version": "1.1.0.2", "Branch": "dev", "Alias": "Stale@1"}),
            json!({"SubscriberPackageVersionId": "04t000000000003AAA", "Package2Id": PKG2, "Package2Name": "Other", "Version": "2.0.0.1"}),
        ]
    }

    #[test]
    fn cells() {
        let p = project();
        let v = version_rows();
        let c = alias_cell(&p, &v[0], "SubscriberPackageVersionId");
        assert_eq!(c.text, "MyApp@1.0.0-1 +1");
        assert!(c.warn);
        let c = alias_cell(&p, &v[1], "SubscriberPackageVersionId");
        assert_eq!(c.text, "≠ sf: Stale@1");
        assert!(c.warn);
        let c = alias_cell(&p, &pkg_rows()[0], "Id");
        assert_eq!(c.text, "MyApp");
        assert!(!c.warn);
    }

    #[test]
    fn report_and_warnings() {
        let p = project();
        let pk = pkg_rows();
        let v = version_rows();
        let scope = VersionScope::All(&v);
        // Orphan package "Gone", duplicate on V1; no 04t orphans.
        assert_eq!(warning_count(&p, Some(&pk), &scope), 2);
        let state = ProjectState::Loaded(p);
        let text = report_lines(&state, Some("hub"), Some(&pk), &scope).join("\n");
        assert!(text.contains("Orphan package aliases (1)"), "{text}");
        assert!(text.contains("Gone"));
        assert!(text.contains("Packages without an alias (1)"));
        assert!(text.contains("2 of 3 versions have no alias"));
        assert!(text.contains("Several aliases for one ID (1)"));
        let partial = report_lines(&state, None, None, &VersionScope::Partial).join("\n");
        assert!(partial.contains("All packages"));
        for s in [
            ProjectState::Off,
            ProjectState::NotFound("/x".into()),
            ProjectState::Failed("bad".into()),
        ] {
            assert!(!report_lines(&s, None, None, &VersionScope::NotLoaded).is_empty());
        }
    }

    #[test]
    fn snippets() {
        let p = project();
        let v = version_rows();
        // Uses the package's project alias, like the CLI.
        assert_eq!(
            version_entry(Some(&p), &v[1]),
            Some(("MyApp@1.1.0-2-dev".into(), V2.into()))
        );
        assert_eq!(
            version_entry(None, &v[1]),
            Some(("My App@1.1.0-2-dev".into(), V2.into()))
        );
        assert_eq!(
            version_entry(Some(&p), &v[0]),
            Some(("MyApp@1.0.0-1".into(), V1.into()))
        );
        let refs: Vec<&Value> = v.iter().collect();
        let (s, n) =
            unaliased_snippet(Some(&p), &refs, "SubscriberPackageVersionId", version_entry);
        assert_eq!(n, 2);
        assert!(s.contains("\"MyApp@1.1.0-2-dev\": \"04t000000000002AAA\","));
        assert!(s.contains("\"Other@2.0.0-1\""));
        let pk = pkg_rows();
        assert_eq!(
            package_entry(Some(&p), &pk[1]),
            Some(("Other".into(), PKG2.into()))
        );
    }

    #[test]
    fn explicit_missing_project_is_an_error() {
        let missing = std::env::temp_dir().join("gp-atlas-no-such-project-dir");
        assert!(matches!(
            ProjectState::load(&ProjectSource::Path(missing)),
            ProjectState::Failed(_)
        ));
        assert!(matches!(
            ProjectState::load(&ProjectSource::Off),
            ProjectState::Off
        ));
    }
}
