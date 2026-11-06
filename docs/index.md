---
title: Home
nav_order: 1
---

# GP Atlas

**Browse 1GP and 2GP package versions and build `sf` commands, from a
clickable terminal app.**

GP Atlas runs in your terminal next to the Salesforce CLI. It shows the
packages and versions of your Dev Hubs and orgs, what each org lets you read,
how versions are related, and builds exact `sf` commands for you to copy. It
uses your installed `sf` as its only data source and is **read-only**: it never
changes an org.

> Not affiliated with or endorsed by Salesforce. Salesforce and related marks
> are trademarks of Salesforce, Inc.

**Start here:** [Install](install.md) · [User guide](guide.md) ·
[Beta testing](beta-testing.md)

## Features

| Feature | What it does |
|---|---|
| **Doctor** | Checks that `sf` is installed and 2.150.6 or newer, that the packaging plugin is the bundled one, and that every command GP Atlas uses still has the flags it expects. On a newer CLI only a changed command is disabled. |
| **Orgs** | Your Dev Hubs and orgs as `alias — username`, with type and connection status. Pick the Dev Hub and target org with one key. |
| **Access Matrix** | One row per org, one column per thing GP Atlas can read (2GP packages, versions, builds, installed packages, 1GP versions), with the reason when something is denied and a fix to copy. Results are kept 15 minutes, also across restarts. |
| **2GP packages & versions** | Packages and their versions side by side; released only, latest released per package, verbose; copy the 04t or the install link; full version details. |
| **Ancestry** | A package's released version tree, with the selected version's path to its root. |
| **Dependencies** | What to install first, in install order, and whether each dependency is already in the selected org. |
| **Installed packages · 1GP versions** | For the selected org, with released / latest-per-package / package filters for long 1GP lists. |
| **Project aliases** | Reads `packageAliases` from your `sfdx-project.json` (never edits it): an Alias column, orphan / duplicate / missing alias warnings, and ready-to-paste alias entries. |
| **Command Builder** | Forms built from the CLI's own flag list for every read-only command; validated values; the exact command for bash/zsh, PowerShell or cmd; copy it or run it in GP Atlas. |
| **History** | Every `sf` call GP Atlas made, with exit code, time and stderr. |
| **Copy anything** | `c` or a right-click copies the selected ID; works over SSH too. |

## What it never does

GP Atlas can only run a fixed list of read-only `sf` commands. It never
creates, promotes, deletes, installs, converts or schedules anything, never
logs in or out, never changes the CLI configuration or your aliases, never
runs `sf org display`, never reads auth files, and never stores tokens or
installation keys. It makes no network calls of its own.

## Requirements

- Salesforce CLI **2.150.6 or newer** (`sf version`), with at least one org
  you are logged in to (`sf org list`).
- macOS (Apple Silicon or Intel), Linux x64 or Windows x64, and a terminal.

## Links

- [Roadmap](https://github.com/holapancho/gp-atlas/blob/main/ROADMAP.md): what comes next.
- [Changelog](https://github.com/holapancho/gp-atlas/blob/main/CHANGELOG.md).
- [Source code and issues](https://github.com/holapancho/gp-atlas).
