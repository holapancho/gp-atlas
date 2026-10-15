//! Command Builder (SPEC §9): forms generated from the manifest, values
//! validated (§9.2), and a [`ReadOnlyCommand`] built from them. The copied
//! text is rendered from that command's own argv, so what you copy is exactly
//! what GP Atlas would run.

use std::collections::BTreeMap;

use crate::command::{
    ArgvError, CreateStatus, EdgeDirection, OrderBy, PkgVersionListArgs, ReadOnlyCommand,
    parse_packages,
};
use crate::ids::{
    AncestryTarget, ApiVersion, Branch, DependencyTarget, Id0Ho, Id04t, Id08c, Id033, OrgRef,
    PackageAlias, PackageRef,
};
use crate::manifest::Manifest;
use crate::quote::{QuoteError, Shell};

/// Commands offered in the builder, in menu order (all read-only).
pub const BUILDER_COMMANDS: &[&str] = &[
    "package:list",
    "package:version:list",
    "package:version:report",
    "package:version:displayancestry",
    "package:version:displaydependencies",
    "package:version:create:list",
    "package:version:create:report",
    "package:installed:list",
    "package1:version:list",
    "package1:version:display",
    "org:list",
];

/// Flags each builder command supports (the subset [`ReadOnlyCommand`] can
/// express). Other manifest flags are listed as "not offered".
fn supported(id: &str) -> &'static [&'static str] {
    match id {
        "package:list" => &["target-dev-hub", "verbose", "api-version"],
        "package:version:list" => &[
            "target-dev-hub",
            "packages",
            "released",
            "branch",
            "created-last-days",
            "modified-last-days",
            "order-by",
            "concise",
            "verbose",
            "show-conversions-only",
            "api-version",
        ],
        "package:version:report" => &["target-dev-hub", "package", "verbose"],
        "package:version:displayancestry" => &["target-dev-hub", "package", "dot-code", "verbose"],
        "package:version:displaydependencies" => {
            &["target-dev-hub", "package", "edge-direction", "verbose"]
        }
        "package:version:create:list" => &[
            "target-dev-hub",
            "created-last-days",
            "status",
            "show-conversions-only",
            "verbose",
        ],
        "package:version:create:report" => &["target-dev-hub", "package-create-request-id"],
        "package:installed:list" => &["target-org"],
        "package1:version:list" => &["target-org", "package-id"],
        "package1:version:display" => &["target-org", "package-version-id"],
        "org:list" => &["skip-connection-status", "all"],
        _ => &[],
    }
}

/// Flags every command has that the builder controls itself.
const MANAGED: &[&str] = &["json", "flags-dir", "loglevel"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldKind {
    /// Dev Hub (alias or username).
    Hub,
    /// Target org (alias or username).
    Org,
    Bool,
    /// One of the manifest's options.
    Choice(Vec<String>),
    /// Integer ≥ 0.
    Days,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub flag: String,
    pub short: Option<char>,
    pub kind: FieldKind,
    pub required: bool,
    /// What a valid value looks like.
    pub hint: &'static str,
}

/// The form for one command: manifest flags in manifest order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    pub id: String,
    pub fields: Vec<Field>,
    /// Manifest flags the builder does not offer (yet).
    pub not_offered: Vec<String>,
}

fn hint(id: &str, flag: &str) -> &'static str {
    match (id, flag) {
        (_, "target-dev-hub") => "Dev Hub alias or username",
        (_, "target-org") => "org alias or username",
        (_, "packages") => "0Ho ids or project aliases, comma-separated",
        ("package:version:report", "package") => "04t id or project alias",
        ("package:version:displayancestry", "package") => "0Ho, 04t or project alias",
        ("package:version:displaydependencies", "package") => "04t, 08c or project alias",
        (_, "branch") => "letters, digits . _ - / (no spaces or quotes)",
        (_, "order-by") => "e.g. CreatedDate DESC,Name (allowed fields only)",
        (_, "api-version") => "e.g. 62.0",
        (_, "package-create-request-id") => "08c id (15 or 18 characters)",
        (_, "package-id") => "033 id (18 characters)",
        (_, "package-version-id") => "04t id (15 or 18 characters)",
        (_, f) if f.ends_with("-days") => "whole number of days, 0 or more",
        _ => "",
    }
}

