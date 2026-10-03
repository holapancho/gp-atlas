//! The closed set of commands GP Atlas may run (SPEC §1 rule 1, §4.4, §7.2).
//!
//! [`ReadOnlyCommand::argv`] is the only place argv is built. Every flag is
//! checked against the manifest (canonical long names only) and every value
//! is a validated type from [`crate::ids`].

use crate::blocklist::{is_blocked_command, is_blocked_flag};
use crate::ids::{
    AncestryTarget, ApiVersion, Branch, DependencyTarget, Id0Ho, Id04t, Id08c, Id033, OrgRef,
    PackageAlias, PackageRef,
};
use crate::manifest::Manifest;

/// Why argv could not be built.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ArgvError {
    #[error("command `{0}` is not in the manifest")]
    UnknownCommand(String),
    #[error("command `{command}`: flag `--{flag}` is not in the manifest (contract drift?)")]
    UnknownFlag { command: String, flag: String },
    #[error("command `{0}` is blocklisted")]
    Blocked(String),
    #[error("command `{command}`: flag `--{flag}` is blocklisted")]
    BlockedFlag { command: String, flag: String },
    #[error("command `{command}`: value {value:?} not allowed for `--{flag}`")]
    BadOption {
        command: String,
        flag: String,
        value: String,
    },
    #[error("{0}")]
    Invalid(String),
}

/// `package version create list --status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateStatus {
    Queued,
    InProgress,
    Success,
    Error,
}

impl CreateStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::InProgress => "InProgress",
            Self::Success => "Success",
            Self::Error => "Error",
        }
    }
}

/// `displaydependencies --edge-direction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeDirection {
    RootFirst,
    RootLast,
}

impl EdgeDirection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RootFirst => "root-first",
            Self::RootLast => "root-last",
        }
    }
}

/// Fields allowed in `--order-by` (SPEC §4.4; raw SOQL, F14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderField {
    CreatedDate,
    LastModifiedDate,
    MajorVersion,
    MinorVersion,
    PatchVersion,
    BuildNumber,
    Package2Id,
    Branch,
    Name,
    IsReleased,
}

impl OrderField {
    pub const ALL: [Self; 10] = [
        Self::CreatedDate,
        Self::LastModifiedDate,
        Self::MajorVersion,
        Self::MinorVersion,
        Self::PatchVersion,
        Self::BuildNumber,
        Self::Package2Id,
        Self::Branch,
        Self::Name,
        Self::IsReleased,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::CreatedDate => "CreatedDate",
            Self::LastModifiedDate => "LastModifiedDate",
            Self::MajorVersion => "MajorVersion",
            Self::MinorVersion => "MinorVersion",
            Self::PatchVersion => "PatchVersion",
            Self::BuildNumber => "BuildNumber",
            Self::Package2Id => "Package2Id",
            Self::Branch => "Branch",
            Self::Name => "Name",
            Self::IsReleased => "IsReleased",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.as_str() == s)
    }
}

/// One `--order-by` term.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderBy {
    pub field: OrderField,
    pub descending: Option<bool>,
}

impl OrderBy {
    /// Parses `Field`, `Field ASC` or `Field DESC`.
    pub fn parse(term: &str) -> Result<Self, ArgvError> {
        let mut parts = term.split_whitespace();
        let field = parts
            .next()
            .and_then(OrderField::parse)
            .ok_or_else(|| ArgvError::Invalid(format!("order-by field not allowed: {term:?}")))?;
        let descending = match parts.next() {
            None => None,
            Some(d) if d.eq_ignore_ascii_case("ASC") => Some(false),
            Some(d) if d.eq_ignore_ascii_case("DESC") => Some(true),
            Some(_) => return Err(ArgvError::Invalid(format!("bad order-by term {term:?}"))),
        };
        if parts.next().is_some() {
            return Err(ArgvError::Invalid(format!("bad order-by term {term:?}")));
        }
        Ok(Self { field, descending })
    }

    fn render(self) -> String {
        match self.descending {
            None => self.field.as_str().to_owned(),
            Some(false) => format!("{} ASC", self.field.as_str()),
            Some(true) => format!("{} DESC", self.field.as_str()),
        }
    }
}

/// Arguments of `package version list`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PkgVersionListArgs {
    pub hub: OrgRef,
    pub packages: Vec<PackageRef<Id0Ho>>,
    pub released: bool,
    pub branch: Option<Branch>,
    pub created_last_days: Option<u32>,
    pub modified_last_days: Option<u32>,
    pub order_by: Vec<OrderBy>,
    pub concise: bool,
    pub verbose: bool,
    pub show_conversions_only: bool,
    pub api_version: Option<ApiVersion>,
}

