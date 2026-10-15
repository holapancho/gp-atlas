# GP Atlas beta

Two programs, both read-only and built on the same core:

| Program | What it is |
|---|---|
| **`gp-atlas`** | The **clickable terminal UI**: tabs, tables, popups. Use this day to day. |
| `gp-atlas-beta` | Command-line debug tool: one command per action, `--debug`, and `capture` for fixtures. |

## Terminal UI (`gp-atlas`)

```bash
./target/debug/gp-atlas            # if you built it yourself (see below)
./gp-atlas                         # if you downloaded it
```

It opens full-screen in your terminal. Click a tab or a row (click the selected
row again to open it), scroll with the wheel, or use the keyboard; `?` shows
all keys, `q` quits.

**Copying IDs:** `c` or a **right-click** on a row copies its ID (0Ho, 04t; the
username on Orgs/Access; the command in History). Over SSH, where there is no
OS clipboard, GP Atlas asks the terminal to copy instead (OSC 52: iTerm2,
kitty, WezTerm, Windows Terminal, tmux with `set-clipboard on`; not macOS
Terminal.app). The copied text is also shown in the status line. To select
text with the mouse, hold Shift (Option on macOS) or start with `--no-mouse`.

1. **Doctor** checks `sf` (≥ 2.150.6), then the app jumps to **Orgs**.
2. **Orgs**: your default Dev Hub and org are preselected. Select a row and
   press `h` (use as Dev Hub) or `o` (use as target org).
3. **Access** (the Access Matrix): one row per org, one column per thing GP
   Atlas can read (2GP packages, 2GP versions, 2GP builds, installed
   packages, 1GP versions). `a` probes all orgs, `A` re-tests all, `p`
   re-tests the selected org, `Enter` on a cell explains the result (error,
   rule, a fix to copy). Results are kept for **15 minutes**, also after you
   close the app, so `a` only probes what is missing or older; timeouts and
   network errors are always retried. The cache is a JSON file in your OS
   cache folder (`~/Library/Caches/gp-atlas` on macOS, `%LOCALAPPDATA%\gp-atlas`
   on Windows, `~/.cache/gp-atlas` on Linux; `GP_ATLAS_CACHE_DIR` overrides
   it) holding states and the already-scrubbed error text, never tokens.
   `--no-cache` turns it off.
4. **2GP Packages & Versions**: packages on the left, versions on the right.
   Click a package (or move with ↑↓) to show its versions; `‹ All packages ›`
   shows everything. ←/→ or a click switches pane. Filters: `R` released,
   `L` latest per package, `V` verbose. On a version: `c` copy 04t,
   `i` copy install link, `Enter` details.
   - `a` **ancestry**: the package's version tree (released versions), with the
     selected version and its path to the root highlighted. `A` = whole tree.
   - `d` **dependencies**: what to install first, in install order (`t` for a
     tree), and whether each one is already in the selected org. If `sf`
     cannot build the graph (`InvalidDependencyGraphError`, usually a
     dependency from another publisher or Dev Hub), GP Atlas explains why and
     shows the direct dependencies from the version details instead.
   - **Project aliases:** start `gp-atlas` inside an SFDX project (or pass
     `--project <folder>`; `--no-project` to skip). An **Alias** column then
     shows each package's and version's alias from `sfdx-project.json`
     (yellow: several aliases, or the CLI disagrees). The header shows
     `Project: <folder> · N aliases ⚠ W`; `P` lists the warnings: orphan aliases
     (ID not in this Dev Hub), packages/versions without an alias, duplicates,
     ignored entries. `s` copies a `packageAliases` entry for the selected row,
     `S` entries for every row without an alias, named like the CLI does
     (`MyApp@1.2.0-3`). GP Atlas never edits `sfdx-project.json`.
5. **Installed** and **1GP Versions** use the selected org. On 1GP Versions
   (lists can have thousands of rows): `R` released only, `L` latest released
   per package (computed by GP Atlas: numeric version, then build), `f` only
   the selected row's package (033; `f` again to clear), `x` clear filters.
6. **History** lists every `sf` call with exit code, time and stderr.
7. **Command Builder** (tab 8): pick a read-only command on the left; the
   form on the right comes from the CLI's own flag list (required fields
   marked `*`, Dev Hub / org first). Empty fields are filled from what you
   selected elsewhere (Dev Hub, org, 2GP package or version; `f` refills).
   `Enter` / `Space` edits a value, toggles a switch or cycles a choice;
   `Del` clears it; invalid values show `⚠` and the reason. The preview below
   is the exact command: `s` shell (bash/zsh, PowerShell, cmd), `J` `--json`,
   `F` one-letter flags, `M` one flag per line, `U` alias ↔ username,
   `A` package ID ↔ project alias (adds `cd <project>` because aliases only
   resolve inside the project). `c` copies it; `R` runs it in GP Atlas
   (read-only commands only; the result opens in a popup and the call goes
   to History). Flags not offered yet are listed under the preview.

