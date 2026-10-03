//! UI-free core of GP Atlas.
//!
//! Contains the pinned CLI contract (manifest, blocklist), validated inputs,
//! the closed `ReadOnlyCommand` set, the runner, JSON parsing and scrubbing,
//! error classification, Doctor checks, org inventory and capability probes.

pub mod blocklist;
pub mod classify;
pub mod command;
pub mod doctor;
pub mod envelope;
pub mod ids;
pub mod manifest;
pub mod orgs;
pub mod probes;
pub mod runner;
pub mod versions;

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