/// The form of a builder command, generated from the manifest (AC-31).
pub fn form(manifest: &Manifest, id: &str) -> Result<Form, ArgvError> {
    let spec = manifest
        .command(id)
        .ok_or_else(|| ArgvError::UnknownCommand(id.to_owned()))?;
    let ok = supported(id);
    let mut fields = Vec::new();
    let mut not_offered = Vec::new();
    for f in &spec.flags {
        if f.hidden || f.deprecated || MANAGED.contains(&f.name.as_str()) {
            continue;
        }
        if !ok.contains(&f.name.as_str()) {
            not_offered.push(f.name.clone());
            continue;
        }
        let kind = match (f.name.as_str(), f.kind.as_str(), &f.options) {
            ("target-dev-hub", ..) => FieldKind::Hub,
            ("target-org", ..) => FieldKind::Org,
            (_, "boolean", _) => FieldKind::Bool,
            (_, _, Some(opts)) => FieldKind::Choice(opts.clone()),
            (n, ..) if n.ends_with("-days") => FieldKind::Days,
            _ => FieldKind::Text,
        };
        fields.push(Field {
            flag: f.name.clone(),
            short: f.char.as_deref().and_then(|c| c.chars().next()),
            kind,
            // The CLI defaults the Dev Hub/org from its config; GP Atlas
            // always passes it explicitly.
            required: f.required || matches!(f.name.as_str(), "target-dev-hub" | "target-org"),
            hint: hint(id, &f.name),
        });
    }
    // Dev Hub / org first, then other required fields, then manifest order.
    fields.sort_by_key(|f| {
        (
            !matches!(f.kind, FieldKind::Hub | FieldKind::Org),
            !f.required,
        )
    });
    Ok(Form {
        id: id.to_owned(),
        fields,
        not_offered,
    })
}

/// A field that failed validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldError {
    pub flag: String,
    pub message: String,
}

/// Form values by flag name. Booleans are `"true"` when on.
pub type Values = BTreeMap<String, String>;

struct Reader<'a> {
    values: &'a Values,
    errors: Vec<FieldError>,
}

impl Reader<'_> {
    fn text(&self, flag: &str) -> Option<&str> {
        self.values
            .get(flag)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
    }

    fn on(&self, flag: &str) -> bool {
        self.text(flag) == Some("true")
    }

    fn fail(&mut self, flag: &str, message: impl Into<String>) {
        self.errors.push(FieldError {
            flag: flag.to_owned(),
            message: message.into(),
        });
    }

    fn required<T, E: std::fmt::Display>(
        &mut self,
        flag: &str,
        parse: impl FnOnce(&str) -> Result<T, E>,
    ) -> Option<T> {
        match self.text(flag) {
            None => {
                self.fail(flag, "required");
                None
            }
            Some(v) => match parse(v) {
                Ok(t) => Some(t),
                Err(e) => {
                    self.fail(flag, e.to_string());
                    None
                }
            },
        }
    }

    /// `Ok(None)` when empty; `Err(())` (already recorded) when invalid.
    fn optional<T, E: std::fmt::Display>(
        &mut self,
        flag: &str,
        parse: impl FnOnce(&str) -> Result<T, E>,
    ) -> Result<Option<T>, ()> {
        match self.text(flag) {
            None => Ok(None),
            Some(v) => match parse(v) {
                Ok(t) => Ok(Some(t)),
                Err(e) => {
                    self.fail(flag, e.to_string());
                    Err(())
                }
            },
        }
    }

    fn org(&mut self, flag: &str) -> Option<OrgRef> {
        self.required(flag, OrgRef::new)
    }

    fn days(&mut self, flag: &str) -> Result<Option<u32>, ()> {
        self.optional(flag, |v| {
            v.parse::<u32>()
                .map_err(|_| "a whole number of days, 0 or more")
        })
    }
}

fn package_ref<I>(
    v: &str,
    id: impl Fn(&str) -> Result<I, crate::ids::ValidationError>,
) -> Result<PackageRef<I>, crate::ids::ValidationError> {
    match id(v) {
        Ok(i) => Ok(PackageRef::Id(i)),
        Err(e) if looks_like_id(v) => Err(e),
        Err(_) => PackageAlias::new(v).map(PackageRef::Alias),
    }
}

/// `0Ho…`/`04t…`-shaped input: report the id error rather than "bad alias".
fn looks_like_id(v: &str) -> bool {
    v.len() >= 15 && v.len() <= 18 && v.chars().all(|c| c.is_ascii_alphanumeric())
}

fn ancestry_target(v: &str) -> Result<AncestryTarget, String> {
    if let Ok(i) = Id0Ho::new(v) {
        return Ok(AncestryTarget::Package(i));
    }
    if let Ok(i) = Id04t::new(v) {
        return Ok(AncestryTarget::Version(i));
    }
    if looks_like_id(v) {
        return Err("expected a 0Ho or 04t id (15 or 18 characters)".into());
    }
    PackageAlias::new(v)
        .map(AncestryTarget::Alias)
        .map_err(|e| e.to_string())
}