impl PkgVersionListArgs {
    pub fn new(hub: OrgRef) -> Self {
        Self {
            hub,
            packages: Vec::new(),
            released: false,
            branch: None,
            created_last_days: None,
            modified_last_days: None,
            order_by: Vec::new(),
            concise: false,
            verbose: false,
            show_conversions_only: false,
            api_version: None,
        }
    }
}

/// Every command GP Atlas can run. All are read-only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadOnlyCommand {
    Version,
    Plugins,
    Commands,
    OrgList {
        skip_connection_status: bool,
        all: bool,
    },
    AliasList,
    /// `config get target-dev-hub target-org` (inventory I3, display only).
    ConfigGet,
    PkgList {
        hub: OrgRef,
        verbose: bool,
        api_version: Option<ApiVersion>,
    },
    PkgVersionList(PkgVersionListArgs),
    PkgVersionReport {
        hub: OrgRef,
        package: PackageRef<Id04t>,
        verbose: bool,
    },
    PkgVersionAncestry {
        hub: OrgRef,
        package: AncestryTarget,
        dot_code: bool,
        verbose: bool,
    },
    PkgVersionDeps {
        hub: OrgRef,
        package: DependencyTarget,
        edge_direction: Option<EdgeDirection>,
        verbose: bool,
    },
    PkgCreateList {
        hub: OrgRef,
        created_last_days: Option<u32>,
        status: Option<CreateStatus>,
        show_conversions_only: bool,
        verbose: bool,
    },
    PkgCreateReport {
        hub: OrgRef,
        request: Id08c,
    },
    PkgInstalledList {
        org: OrgRef,
    },
    Pkg1VersionList {
        org: OrgRef,
        package_id: Option<Id033>,
    },
    Pkg1VersionDisplay {
        org: OrgRef,
        version: Id04t,
    },
}

/// Accumulates argv for one command, checking each flag against the manifest.
struct Builder<'m> {
    manifest: &'m Manifest,
    id: &'static str,
    argv: Vec<String>,
}

impl<'m> Builder<'m> {
    fn new(manifest: &'m Manifest, id: &'static str) -> Result<Self, ArgvError> {
        if is_blocked_command(id) {
            return Err(ArgvError::Blocked(id.to_owned()));
        }
        if manifest.command(id).is_none() {
            return Err(ArgvError::UnknownCommand(id.to_owned()));
        }
        Ok(Self {
            manifest,
            id,
            argv: id.split(':').map(str::to_owned).collect(),
        })
    }

    fn check(&self, flag: &str) -> Result<(), ArgvError> {
        if is_blocked_flag(self.id, flag) {
            return Err(ArgvError::BlockedFlag {
                command: self.id.to_owned(),
                flag: flag.to_owned(),
            });
        }
        let spec = self
            .manifest
            .command(self.id)
            .and_then(|c| c.flag(flag))
            .ok_or_else(|| ArgvError::UnknownFlag {
                command: self.id.to_owned(),
                flag: flag.to_owned(),
            })?;
        if spec.deprecated {
            return Err(ArgvError::Invalid(format!("flag --{flag} is deprecated")));
        }
        Ok(())
    }

    fn switch(&mut self, flag: &str, on: bool) -> Result<&mut Self, ArgvError> {
        if on {
            self.check(flag)?;
            self.argv.push(format!("--{flag}"));
        }
        Ok(self)
    }

    fn value(&mut self, flag: &str, value: impl Into<String>) -> Result<&mut Self, ArgvError> {
        self.check(flag)?;
        let value = value.into();
        if let Some(options) = self
            .manifest
            .command(self.id)
            .and_then(|c| c.flag(flag))
            .and_then(|f| f.options.as_ref())
            && !options.contains(&value)
        {
            return Err(ArgvError::BadOption {
                command: self.id.to_owned(),
                flag: flag.to_owned(),
                value,
            });
        }
        self.argv.push(format!("--{flag}"));
        self.argv.push(value);
        Ok(self)
    }

    fn opt(
        &mut self,
        flag: &str,
        value: Option<impl Into<String>>,
    ) -> Result<&mut Self, ArgvError> {
        match value {
            Some(v) => self.value(flag, v),
            None => Ok(self),
        }
    }

    fn positional(&mut self, value: &str) -> &mut Self {
        self.argv.push(value.to_owned());
        self
    }

