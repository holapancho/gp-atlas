---
title: User guide
nav_order: 3
---

# User guide

Start the app in a terminal:

```bash
gp-atlas
```

Start it inside an SFDX project folder (or pass `--project <folder>`) to also
see the project's package aliases.

It opens full-screen. Click a tab or a row (click the selected row again to
open it), scroll with the wheel, or use the keyboard. `?` shows every key,
`q` quits, `Esc` cancels or closes.

## Keys on every tab

| Key | Action |
|---|---|
| `1`–`8`, `Tab`, `Shift-Tab` | Switch tab |
| `↑` `↓` `j` `k` `PgUp` `PgDn` `g` `G` | Move |
| `Enter` | Open or select |
| `r` | Refresh the tab |
| `c` or **right-click** | Copy the selected row's ID (username on Orgs and Access, the command on History) |
| `P` | Project aliases and their warnings |
| `?` | Help |
| `Esc` | Cancel / close |
| `q` | Quit |

Copied text is also shown in the status line. Over SSH, where there is no
system clipboard, GP Atlas asks the terminal to copy (OSC 52: iTerm2, kitty,
WezTerm, Windows Terminal, tmux with `set-clipboard on`; not macOS
Terminal.app). To select text with the mouse, hold Shift (Option on macOS),
or start with `--no-mouse`.

## 1 · Doctor

Checks `sf`: found, version 2.150.6 or newer, the bundled packaging plugin, and
each command's flags. On a newer CLI, only a command whose flags changed is
disabled, and Doctor lists it. When everything passes the app moves to Orgs.

## 2 · Orgs

Your Dev Hubs and orgs as `alias — username`, with type, Dev Hub flag and
connection status. Your default Dev Hub and org are preselected. Select a row
and press `h` to use it as Dev Hub, `o` as target org.

## 3 · Access

The **Access Matrix**: one row per org, one column per thing GP Atlas can read
(2GP packages, 2GP versions, 2GP builds, installed packages, 1GP versions).

| Key | Action |
|---|---|
| `a` | Probe all orgs (keeps results younger than 15 minutes) |
| `A` | Re-test all |
| `p` | Re-test the selected org |
| `←` `→` | Choose a column |
| `Enter` | Explain the cell: the error, the rule that classified it, a fix to copy |
| `t` | Also try the 2GP columns on orgs that are not Dev Hubs |

✅ allowed (an empty list counts) · ⛔ denied · 🔌 unreachable (expired session,
deleted org, no network) · ? unknown · — not applicable. Results are kept for 15
minutes, also after you close the app; timeouts and network errors are always
retried. `--no-cache` turns the cache off.

## 4 · 2GP Packages & Versions

Packages on the left, the selected package's versions on the right
(`‹ All packages ›` shows every version). `←` `→` or a click switch pane.
The Description column shows each version's description (the first line,
`…` when there is more); `Enter` shows it in full at the top of the details,
with the package, version, 04t, branch, tag and ancestor, then the full
`package version report --verbose`. When the pane is narrow, Created, then
Ancestor, then Branch are hidden from the table to leave room for the
description; widen the terminal to see them, or open the details.

| Key | Action |
|---|---|
| `R` | Released versions only |
| `L` | Latest released version per package (computed by GP Atlas) |
| `V` | Verbose (code coverage and more) |
| `x` | All packages |
| `c` | Copy the 0Ho (left) or 04t (right) |
| `i` | Copy the install link |
| `y` | Copy the `package version report` command |
| `Enter` | Version details |
| `a` | Ancestry: the version tree, with this version's path to the root highlighted |
| `A` | Ancestry of the whole package |
| `d` | Dependencies, in install order (`t` toggles a tree), with "In org" for the selected org |
| `s` / `S` | Copy a `packageAliases` entry for this row / for every row without an alias |

Ancestry covers released versions of managed packages. Dependencies need a 2GP
version; if `sf` cannot build the graph (for example a dependency from another
publisher), GP Atlas explains why and shows the direct dependencies instead.

**Project aliases.** With a project loaded, an Alias column shows each
package's and version's alias from `sfdx-project.json` (yellow: several aliases,
or `sf` disagrees). The header shows `Project: <folder> · N aliases ⚠ W`; `P`
lists orphan aliases (ID not in this Dev Hub), missing aliases, duplicates and
ignored entries. GP Atlas never edits the file.

## 5 · Installed

Packages installed in the selected org. `c` copies the 04t, `Enter` shows
details.

## 6 · 1GP Versions

1GP versions in the selected org (usually your packaging org). Lists can be
long, so:

| Key | Action |
|---|---|
| `R` | Released only |
| `L` | Latest released per package |
| `f` | Only the selected row's package (`f` again clears it) |
| `x` | Clear filters |

## 7 · History

Every `sf` call GP Atlas made: command, exit code, duration and stderr. `Enter`
shows the details, `c` copies the command.

## 8 · Command Builder

Build any read-only command GP Atlas supports, from a form made from the CLI's
own flag list.

1. Pick a command on the left (`↑` `↓`).
2. `→` moves to the form. Empty fields are filled from what you selected
   elsewhere (Dev Hub, org, 2GP package or version; `f` fills again).
   Required fields are marked `*`.
3. `Enter` or `Space` edits a value, toggles a switch or cycles a choice;
   `Del` clears it. An invalid value shows `⚠` and the reason.
4. The box below is the exact command:

| Key | Action |
|---|---|
| `s` | Shell: bash/zsh, PowerShell, cmd (quoting follows) |
| `J` | With or without `--json` |
| `F` | One-letter flags |
| `M` | One flag per line |
| `U` | Dev Hub / org: alias ↔ username |
| `A` | Package: ID ↔ project alias (adds `cd <project>`, because aliases only work inside the project) |
| `c` | Copy the command |
| `R` | Run it in GP Atlas; the result opens in a popup and the call goes to History |
| `x` | Clear the form |

Only read-only commands are offered; flags GP Atlas does not support yet are
listed under the preview.

## Command-line options

| Option | Effect |
|---|---|
| `--project <folder>` | Read `packageAliases` from this project (default: the current folder and its parents) |
| `--no-project` | Do not look for `sfdx-project.json` |
| `--no-cache` | Do not read or write the Access Matrix cache |
| `--no-mouse` | Leave the mouse to the terminal (normal text selection) |
| `--sf <path>` | Use this `sf` (default: `GP_ATLAS_SF_BIN`, then `sf` on `PATH`) |
| `--timeout <secs>` | Per-command timeout (default 120) |

`gp-atlas --help` lists them all.