fn dependency_target(v: &str) -> Result<DependencyTarget, String> {
    if let Ok(i) = Id04t::new(v) {
        return Ok(DependencyTarget::Version(i));
    }
    if let Ok(i) = Id08c::new(v) {
        return Ok(DependencyTarget::Request(i));
    }
    if looks_like_id(v) {
        return Err("expected a 04t or 08c id (15 or 18 characters)".into());
    }
    PackageAlias::new(v)
        .map(DependencyTarget::Alias)
        .map_err(|e| e.to_string())
}

fn order_by(v: &str) -> Result<Vec<OrderBy>, String> {
    v.split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| OrderBy::parse(t).map_err(|e| e.to_string()))
        .collect()
}

fn choice<T: Copy>(v: &str, all: &[T], name: impl Fn(T) -> &'static str) -> Result<T, String> {
    all.iter()
        .copied()
        .find(|t| name(*t) == v)
        .ok_or_else(|| format!("not one of the allowed values: {v:?}"))
}

/// Builds the command from form values, validating every field (§9.2).
pub fn build(id: &str, values: &Values) -> Result<ReadOnlyCommand, Vec<FieldError>> {
    let mut r = Reader {
        values,
        errors: Vec::new(),
    };
    let cmd = match id {
        "package:list" => {
            let hub = r.org("target-dev-hub");
            let api = r.optional("api-version", ApiVersion::new);
            hub.zip(api.ok())
                .map(|(hub, api_version)| ReadOnlyCommand::PkgList {
                    hub,
                    verbose: r.on("verbose"),
                    api_version,
                })
        }
        "package:version:list" => {
            let hub = r.org("target-dev-hub");
            let packages = r.optional("packages", parse_packages);
            let branch = r.optional("branch", Branch::new);
            let created = r.days("created-last-days");
            let modified = r.days("modified-last-days");
            let order = r.optional("order-by", order_by);
            let api = r.optional("api-version", ApiVersion::new);
            match (hub, packages, branch, created, modified, order, api) {
                (Some(hub), Ok(p), Ok(b), Ok(c), Ok(m), Ok(o), Ok(a)) => {
                    let mut args = PkgVersionListArgs::new(hub);
                    args.packages = p.unwrap_or_default();
                    args.released = r.on("released");
                    args.branch = b;
                    args.created_last_days = c;
                    args.modified_last_days = m;
                    args.order_by = o.unwrap_or_default();
                    args.concise = r.on("concise");
                    args.verbose = r.on("verbose");
                    args.show_conversions_only = r.on("show-conversions-only");
                    args.api_version = a;
                    Some(ReadOnlyCommand::PkgVersionList(args))
                }
                _ => None,
            }
        }
        "package:version:report" => {
            let hub = r.org("target-dev-hub");
            let package = r.required("package", |v| package_ref(v, Id04t::new));
            hub.zip(package)
                .map(|(hub, package)| ReadOnlyCommand::PkgVersionReport {
                    hub,
                    package,
                    verbose: r.on("verbose"),
                })
        }
        "package:version:displayancestry" => {
            let hub = r.org("target-dev-hub");
            let package = r.required("package", ancestry_target);
            hub.zip(package)
                .map(|(hub, package)| ReadOnlyCommand::PkgVersionAncestry {
                    hub,
                    package,
                    dot_code: r.on("dot-code"),
                    verbose: r.on("verbose"),
                })
        }
        "package:version:displaydependencies" => {
            let hub = r.org("target-dev-hub");
            let package = r.required("package", dependency_target);
            let edge = r.optional("edge-direction", |v| {
                choice(
                    v,
                    &[EdgeDirection::RootFirst, EdgeDirection::RootLast],
                    EdgeDirection::as_str,
                )
            });
            match (hub, package, edge) {
                (Some(hub), Some(package), Ok(edge_direction)) => {
                    Some(ReadOnlyCommand::PkgVersionDeps {
                        hub,
                        package,
                        edge_direction,
                        verbose: r.on("verbose"),
                    })
                }
                _ => None,
            }
        }
        "package:version:create:list" => {
            let hub = r.org("target-dev-hub");
            let days = r.days("created-last-days");
            let status = r.optional("status", |v| {
                choice(
                    v,
                    &[
                        CreateStatus::Queued,
                        CreateStatus::InProgress,
                        CreateStatus::Success,
                        CreateStatus::Error,
                    ],
                    CreateStatus::as_str,
                )
            });
            match (hub, days, status) {
                (Some(hub), Ok(created_last_days), Ok(status)) => {
                    Some(ReadOnlyCommand::PkgCreateList {
                        hub,
                        created_last_days,
                        status,
                        show_conversions_only: r.on("show-conversions-only"),
                        verbose: r.on("verbose"),
                    })
                }
                _ => None,
            }
        }
        "package:version:create:report" => {
            let hub = r.org("target-dev-hub");
            let request = r.required("package-create-request-id", Id08c::new);
            hub.zip(request)
                .map(|(hub, request)| ReadOnlyCommand::PkgCreateReport { hub, request })
        }
        "package:installed:list" => r
            .org("target-org")
            .map(|org| ReadOnlyCommand::PkgInstalledList { org }),
        "package1:version:list" => {
            let org = r.org("target-org");
            let pkg = r.optional("package-id", Id033::new);
            match (org, pkg) {
                (Some(org), Ok(package_id)) => {
                    Some(ReadOnlyCommand::Pkg1VersionList { org, package_id })
                }
                _ => None,
            }
        }
        "package1:version:display" => {
            let org = r.org("target-org");
            let version = r.required("package-version-id", Id04t::new);
            org.zip(version)
                .map(|(org, version)| ReadOnlyCommand::Pkg1VersionDisplay { org, version })
        }
        "org:list" => Some(ReadOnlyCommand::OrgList {
            skip_connection_status: r.on("skip-connection-status"),
            all: r.on("all"),
        }),
        other => {
            r.fail("", format!("`{other}` is not offered in the builder"));
            None
        }
    };
    match cmd {
        Some(c) if r.errors.is_empty() => Ok(c),
        _ => Err(r.errors),
    }
}