    fn finish(mut self) -> Result<Vec<String>, ArgvError> {
        self.check("json")?;
        self.argv.push("--json".to_owned());
        Ok(self.argv)
    }
}

impl ReadOnlyCommand {
    /// The oclif command id (`package:version:list`).
    pub fn id(&self) -> &'static str {
        match self {
            Self::Version => "version",
            Self::Plugins => "plugins",
            Self::Commands => "commands",
            Self::OrgList { .. } => "org:list",
            Self::AliasList => "alias:list",
            Self::ConfigGet => "config:get",
            Self::PkgList { .. } => "package:list",
            Self::PkgVersionList(_) => "package:version:list",
            Self::PkgVersionReport { .. } => "package:version:report",
            Self::PkgVersionAncestry { .. } => "package:version:displayancestry",
            Self::PkgVersionDeps { .. } => "package:version:displaydependencies",
            Self::PkgCreateList { .. } => "package:version:create:list",
            Self::PkgCreateReport { .. } => "package:version:create:report",
            Self::PkgInstalledList { .. } => "package:installed:list",
            Self::Pkg1VersionList { .. } => "package1:version:list",
            Self::Pkg1VersionDisplay { .. } => "package1:version:display",
        }
    }

    /// The org this command targets, if any.
    pub fn target(&self) -> Option<&OrgRef> {
        match self {
            Self::PkgList { hub, .. }
            | Self::PkgVersionReport { hub, .. }
            | Self::PkgVersionAncestry { hub, .. }
            | Self::PkgVersionDeps { hub, .. }
            | Self::PkgCreateList { hub, .. }
            | Self::PkgCreateReport { hub, .. } => Some(hub),
            Self::PkgVersionList(a) => Some(&a.hub),
            Self::PkgInstalledList { org }
            | Self::Pkg1VersionList { org, .. }
            | Self::Pkg1VersionDisplay { org, .. } => Some(org),
            _ => None,
        }
    }

    /// Whether running this command requires the project directory as cwd (§6.4).
    pub fn uses_project_alias(&self) -> bool {
        match self {
            Self::PkgVersionList(a) => a.packages.iter().any(|p| matches!(p, PackageRef::Alias(_))),
            Self::PkgVersionReport { package, .. } => matches!(package, PackageRef::Alias(_)),
            Self::PkgVersionAncestry { package, .. } => matches!(package, AncestryTarget::Alias(_)),
            Self::PkgVersionDeps { package, .. } => matches!(package, DependencyTarget::Alias(_)),
            _ => false,
        }
    }

    /// Builds argv (without the binary). Always ends with `--json`.
    pub fn argv(&self, manifest: &Manifest) -> Result<Vec<String>, ArgvError> {
        let mut b = Builder::new(manifest, self.id())?;
        match self {
            Self::Version | Self::Plugins | Self::Commands | Self::AliasList => {}
            Self::OrgList {
                skip_connection_status,
                all,
            } => {
                b.switch("skip-connection-status", *skip_connection_status)?
                    .switch("all", *all)?;
            }
            Self::ConfigGet => {
                b.positional("target-dev-hub").positional("target-org");
            }
            Self::PkgList {
                hub,
                verbose,
                api_version,
            } => {
                b.value("target-dev-hub", hub.as_str())?
                    .switch("verbose", *verbose)?
                    .opt("api-version", api_version.as_ref().map(|v| v.as_str()))?;
            }
            Self::PkgVersionList(a) => {
                b.value("target-dev-hub", a.hub.as_str())?;
                if !a.packages.is_empty() {
                    let csv: Vec<String> = a.packages.iter().map(ToString::to_string).collect();
                    b.value("packages", csv.join(","))?;
                }
                b.switch("released", a.released)?
                    .opt("branch", a.branch.as_ref().map(|v| v.as_str()))?
                    .opt(
                        "created-last-days",
                        a.created_last_days.map(|d| d.to_string()),
                    )?
                    .opt(
                        "modified-last-days",
                        a.modified_last_days.map(|d| d.to_string()),
                    )?;
                if !a.order_by.is_empty() {
                    let terms: Vec<String> = a.order_by.iter().map(|o| o.render()).collect();
                    b.value("order-by", terms.join(","))?;
                }
                b.switch("concise", a.concise)?
                    .switch("verbose", a.verbose)?
                    .switch("show-conversions-only", a.show_conversions_only)?
                    .opt("api-version", a.api_version.as_ref().map(|v| v.as_str()))?;
            }
            Self::PkgVersionReport {
                hub,
                package,
                verbose,
            } => {
                b.value("target-dev-hub", hub.as_str())?
                    .value("package", package.to_string())?
                    .switch("verbose", *verbose)?;
            }
            Self::PkgVersionAncestry {
                hub,
                package,
                dot_code,
                verbose,
            } => {
                b.value("target-dev-hub", hub.as_str())?
                    .value("package", package.to_string())?
                    .switch("dot-code", *dot_code)?
                    .switch("verbose", *verbose)?;
            }
            Self::PkgVersionDeps {
                hub,
                package,
                edge_direction,
                verbose,
            } => {
                b.value("target-dev-hub", hub.as_str())?
                    .value("package", package.to_string())?
                    .opt("edge-direction", edge_direction.map(EdgeDirection::as_str))?
                    .switch("verbose", *verbose)?;
            }
            Self::PkgCreateList {
                hub,
                created_last_days,
                status,
                show_conversions_only,
                verbose,
            } => {
                b.value("target-dev-hub", hub.as_str())?
                    .opt(
                        "created-last-days",
                        created_last_days.map(|d| d.to_string()),
                    )?
                    .opt("status", status.map(CreateStatus::as_str))?
                    .switch("show-conversions-only", *show_conversions_only)?
                    .switch("verbose", *verbose)?;
            }
            Self::PkgCreateReport { hub, request } => {
                b.value("target-dev-hub", hub.as_str())?
                    .value("package-create-request-id", request.as_str())?;
            }
            Self::PkgInstalledList { org } => {
                b.value("target-org", org.as_str())?;
            }
            Self::Pkg1VersionList { org, package_id } => {
                b.value("target-org", org.as_str())?
                    .opt("package-id", package_id.as_ref().map(|v| v.as_str()))?;
            }
            Self::Pkg1VersionDisplay { org, version } => {
                b.value("target-org", org.as_str())?
                    .value("package-version-id", version.as_str())?;
            }
        }
        b.finish()
    }

    /// Human-readable command line (for display only; never executed).
    pub fn display(&self, manifest: &Manifest) -> String {
        match self.argv(manifest) {
            Ok(argv) => format!("sf {}", argv.join(" ")),
            Err(e) => format!("<invalid: {e}>"),
        }
    }
}