Options: `--no-mouse` (select text normally), `--project <folder>` /
`--no-project` (project aliases), `--no-cache` (Access Matrix cache),
`--sf <path>`, `--timeout <secs>`. `gp-atlas --help` lists them all.

## Command-line debug tool (`gp-atlas-beta`)

A command-line **debug build** of GP Atlas. It runs on the same core as the
terminal UI: the version gate, the read-only command set, the runner,
error classification and the Access Matrix. Its two jobs:

1. Let you try GP Atlas against your real orgs today.
2. Collect sanitized `sf` output (`capture`) so the remaining unknowns
   (SPEC §4.7: U5 error cases, U7 1GP in subscriber orgs) can be settled.

> Not affiliated with or endorsed by Salesforce. **Read-only**: it can only run
> the commands listed in SPEC §4.4 and never changes an org, an alias, the
> CLI config or `sfdx-project.json`.

## Requirements

- Salesforce CLI **2.150.6 or newer** (`sf version`). Older versions are refused.
  On a newer CLI, `doctor` lists any command that changed since 2.150.6; only
  that command is disabled.
- macOS (Apple Silicon), Windows x64 or Linux x64.

## Install

Three ways. Every code block below can be pasted as is into bash, zsh or
PowerShell where noted (no inline `#` comments: zsh treats them as arguments
unless `setopt interactivecomments` is on).

### Download (no Rust needed)

