---
title: Beta testing
nav_order: 4
---

# Beta testing

Thanks for testing GP Atlas. This page covers a **debug build** (readable
crash backtraces), the `gp-atlas-beta` debug tool, and how to send what you
find. Everything here is read-only too.

## Install a debug build

You need Rust ([rustup.rs](https://rustup.rs)), a C linker (see
[Install: build from source](install.md#build-from-source-with-cargo)) and `git`.

**From GitHub, no clone** (into `~/.cargo/bin`):

```bash
cargo install --locked --debug --git https://github.com/holapancho/gp-atlas gp-atlas-tui gp-atlas-cli
```

**Or clone it**, which also lets you run the tests and follow `main`:

```bash
git clone https://github.com/holapancho/gp-atlas.git
cd gp-atlas
cargo build --locked -p gp-atlas-tui -p gp-atlas-cli
```

The programs are `target/debug/gp-atlas` and `target/debug/gp-atlas-beta`
(`.exe` on Windows). Update with `git pull` and the same `cargo build` line.

**Pre-release binaries without Rust:** every push to `main` builds them in
[Actions → Beta binaries](https://github.com/holapancho/gp-atlas/actions/workflows/beta.yml)
(open the latest green run, then *Artifacts*; you must be signed in to GitHub;
kept 30 days). Unzip twice; on macOS remove the quarantine flag as in
[Install](install.md#download-a-release-by-hand).

## Run with debugging on

macOS / Linux:

```bash
RUST_BACKTRACE=1 ./target/debug/gp-atlas
RUST_BACKTRACE=1 ./target/debug/gp-atlas-beta --debug doctor
```

Windows PowerShell:

```powershell
$env:RUST_BACKTRACE = "1"
.\target\debug\gp-atlas.exe
.\target\debug\gp-atlas-beta.exe --debug doctor
```

- `RUST_BACKTRACE=1` prints a full backtrace if a program crashes.
- In the app, the **History** tab (7) shows every `sf` call with its stderr.
- `--sf /path/to/sf` (or `GP_ATLAS_SF_BIN`) picks a specific `sf`, handy to
  compare two CLI versions.

## The debug tool: `gp-atlas-beta`

Same core as the app, one command per action. Global options: `--debug` (print
every `sf` call, exit code, time, stderr), `--raw` (the JSON instead of a
table), `--timeout SECS`, `--sf PATH`.

| Command | What it does |
|---|---|
| `gp-atlas-beta doctor` | `sf` found, version, packaging plugin, command contract |
| `gp-atlas-beta orgs` | Your orgs: alias, username, type, Dev Hub flag, connection status |
| `gp-atlas-beta probe` | Access Matrix (`--org X` to limit, `--try-anyway`, `--details`) |
| `gp-atlas-beta packages --hub X` | 2GP packages in a Dev Hub |
| `gp-atlas-beta versions --hub X` | 2GP versions (`--released`, `--branch`, `--created-last-days`, `--modified-last-days`, `--order-by "CreatedDate DESC"`, `--packages 0Ho…`, `--verbose`, `--latest`) |
| `gp-atlas-beta report --hub X --package 04t…` | One version's details |
| `gp-atlas-beta builds --hub X` | Version-create requests (`--status`, `--created-last-days`) |
| `gp-atlas-beta installed --org X` | Installed packages |
| `gp-atlas-beta pkg1 --org X` | 1GP versions (`--package-id 033…`) |
| `gp-atlas-beta project [--dir D] [--hub X]` | Project aliases; with `--hub`, orphan and missing aliases |
| `gp-atlas-beta show probes --org X` | The probe commands, without running them |
| `gp-atlas-beta capture --out DIR` | Run everything and save **sanitized** output (below) |

## Report a problem

Open an [issue](https://github.com/holapancho/gp-atlas/issues) with:

1. what you did and what you expected,
2. `gp-atlas-beta --debug doctor`,
3. the failing step again with `--debug` (or the History entry from the app),
4. a backtrace, if it crashed.

Debug output contains your usernames and aliases: redact them first if needed.

## Help with real `sf` outputs

Some error cases can only be learned from real orgs. `capture` records them
with every username, ID, host and name replaced by fakes:

```bash
gp-atlas-beta doctor
gp-atlas-beta capture --out gp-atlas-capture
```

It runs the inventory, the Dev Hub listings and the Access Matrix probes on
every org (`--max-orgs N` limits it). **Review the folder before sending**
(its `README.txt` explains what is inside); `summary.md` shows every distinct
failure and the rule that classified it. The list of outputs still wanted is
in [Fixture capture](fixture-capture.md). Pasting an anonymized command and
its JSON into an issue works too.