/// Parses a comma-separated `--packages` value of 0Ho ids or aliases.
pub fn parse_packages(csv: &str) -> Result<Vec<PackageRef<Id0Ho>>, crate::ids::ValidationError> {
    csv.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| match Id0Ho::new(s) {
            Ok(id) => Ok(PackageRef::Id(id)),
            Err(_) => PackageAlias::new(s).map(PackageRef::Alias),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocklist::is_blocked_command;

    fn m() -> Manifest {
        Manifest::embedded().unwrap()
    }

    fn hub() -> OrgRef {
        OrgRef::new("my-hub").unwrap()
    }

    fn all_variants() -> Vec<ReadOnlyCommand> {
        let mut vl = PkgVersionListArgs::new(hub());
        vl.packages = parse_packages("0Ho000000000001AAA,My Pkg").unwrap();
        vl.released = true;
        vl.branch = Some(Branch::new("main").unwrap());
        vl.created_last_days = Some(0);
        vl.modified_last_days = Some(3);
        vl.order_by = vec![
            OrderBy::parse("CreatedDate DESC").unwrap(),
            OrderBy::parse("Name").unwrap(),
        ];
        vl.concise = true;
        vl.verbose = true;
        vl.show_conversions_only = true;
        vl.api_version = Some(ApiVersion::new("62.0").unwrap());
        let v04t = Id04t::new("04t000000000001AAA").unwrap();
        vec![
            ReadOnlyCommand::Version,
            ReadOnlyCommand::Plugins,
            ReadOnlyCommand::Commands,
            ReadOnlyCommand::OrgList {
                skip_connection_status: true,
                all: true,
            },
            ReadOnlyCommand::AliasList,
            ReadOnlyCommand::ConfigGet,
            ReadOnlyCommand::PkgList {
                hub: hub(),
                verbose: true,
                api_version: Some(ApiVersion::new("62.0").unwrap()),
            },
            ReadOnlyCommand::PkgVersionList(vl),
            ReadOnlyCommand::PkgVersionReport {
                hub: hub(),
                package: PackageRef::Id(v04t.clone()),
                verbose: true,
            },
            ReadOnlyCommand::PkgVersionAncestry {
                hub: hub(),
                package: AncestryTarget::Package(Id0Ho::new("0Ho000000000001AAA").unwrap()),
                dot_code: true,
                verbose: true,
            },
            ReadOnlyCommand::PkgVersionDeps {
                hub: hub(),
                package: DependencyTarget::Version(v04t.clone()),
                edge_direction: Some(EdgeDirection::RootLast),
                verbose: true,
            },
            ReadOnlyCommand::PkgCreateList {
                hub: hub(),
                created_last_days: Some(0),
                status: Some(CreateStatus::InProgress),
                show_conversions_only: true,
                verbose: true,
            },
            ReadOnlyCommand::PkgCreateReport {
                hub: hub(),
                request: Id08c::new("08c000000000001AAA").unwrap(),
            },
            ReadOnlyCommand::PkgInstalledList { org: hub() },
            ReadOnlyCommand::Pkg1VersionList {
                org: hub(),
                package_id: Some(Id033::new("033000000000001AAA").unwrap()),
            },
            ReadOnlyCommand::Pkg1VersionDisplay {
                org: hub(),
                version: v04t,
            },
        ]
    }

    #[test]
    fn every_variant_builds_and_is_safe() {
        let m = m();
        let deprecated = [
            "--targetdevhubusername",
            "--target-hub-org",
            "--targetusername",
            "-u",
            "--orderby",
            "--createdlastdays",
            "--modifiedlastdays",
            "--packageid",
            "--packageversionid",
            "--packagecreaterequestid",
            "--apiversion",
            "--loglevel",
            "--clean",
            "--no-prompt",
        ];
        for cmd in all_variants() {
            let argv = cmd.argv(&m).unwrap_or_else(|e| panic!("{cmd:?}: {e}"));
            assert_eq!(argv.last().map(String::as_str), Some("--json"));
            assert!(!is_blocked_command(cmd.id()));
            for a in &argv {
                assert!(!deprecated.contains(&a.as_str()), "{cmd:?} emitted {a}");
            }
            // Command words come first and match the id.
            let words: Vec<&str> = cmd.id().split(':').collect();
            assert_eq!(&argv[..words.len()], words.as_slice());
        }
    }

    #[test]
    fn golden_version_list() {
        let cmd = &all_variants()[7];
        assert_eq!(
            cmd.argv(&m()).unwrap(),
            [
                "package",
                "version",
                "list",
                "--target-dev-hub",
                "my-hub",
                "--packages",
                "0Ho000000000001AAA,My Pkg",
                "--released",
                "--branch",
                "main",
                "--created-last-days",
                "0",
                "--modified-last-days",
                "3",
                "--order-by",
                "CreatedDate DESC,Name",
                "--concise",
                "--verbose",
                "--show-conversions-only",
                "--api-version",
                "62.0",
                "--json"
            ]
        );
        assert!(cmd.uses_project_alias());
    }

    #[test]
    fn golden_simple_commands() {
        let m = m();
        assert_eq!(
            ReadOnlyCommand::Version.argv(&m).unwrap(),
            ["version", "--json"]
        );
        assert_eq!(
            ReadOnlyCommand::ConfigGet.argv(&m).unwrap(),
            ["config", "get", "target-dev-hub", "target-org", "--json"]
        );
        assert_eq!(
            ReadOnlyCommand::PkgInstalledList { org: hub() }
                .argv(&m)
                .unwrap(),
            [
                "package",
                "installed",
                "list",
                "--target-org",
                "my-hub",
                "--json"
            ]
        );
    }

    #[test]
    fn order_by_is_allow_listed() {
        assert!(OrderBy::parse("CreatedDate").is_ok());
        assert!(OrderBy::parse("createddate").is_err());
        assert!(OrderBy::parse("Id; DELETE").is_err());
        assert!(OrderBy::parse("Name DESC NULLS LAST").is_err());
        assert!(OrderBy::parse("Name SIDEWAYS").is_err());
    }

    #[test]
    fn manifest_drift_is_detected() {
        let mut m = m();
        let c = m
            .commands
            .iter_mut()
            .find(|c| c.id == "package:list")
            .unwrap();
        c.flags.retain(|f| f.name != "verbose");
        let cmd = ReadOnlyCommand::PkgList {
            hub: hub(),
            verbose: true,
            api_version: None,
        };
        assert!(matches!(cmd.argv(&m), Err(ArgvError::UnknownFlag { .. })));
    }

    #[test]
    fn parse_packages_mixes_ids_and_aliases() {
        let p = parse_packages("0Ho000000000001AAA, Foo@1.0.0-1").unwrap();
        assert!(matches!(p[0], PackageRef::Id(_)));
        assert!(matches!(p[1], PackageRef::Alias(_)));
        assert!(parse_packages("bad\"x").is_err());
    }
}
