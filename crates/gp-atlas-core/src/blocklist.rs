//! Commands and flags GP Atlas must never run (SPEC §1 rule 1, §4.4).
//!
//! The runner's closed `ReadOnlyCommand` enum (M1) is the primary control:
//! nothing outside it can be executed. This module is the second line of
//! defence: tests assert that nothing in the manifest, and (from M1) nothing
//! the runner can emit, matches these rules.

/// Final command-id segments that mark a state-changing command.
pub const BLOCKED_FINAL_SEGMENTS: &[&str] = &[
    "create",
    "delete",
    "promote",
    "update",
    "install",
    "uninstall",
    "convert",
    "retrieve",
    "schedule",
    "abort",
];

/// Exact command ids (oclif `:`-separated form) that are never runnable.
pub const BLOCKED_COMMANDS: &[&str] = &[
    "org:display",
    "org:logout",
    "config:set",
    "config:unset",
    "alias:set",
    "alias:unset",
];

/// Command-id prefixes whose every subcommand is never runnable.
/// `org:login:*` authenticates; `plugins:*` (install, link, uninstall,
/// update, reset, …) changes the CLI itself. Bare `plugins` (list) is allowed.
pub const BLOCKED_PREFIXES: &[&str] = &["org:login:", "plugins:"];

/// Flags that must never be passed to an otherwise allowed command.
pub const BLOCKED_FLAGS: &[(&str, &str)] = &[("org:list", "clean")];

/// Normalizes a command id: accepts `package version list` or
/// `package:version:list` and returns the `:` form used by `sf commands --json`.
pub fn normalize_id(id: &str) -> String {
    id.split([' ', ':'])
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(":")
}

/// Returns `true` if the command must never be run by GP Atlas.
pub fn is_blocked_command(id: &str) -> bool {
    let id = normalize_id(id);
    let last = id.rsplit(':').next().unwrap_or_default();
    BLOCKED_FINAL_SEGMENTS.contains(&last)
        || BLOCKED_COMMANDS.contains(&id.as_str())
        || BLOCKED_PREFIXES.iter().any(|p| id.starts_with(p))
}

/// Returns `true` if `flag` (long name, without dashes) must never be passed to `command`.
pub fn is_blocked_flag(command: &str, flag: &str) -> bool {
    let command = normalize_id(command);
    BLOCKED_FLAGS
        .iter()
        .any(|(c, f)| *c == command && *f == flag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_mutating_commands() {
        for id in [
            "package version create",
            "package:version:promote",
            "package version delete",
            "package version update",
            "package create",
            "package install",
            "package uninstall",
            "package convert",
            "package version retrieve",
            "package push-upgrade schedule",
            "package push-upgrade abort",
            "package1 version create",
            "org display",
            "org login web",
            "org login jwt",
            "org logout",
            "config set",
            "alias set",
            "plugins install",
            "plugins link",
            "plugins uninstall",
            "plugins update",
        ] {
            assert!(is_blocked_command(id), "{id} must be blocked");
        }
    }

    #[test]
    fn allows_read_only_commands() {
        for id in [
            "version",
            "plugins",
            "commands",
            "org list",
            "alias list",
            "config get",
            "package list",
            "package version list",
            "package version report",
            "package version displayancestry",
            "package version displaydependencies",
            "package version create list",
            "package version create report",
            "package installed list",
            "package1 version list",
            "package1 version display",
            "package push-upgrade list",
            "package1 version create get",
        ] {
            assert!(!is_blocked_command(id), "{id} must be allowed");
        }
    }

    #[test]
    fn blocks_org_list_clean() {
        assert!(is_blocked_flag("org list", "clean"));
        assert!(!is_blocked_flag("org list", "skip-connection-status"));
    }
}
