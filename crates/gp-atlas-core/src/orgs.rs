//! Org inventory from `sf org list --json` (SPEC §5.2 I1, F25, F26, F36).

use serde_json::Value;

/// How the CLI grouped an org.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OrgKind {
    DevHub,
    Sandbox,
    Scratch,
    Other,
}

impl OrgKind {
    pub fn badge(self) -> &'static str {
        match self {
            Self::DevHub => "DevHub",
            Self::Sandbox => "Sandbox",
            Self::Scratch => "Scratch",
            Self::Other => "Org",
        }
    }
}

/// One org, de-duplicated by username.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Org {
    pub username: String,
    pub alias: Option<String>,
    pub org_id: Option<String>,
    pub instance_url: Option<String>,
    pub kind: OrgKind,
    pub is_dev_hub: bool,
    /// Free text: "Connected" or an error message (F36). Absent for scratch orgs.
    pub connected_status: Option<String>,
    /// Scratch org status ("Active", …).
    pub status: Option<String>,
    pub is_default_dev_hub: bool,
    pub is_default_org: bool,
}

impl Org {
    /// `alias — username` (§6.1).
    pub fn label(&self) -> String {
        match &self.alias {
            Some(a) => format!("{a} — {}", self.username),
            None => self.username.clone(),
        }
    }

    /// What to pass as `--target-*`: the alias if any, else the username.
    pub fn target(&self) -> &str {
        self.alias.as_deref().unwrap_or(&self.username)
    }

    /// `connectedStatus` is free text; anything but "Connected" is a problem.
    pub fn is_connected(&self) -> Option<bool> {
        self.connected_status.as_deref().map(|s| s == "Connected")
    }
}

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn b(v: &Value, k: &str) -> bool {
    v.get(k).and_then(Value::as_bool).unwrap_or(false)
}

/// Builds the de-duplicated inventory from the `result` of `org list --json`.
pub fn from_org_list(result: &Value) -> Vec<Org> {
    // Most specific group wins; devHubs/sandboxes/other are subsets of nonScratchOrgs (F26).
    let groups = [
        ("devHubs", OrgKind::DevHub),
        ("sandboxes", OrgKind::Sandbox),
        ("scratchOrgs", OrgKind::Scratch),
        ("other", OrgKind::Other),
        ("nonScratchOrgs", OrgKind::Other),
    ];
    let mut orgs: Vec<Org> = Vec::new();
    for (key, kind) in groups {
        let Some(list) = result.get(key).and_then(Value::as_array) else {
            continue;
        };
        for o in list {
            let Some(username) = s(o, "username") else {
                continue;
            };
            if orgs.iter().any(|x| x.username == username) {
                continue;
            }
            orgs.push(Org {
                alias: s(o, "alias"),
                org_id: s(o, "orgId"),
                instance_url: s(o, "instanceUrl"),
                kind,
                is_dev_hub: b(o, "isDevHub"),
                connected_status: s(o, "connectedStatus"),
                status: s(o, "status"),
                is_default_dev_hub: b(o, "isDefaultDevHubUsername"),
                is_default_org: b(o, "isDefaultUsername"),
                username,
            });
        }
    }
    orgs.sort_by(|a, b| {
        (a.kind, a.label().to_lowercase()).cmp(&(b.kind, b.label().to_lowercase()))
    });
    orgs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::{SfOutput, parse};

    #[test]
    fn real_capture() {
        let bytes = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/sf-2.150.6/org-list.real.json"),
        )
        .unwrap();
        let SfOutput::Success { result, .. } = parse(&bytes).unwrap() else {
            panic!()
        };
        let orgs = from_org_list(&result);
        assert_eq!(orgs.len(), 48);
        assert_eq!(orgs.iter().filter(|o| o.kind == OrgKind::DevHub).count(), 3);
        assert_eq!(
            orgs.iter().filter(|o| o.kind == OrgKind::Sandbox).count(),
            4
        );
        assert_eq!(
            orgs.iter().filter(|o| o.kind == OrgKind::Scratch).count(),
            7
        );
        assert_eq!(orgs.iter().filter(|o| o.is_default_dev_hub).count(), 1);
        assert_eq!(
            orgs.iter()
                .filter(|o| o.is_connected() == Some(false))
                .count(),
            7
        );
        assert!(
            orgs.iter()
                .filter(|o| o.kind == OrgKind::Scratch)
                .all(|o| o.connected_status.is_none())
        );
    }
}
