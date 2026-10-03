//! Per-org capability probes (SPEC §5.2 L2).

use crate::command::{PkgVersionListArgs, ReadOnlyCommand};
use crate::ids::{OrgRef, ValidationError};
use crate::orgs::{Org, OrgKind};

/// The five probed capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Capability {
    Pkg2ListPackages,
    Pkg2ListVersions,
    Pkg2CreateRequests,
    Pkg2InstalledList,
    Pkg1ListVersions,
}

impl Capability {
    pub const ALL: [Self; 5] = [
        Self::Pkg2ListPackages,
        Self::Pkg2ListVersions,
        Self::Pkg2CreateRequests,
        Self::Pkg2InstalledList,
        Self::Pkg1ListVersions,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Pkg2ListPackages => "Pkg2.ListPackages",
            Self::Pkg2ListVersions => "Pkg2.ListVersions",
            Self::Pkg2CreateRequests => "Pkg2.CreateRequests",
            Self::Pkg2InstalledList => "Pkg2.InstalledList",
            Self::Pkg1ListVersions => "Pkg1.ListVersions",
        }
    }

    /// Short column header.
    pub fn short(self) -> &'static str {
        match self {
            Self::Pkg2ListPackages => "2GP pkgs",
            Self::Pkg2ListVersions => "2GP vers",
            Self::Pkg2CreateRequests => "2GP builds",
            Self::Pkg2InstalledList => "Installed",
            Self::Pkg1ListVersions => "1GP vers",
        }
    }

    /// Needs a Dev Hub.
    pub fn is_dev_hub_capability(self) -> bool {
        matches!(
            self,
            Self::Pkg2ListPackages | Self::Pkg2ListVersions | Self::Pkg2CreateRequests
        )
    }

    /// Exactly the probe command of §5.2.
    pub fn probe(self, org: OrgRef) -> ReadOnlyCommand {
        match self {
            Self::Pkg2ListPackages => ReadOnlyCommand::PkgList {
                hub: org,
                verbose: false,
                api_version: None,
            },
            Self::Pkg2ListVersions => {
                let mut a = PkgVersionListArgs::new(org);
                a.concise = true;
                a.created_last_days = Some(0);
                ReadOnlyCommand::PkgVersionList(a)
            }
            Self::Pkg2CreateRequests => ReadOnlyCommand::PkgCreateList {
                hub: org,
                created_last_days: Some(0),
                status: None,
                show_conversions_only: false,
                verbose: false,
            },
            Self::Pkg2InstalledList => ReadOnlyCommand::PkgInstalledList { org },
            Self::Pkg1ListVersions => ReadOnlyCommand::Pkg1VersionList {
                org,
                package_id: None,
            },
        }
    }

    /// Whether this capability is expected to apply to the org (§5.2 candidates).
    /// `Err(reason)` means "not applicable"; the user may still "try anyway".
    pub fn applies_to(self, org: &Org) -> Result<(), &'static str> {
        if org.kind == OrgKind::Scratch && self != Self::Pkg2InstalledList {
            return Err("scratch org");
        }
        if self.is_dev_hub_capability() && !org.is_dev_hub {
            return Err("not flagged as Dev Hub");
        }
        Ok(())
    }
}

/// The validated `--target-*` value for an org.
pub fn org_ref(org: &Org) -> Result<OrgRef, ValidationError> {
    OrgRef::new(org.target()).or_else(|_| OrgRef::new(&org.username))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Manifest;

    #[test]
    fn probes_are_exactly_the_spec_commands() {
        let m = Manifest::embedded().unwrap();
        let o = OrgRef::new("hub").unwrap();
        let got: Vec<String> = Capability::ALL
            .iter()
            .map(|c| c.probe(o.clone()).argv(&m).unwrap().join(" "))
            .collect();
        assert_eq!(
            got,
            [
                "package list --target-dev-hub hub --json",
                "package version list --target-dev-hub hub --created-last-days 0 --concise --json",
                "package version create list --target-dev-hub hub --created-last-days 0 --json",
                "package installed list --target-org hub --json",
                "package1 version list --target-org hub --json",
            ]
        );
    }
}
