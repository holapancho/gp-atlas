//! Capability states and error classification (SPEC §5.1, §5.3).

use crate::envelope::{self, SfError, SfOutput};
use crate::runner::RunOutput;

/// Why a capability is denied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DenyReason {
    PackagingNotEnabled,
    InsufficientAccess,
    ApiDisabled,
}

/// Why an org could not be reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnreachableReason {
    NotAuthenticated,
    SessionExpired,
    Network,
    /// Salesforce answered with an HTML page (HTTP 420): the org no longer
    /// exists or its domain changed (deleted scratch org, expired trial,
    /// refreshed sandbox). Observed: `ERROR_HTTP_420` (F38).
    OrgUnavailable,
}

/// Why the state is unknown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnknownReason {
    Timeout,
    Cancelled,
    NotJson,
    Unclassified,
}

/// Capability state (§5.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapState {
    Allowed,
    Denied(DenyReason),
    NotApplicable(String),
    Unreachable(UnreachableReason),
    Unknown(UnknownReason),
    ContractDrift(String),
}

impl CapState {
    pub fn short(&self) -> String {
        match self {
            Self::Allowed => "Allowed".into(),
            Self::Denied(r) => format!("Denied({r:?})"),
            Self::NotApplicable(r) => format!("N/A({r})"),
            Self::Unreachable(r) => format!("Unreachable({r:?})"),
            Self::Unknown(r) => format!("Unknown({r:?})"),
            Self::ContractDrift(d) => format!("ContractDrift({d})"),
        }
    }

    /// Copy-only fix hint (§5.3). Never executed.
    pub fn hint(&self, org: &str) -> Option<String> {
        Some(match self {
            Self::Unreachable(UnreachableReason::NotAuthenticated | UnreachableReason::SessionExpired) => {
                format!("sf org login web --alias {org}")
            }
            Self::Denied(DenyReason::PackagingNotEnabled) => "This org is not a Dev Hub / second-generation packaging isn't enabled, or the object isn't exposed here.".into(),
            Self::Denied(DenyReason::InsufficientAccess) => "Your user lacks access to this object. Ask an admin.".into(),
            Self::Denied(DenyReason::ApiDisabled) => "API access is disabled for this org/user.".into(),
            Self::Unreachable(UnreachableReason::Network) => "Check your connection / VPN.".into(),
            Self::Unreachable(UnreachableReason::OrgUnavailable) => format!(
                "The org looks deleted, expired or moved. If it still exists: sf org login web --alias {org} \
                 — otherwise remove it from sf: sf org logout --target-org {org}"
            ),
            Self::Unknown(UnknownReason::Timeout) => "Retry; increase the timeout.".into(),
            _ => return None,
        })
    }
}

/// Result of classifying one run: the state plus the raw error, always kept.
#[derive(Debug, Clone)]
pub struct Classified {
    pub state: CapState,
    pub error: Option<SfError>,
    /// Which rule matched (for debugging the heuristics, U5).
    pub rule: &'static str,
}

