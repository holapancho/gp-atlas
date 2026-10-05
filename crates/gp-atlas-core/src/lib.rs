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

pub mod cli_version;

/// Oldest supported Salesforce CLI version (SPEC §4.2). Newer versions are
/// accepted; per-command contract drift (D4) disables only affected commands.
pub const MIN_CLI_VERSION: &str = "2.150.6";

/// The CLI version the manifest and fixtures are generated from (baseline).
pub const BASELINE_CLI_VERSION_STRING: &str = "@salesforce/cli/2.150.6";

/// The packaging plugin (F2, D3).
pub const PACKAGING_PLUGIN_NAME: &str = "@salesforce/plugin-packaging";
/// Version of [`PACKAGING_PLUGIN_NAME`] bundled with the baseline CLI (F2).
pub const PACKAGING_PLUGIN_BASELINE_VERSION: &str = "3.0.6";

/// Fix shown for a missing or too-old CLI (§4.2).
pub const CLI_INSTALL_COMMAND: &str = "npm install --global @salesforce/cli@latest";

/// Installs the baseline CLI (manifest regeneration, contract tests).
pub const BASELINE_INSTALL_COMMAND: &str = "npm install --global @salesforce/cli@2.150.6";
