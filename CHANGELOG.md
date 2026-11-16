# Changelog

All notable changes. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Fixed

- 2GP versions: the version description is shown, as a Description column and
  at the top of the version details (from the report, or the version list when
  the report has none) ([#5](https://github.com/holapancho/gp-atlas/issues/5)).

## [0.1.0] - 2026-11-10

First public version. Requires the Salesforce CLI 2.150.6 or newer.

### Added

- `gp-atlas`, a clickable terminal app (mouse and keyboard), read-only:
  - **Doctor**: `sf` found, version gate (2.150.6 or newer), bundled packaging
    plugin, per-command flag contract; on a newer CLI only a changed command
    is disabled.
  - **Orgs**: Dev Hubs and orgs, default hub and org preselected.
  - **Access Matrix**: five read capabilities per org, classified errors with
    copyable fixes, results cached 15 minutes across restarts.
  - **2GP packages & versions**: split view, released / latest-per-package /
    verbose filters, version details, copy 04t and install link.
  - **Ancestry** (whole-package tree, path to root) and **dependencies**
    (install order, "In org" check, fallback to direct dependencies when the
    CLI cannot build the graph).
  - **Installed packages** and **1GP versions** with released /
    latest-per-package / package filters.
  - **Project aliases** from `sfdx-project.json` (read-only): Alias column,
    orphan / duplicate / missing warnings, `packageAliases` snippets.
  - **Command Builder**: manifest-generated forms for every read-only
    command, validated fields, quoting for bash/zsh, PowerShell and cmd,
    copy or run.
  - **History** of every `sf` call; copy IDs with `c` or a right-click, also
    over SSH.
- `gp-atlas-beta`, a command-line debug tool with sanitized fixture capture.
- Install scripts for macOS, Linux and Windows; release builds with
  checksums; documentation site.

[Unreleased]: https://github.com/holapancho/gp-atlas/compare/v0.1.0...main
[0.1.0]: https://github.com/holapancho/gp-atlas/releases/tag/v0.1.0
