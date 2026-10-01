//! UI-free core of GP Atlas.
//!
//! M0 provides the pinned CLI contract: the supported `sf` version, the
//! command manifest generated from `sf commands --json`, and the blocklist of
//! commands GP Atlas must never run. The runner, parsers and probes follow in
//! later milestones (see `SPEC.md` §12).

pub mod blocklist;
pub mod manifest;

/// The only Salesforce CLI version GP Atlas supports (SPEC §4.2).
pub const REQUIRED_CLI_VERSION: &str = "2.150.6";

/// `cliVersion` as reported by `sf version --json` for [`REQUIRED_CLI_VERSION`] (F1).
pub const REQUIRED_CLI_VERSION_STRING: &str = "@salesforce/cli/2.150.6";

/// The packaging plugin bundled with `sf` 2.150.6 (F2, D3).
pub const PACKAGING_PLUGIN_NAME: &str = "@salesforce/plugin-packaging";
/// Version of [`PACKAGING_PLUGIN_NAME`] bundled with `sf` 2.150.6 (F2, D3).
pub const PACKAGING_PLUGIN_VERSION: &str = "3.0.6";

/// Install command shown as the fix for a missing or mismatched CLI (§4.2).
pub const CLI_INSTALL_COMMAND: &str = "npm install --global @salesforce/cli@2.150.6";