fn contains_ci(hay: &str, needle: &str) -> bool {
    hay.to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

/// Classifies an error envelope (§5.3, first match wins).
pub fn classify_error(e: &SfError) -> (CapState, &'static str) {
    let text = format!("{} {} {}", e.name, e.message, e.actions.join(" "));
    if e.name == "NamedOrgNotFoundError" {
        return (
            CapState::Unreachable(UnreachableReason::NotAuthenticated),
            "NamedOrgNotFoundError (F7)",
        );
    }
    if e.name == "ERROR_HTTP_420" {
        return (
            CapState::Unreachable(UnreachableReason::OrgUnavailable),
            "ERROR_HTTP_420 (F38, observed)",
        );
    }
    if (text.contains("INVALID_TYPE") && text.contains("sObject type") && text.contains("Package"))
        || contains_ci(&text, "packaging is not enabled")
    {
        return (
            CapState::Denied(DenyReason::PackagingNotEnabled),
            "INVALID_TYPE/packaging not enabled (F20)",
        );
    }
    if contains_ci(&text, "INSUFFICIENT_ACCESS") {
        return (
            CapState::Denied(DenyReason::InsufficientAccess),
            "substring INSUFFICIENT_ACCESS (heuristic, U5)",
        );
    }
    if contains_ci(&text, "API_DISABLED") || contains_ci(&text, "API_CURRENTLY_DISABLED") {
        return (
            CapState::Denied(DenyReason::ApiDisabled),
            "substring API_DISABLED (heuristic, U5)",
        );
    }
    for s in [
        "INVALID_SESSION_ID",
        "expired",
        "invalid_grant",
        "RefreshToken",
    ] {
        if contains_ci(&text, s) {
            return (
                CapState::Unreachable(UnreachableReason::SessionExpired),
                "session substring (heuristic, U5)",
            );
        }
    }
    for s in ["ENOTFOUND", "ETIMEDOUT", "ECONN", "EAI_AGAIN"] {
        if text.contains(s) {
            return (
                CapState::Unreachable(UnreachableReason::Network),
                "network substring (heuristic, U5)",
            );
        }
    }
    (
        CapState::Unknown(UnknownReason::Unclassified),
        "no rule matched",
    )
}

/// Classifies a finished run.
pub fn classify(out: &RunOutput) -> Classified {
    if out.timed_out {
        return Classified {
            state: CapState::Unknown(UnknownReason::Timeout),
            error: None,
            rule: "timeout",
        };
    }
    if out.cancelled {
        return Classified {
            state: CapState::Unknown(UnknownReason::Cancelled),
            error: None,
            rule: "cancelled",
        };
    }
    match envelope::parse(&out.stdout) {
        Ok(SfOutput::Success { .. }) => Classified {
            state: CapState::Allowed,
            error: None,
            rule: "status 0",
        },
        Ok(SfOutput::Error(e)) => {
            let (state, rule) = classify_error(&e);
            Classified {
                state,
                error: Some(e),
                rule,
            }
        }
        Ok(SfOutput::Bare(_)) if out.exit_code == Some(0) => Classified {
            state: CapState::Allowed,
            error: None,
            rule: "exit 0, bare JSON",
        },
        Ok(SfOutput::Bare(_)) => Classified {
            state: CapState::Unknown(UnknownReason::Unclassified),
            error: None,
            rule: "non-zero exit, bare JSON",
        },
        Err(_) => Classified {
            state: CapState::Unknown(UnknownReason::NotJson),
            error: None,
            rule: "stdout not JSON",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(name: &str, message: &str) -> SfError {
        SfError {
            name: name.into(),
            message: message.into(),
            code: None,
            exit_code: Some(1),
            status: Some(1),
            context: None,
            command_name: None,
            actions: vec![],
            warnings: vec![],
        }
    }

    #[test]
    fn rules_in_order() {
        let cases = [
            (
                err("NamedOrgNotFoundError", "x"),
                CapState::Unreachable(UnreachableReason::NotAuthenticated),
            ),
            (
                err(
                    "SfError",
                    "INVALID_TYPE: sObject type 'Package2' is not supported",
                ),
                CapState::Denied(DenyReason::PackagingNotEnabled),
            ),
            (
                err("x", "Packaging is not enabled on this org"),
                CapState::Denied(DenyReason::PackagingNotEnabled),
            ),
            (
                err("x", "INSUFFICIENT_ACCESS_OR_READONLY"),
                CapState::Denied(DenyReason::InsufficientAccess),
            ),
            (
                err("x", "API_DISABLED_FOR_ORG"),
                CapState::Denied(DenyReason::ApiDisabled),
            ),
            (
                err("x", "Session expired or invalid"),
                CapState::Unreachable(UnreachableReason::SessionExpired),
            ),
            (
                err("x", "getaddrinfo ENOTFOUND login.salesforce.com"),
                CapState::Unreachable(UnreachableReason::Network),
            ),
            (
                err("x", "something else"),
                CapState::Unknown(UnknownReason::Unclassified),
            ),
        ];
        for (e, want) in cases {
            assert_eq!(classify_error(&e).0, want, "{e:?}");
        }
    }

    #[test]
    fn real_http_420_fixture() {
        let stdout = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/sf-2.150.6/package-installed-list.http-420.json"),
        )
        .unwrap();
        let out = RunOutput {
            argv: vec![],
            exit_code: Some(1),
            stdout,
            stderr: vec![],
            duration: Default::default(),
            timed_out: false,
            cancelled: false,
            truncated: false,
        };
        let c = classify(&out);
        assert_eq!(
            c.state,
            CapState::Unreachable(UnreachableReason::OrgUnavailable)
        );
        assert!(
            c.state
                .hint("my-org")
                .unwrap()
                .contains("sf org logout --target-org my-org")
        );
    }

    #[test]
    fn real_not_found_fixture() {
        let stdout = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/sf-2.150.6/package-list.named-org-not-found.json"),
        )
        .unwrap();
        let out = RunOutput {
            argv: vec![],
            exit_code: Some(2),
            stdout,
            stderr: vec![],
            duration: Default::default(),
            timed_out: false,
            cancelled: false,
            truncated: false,
        };
        let c = classify(&out);
        assert_eq!(
            c.state,
            CapState::Unreachable(UnreachableReason::NotAuthenticated)
        );
        assert!(c.error.is_some());
    }
}