1. Open [Actions → Beta binaries](https://github.com/holapancho/gp-atlas/actions/workflows/beta.yml)
   (you must be signed in to GitHub to download artifacts).
2. Open the latest run with a green check, scroll to **Artifacts**, download
   `gp-atlas-beta-macos-arm64`, `gp-atlas-beta-linux-x64` or
   `gp-atlas-beta-windows-x64`. Artifacts are kept 30 days.
3. Unzip it; it contains another zip: unzip that too. You get `gp-atlas`,
   `gp-atlas-beta`, this guide and the license.

macOS: the binaries are not signed yet, so remove the quarantine flag once
(run this in the unzipped folder):

```bash
xattr -d com.apple.quarantine gp-atlas gp-atlas-beta
chmod +x gp-atlas gp-atlas-beta
./gp-atlas
```

Windows: run `.\gp-atlas.exe` in PowerShell (SmartScreen may ask once,
because the binary is not signed).

### Clone and use

An optimized build from source, for day-to-day use. Prerequisites: see
[step 1 below](#build-a-debug-version-on-another-machine) (C linker and Rust).

```bash
git clone https://github.com/holapancho/gp-atlas.git
cd gp-atlas
rustup toolchain install
cargo build --release -p gp-atlas-tui -p gp-atlas-cli
./target/release/gp-atlas
```

To run `gp-atlas` from any folder (it lands in `~/.cargo/bin`):

```bash
cargo install --path crates/gp-atlas-tui
cargo install --path crates/gp-atlas-cli
gp-atlas
```

Update: `git pull`, then the same `cargo build` (or `cargo install`) lines.
Start it inside an SFDX project folder to get the project aliases.

### Clone and beta-test

A debug build: readable panic backtraces, the `gp-atlas-beta` debug tool,
and fixture capture. Full steps in
[Build a debug version on another machine](#build-a-debug-version-on-another-machine);
in short:

```bash
git clone https://github.com/holapancho/gp-atlas.git
cd gp-atlas
rustup toolchain install
cargo build -p gp-atlas-tui -p gp-atlas-cli
RUST_BACKTRACE=1 ./target/debug/gp-atlas
```

Then, to report something: `./target/debug/gp-atlas-beta --debug doctor`, or
[help with the missing fixtures](#helping-with-the-missing-fixtures).

## Build a debug version on another machine

A local debug build needs no signing or quarantine steps, gives readable
panic backtraces, and lets you rebuild after `git pull`.

**1. Prerequisites (once per machine)**

| OS | Install |
|---|---|
| macOS | `xcode-select --install` (C linker), then Rust: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Linux | `build-essential` (or your distro's gcc + make), then Rust with the same `rustup` command |
| Windows | [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with "Desktop development with C++", then Rust from [rustup.rs](https://rustup.rs) (`rustup-init.exe`) |

Plus `git`, Node.js ≥ 22 and `sf` ≥ 2.150.6 (`npm install --global @salesforce/cli@latest`).

**2. Get the code and build**

```bash
git clone https://github.com/holapancho/gp-atlas.git
cd gp-atlas
rustup toolchain install
cargo build -p gp-atlas-tui -p gp-atlas-cli
```

`rustup toolchain install` installs the pinned Rust (`rust-toolchain.toml`);
`cargo build` without `--release` is the debug build.

The programs are `target/debug/gp-atlas` and `target/debug/gp-atlas-beta`
(`.exe` on Windows). If the repository is private, clone with an account that has
access (`gh auth login`, or an SSH key).

**3. Run with debugging on**

macOS / Linux (the History tab in the UI shows every `sf` call; the last
line builds and runs the debug tool in one step):

```bash
RUST_BACKTRACE=1 ./target/debug/gp-atlas
RUST_BACKTRACE=1 ./target/debug/gp-atlas-beta --debug doctor
RUST_BACKTRACE=1 cargo run -p gp-atlas-cli -- --debug orgs
```

Windows PowerShell:

```powershell
$env:RUST_BACKTRACE = "1"
.\target\debug\gp-atlas.exe
.\target\debug\gp-atlas-beta.exe --debug doctor
```

- `--debug` prints every `sf` call (argv, exit code, duration, stderr).
- `--raw` prints the JSON GP Atlas received instead of a table.
- `RUST_BACKTRACE=1` adds a full backtrace if the tool panics.
- `--sf /path/to/sf` (or `GP_ATLAS_SF_BIN`) picks a specific `sf`, handy to
  compare two CLI versions side by side.

**4. Update later**

```bash
git pull && cargo build -p gp-atlas-tui -p gp-atlas-cli
```

**5. Optional: run the test suite on that machine**

Unit and fake-`sf` tests (no org needed), then the contract tests against
`sf` 2.150.6 exactly, then against any newer `sf`:

```bash
cargo test
cargo test -p gp-atlas-contract-tests -- --ignored
GP_ATLAS_CONTRACT_MODE=compat cargo test -p gp-atlas-contract-tests -- --ignored
```

A release (optimized) build is `cargo build --release -p gp-atlas-tui -p gp-atlas-cli`
→ `target/release/`. To run them from any folder:
`cargo install --path crates/gp-atlas-tui` and `cargo install --path crates/gp-atlas-cli`.

## Commands

| Command | What it does |
|---|---|
| `gp-atlas-beta doctor` | D1–D4: `sf` found, version ≥ 2.150.6, bundled packaging plugin, command contract (lists commands that changed in a newer CLI) |
| `gp-atlas-beta orgs` | Your orgs as `alias — username`, type, Dev Hub flag, connection status |
| `gp-atlas-beta probe` | Access Matrix for all orgs (`--org X` to limit, `--try-anyway` for 2GP on non-hubs, `--details` for raw errors) |
| `gp-atlas-beta packages --hub X` | 2GP packages in a Dev Hub |
| `gp-atlas-beta versions --hub X` | 2GP versions (`--released`, `--branch`, `--created-last-days`, `--modified-last-days`, `--order-by "CreatedDate DESC"`, `--packages 0Ho…`, `--verbose`, `--latest`) |
| `gp-atlas-beta report --hub X --package 04t…` | One version's details |
| `gp-atlas-beta builds --hub X` | Version-create requests (`--status`, `--created-last-days`) |
| `gp-atlas-beta installed --org X` | Installed packages |
| `gp-atlas-beta pkg1 --org X` | 1GP versions (`--package-id 033…`, 18 chars) |
| `gp-atlas-beta project [--dir D] [--hub X]` | Package aliases in `sfdx-project.json`; with `--hub`, orphan aliases and how many packages/versions have none |
| `gp-atlas-beta show probes --org X` | Print the probe commands without running them |
| `gp-atlas-beta capture --out DIR` | Run everything and save **sanitized** output for the fixtures |

Global options: `--debug` (print every `sf` call, exit code, timing, stderr),
`--raw` (print the JSON instead of a table), `--timeout SECS`, `--sf PATH`.

## Helping with the missing fixtures

```bash
gp-atlas-beta doctor
gp-atlas-beta capture --out gp-atlas-capture
```

`capture` runs the inventory, the Dev Hub listings and the Access Matrix probes
on **every** org (2GP probes also on non-Dev-Hub orgs, which is how the
"not a Dev Hub", expired-session and 1GP-in-subscriber-org cases get recorded).
With ~50 orgs it takes a few minutes; `--max-orgs N` limits it.

It replaces usernames, aliases, IDs, hosts, names, namespaces, descriptions,
branches and tags with fakes, removes secrets and drops stack traces.
**Review the folder before sending** (see its `README.txt`), then zip it and
attach it. `summary.md` inside shows the Access Matrix and every distinct
failure with the rule that classified it.

Optional extra cases, if you can:

- Wi-Fi off, then `gp-atlas-beta --debug probe --org <hub>` (network error).
- A user on a minimal profile, and one without "API Enabled", logged in to
  `sf`, then `capture` again (or `probe --org <alias> --details`).

## Reporting a problem

Run the failing command again with `--debug` and send the output. The debug
output shows exact `sf` commands and their stderr; it contains your usernames
and aliases, so redact them first if needed.