/// How a copied command is printed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RenderOptions {
    pub shell: Shell,
    /// Keep `--json` (default off for copy, SPEC §9.1).
    pub json: bool,
    /// Use one-letter flags where the manifest has them.
    pub short_flags: bool,
    /// One flag per line with the shell's continuation character.
    pub multiline: bool,
    /// Prefix `cd <dir> &&` (needed when a project alias is used, §6.4).
    pub cd: Option<String>,
}

/// The command a human copies: from the command's own argv (so it matches
/// what GP Atlas would run), quoted for the chosen shell.
pub fn render(
    cmd: &ReadOnlyCommand,
    manifest: &Manifest,
    opts: &RenderOptions,
) -> Result<String, RenderError> {
    let argv = cmd.argv(manifest)?;
    let spec = manifest.command(cmd.id());
    let mut words = vec!["sf".to_owned()];
    let mut lines: Vec<Vec<String>> = Vec::new();
    for a in argv {
        if a == "--json" && !opts.json {
            continue;
        }
        let word = match a.strip_prefix("--") {
            Some(name) if opts.short_flags => spec
                .and_then(|c| c.flag(name))
                .and_then(|f| f.char.clone())
                .map_or(a.clone(), |c| format!("-{c}")),
            _ => a.clone(),
        };
        let quoted = opts.shell.quote(&word)?;
        if word.starts_with('-') && opts.multiline {
            lines.push(std::mem::take(&mut words));
        }
        words.push(quoted);
    }
    lines.push(words);
    let lines: Vec<String> = lines
        .into_iter()
        .filter(|l| !l.is_empty())
        .map(|l| l.join(" "))
        .collect();
    let body = if opts.multiline {
        lines.join(&format!("{}\n  ", opts.shell.continuation()))
    } else {
        lines.join(" ")
    };
    Ok(match &opts.cd {
        Some(dir) => format!("{}{body}", opts.shell.cd_prefix(dir)?),
        None => body,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RenderError {
    #[error(transparent)]
    Argv(#[from] ArgvError),
    #[error(transparent)]
    Quote(#[from] QuoteError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocklist::is_blocked_command;

    fn m() -> Manifest {
        Manifest::embedded().unwrap()
    }

    fn vals(pairs: &[(&str, &str)]) -> Values {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    /// AC-31: every builder command has a form, from the manifest only.
    #[test]
    fn forms_come_from_the_manifest() {
        let m = m();
        for id in BUILDER_COMMANDS {
            assert!(!is_blocked_command(id), "{id}");
            let f = form(&m, id).unwrap();
            let spec = m.command(id).unwrap();
            for field in &f.fields {
                let flag = spec.flag(&field.flag).expect("field is a manifest flag");
                assert!(!flag.deprecated && !flag.hidden);
                assert_eq!(
                    field.short,
                    flag.char.as_deref().and_then(|c| c.chars().next())
                );
            }
            // Every supported flag exists in the manifest (no drift).
            for s in supported(id) {
                assert!(spec.flag(s).is_some(), "{id} --{s}");
            }
        }
        let f = form(&m, "package:version:list").unwrap();
        assert_eq!(f.fields[0].flag, "target-dev-hub");
        assert!(f.fields[0].required);
        assert!(
            f.fields
                .iter()
                .any(|x| x.flag == "released" && x.kind == FieldKind::Bool)
        );
        let deps = form(&m, "package:version:displaydependencies").unwrap();
        assert!(deps.not_offered.contains(&"installation-key".to_owned()));
        assert!(deps.fields.iter().any(|x| x.flag == "edge-direction"
            && x.kind == FieldKind::Choice(vec!["root-first".into(), "root-last".into()])));
        // org list --clean (blocklisted) is never offered.
        let ol = form(&m, "org:list").unwrap();
        assert!(!ol.fields.iter().any(|x| x.flag == "clean"));
    }

    #[test]
    fn builds_and_validates() {
        let c = build(
            "package:version:list",
            &vals(&[
                ("target-dev-hub", "my-hub"),
                ("packages", "0Ho000000000001CAA, My App"),
                ("released", "true"),
                ("order-by", "CreatedDate DESC,Name"),
                ("created-last-days", "7"),
            ]),
        )
        .unwrap();
        assert_eq!(
            c.display(&m()),
            "sf package version list --target-dev-hub my-hub --packages 0Ho000000000001CAA,My App \
             --released --created-last-days 7 --order-by CreatedDate DESC,Name --json"
        );
        assert!(c.uses_project_alias());

        let e = build(
            "package:version:list",
            &vals(&[
                ("branch", "my branch"),
                ("order-by", "Id; DROP"),
                ("created-last-days", "-1"),
            ]),
        )
        .unwrap_err();
        let flags: Vec<&str> = e.iter().map(|x| x.flag.as_str()).collect();
        assert_eq!(
            flags,
            ["target-dev-hub", "branch", "created-last-days", "order-by"]
        );

        // An id-shaped value with a wrong prefix is an id error, not an alias.
        let e = build(
            "package:version:report",
            &vals(&[("target-dev-hub", "h"), ("package", "0Ho000000000001CAA")]),
        )
        .unwrap_err();
        assert_eq!(e[0].flag, "package");
        assert!(
            build(
                "package:version:displayancestry",
                &vals(&[("target-dev-hub", "h"), ("package", "0Ho000000000001CAA")])
            )
            .is_ok()
        );
        assert!(
            build(
                "package:version:displaydependencies",
                &vals(&[
                    ("target-dev-hub", "h"),
                    ("package", "08c000000000001AAA"),
                    ("edge-direction", "sideways")
                ])
            )
            .is_err()
        );
        assert!(
            build(
                "package1:version:list",
                &vals(&[("target-org", "o"), ("package-id", "033000000000001")])
            )
            .is_err()
        );
        assert!(build("org:list", &vals(&[])).is_ok());
        assert!(build("org:logout", &vals(&[])).is_err());
    }

    #[test]
    fn renders_per_shell() {
        let m = m();
        let c = build(
            "package:version:list",
            &vals(&[
                ("target-dev-hub", "dev@example.com"),
                ("packages", "My App"),
                ("order-by", "CreatedDate DESC"),
            ]),
        )
        .unwrap();
        let r = |o: RenderOptions| render(&c, &m, &o).unwrap();
        assert_eq!(
            r(RenderOptions::default()),
            "sf package version list --target-dev-hub dev@example.com --packages 'My App' --order-by 'CreatedDate DESC'"
        );
        assert_eq!(
            r(RenderOptions {
                shell: Shell::PowerShell,
                ..Default::default()
            }),
            "sf package version list --target-dev-hub 'dev@example.com' --packages 'My App' --order-by 'CreatedDate DESC'"
        );
        assert_eq!(
            r(RenderOptions {
                shell: Shell::Cmd,
                json: true,
                short_flags: true,
                ..Default::default()
            }),
            "sf package version list -v dev@example.com -p \"My App\" -o \"CreatedDate DESC\" --json"
        );
        assert_eq!(
            r(RenderOptions {
                multiline: true,
                cd: Some("/w/proj".into()),
                ..Default::default()
            }),
            "cd /w/proj && sf package version list \\\n  --target-dev-hub dev@example.com \\\n  --packages 'My App' \\\n  --order-by 'CreatedDate DESC'"
        );
    }
}
