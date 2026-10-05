# GP Atlas beta

Two programs, both read-only and built on the same core:

| Program | What it is |
|---|---|
| **`gp-atlas-tui`** | The **clickable terminal UI**: tabs, tables, popups. Use this day to day. |
| `gp-atlas-beta` | Command-line debug tool: one command per action, `--debug`, and `capture` for fixtures. |

## Terminal UI (`gp-atlas-tui`)

```bash
./target/debug/gp-atlas-tui            # if you built it yourself (see below)
./gp-atlas-tui                         # if you downloaded it
```

It opens full-screen in your terminal. Click a tab or a row (click the selected
row again to open it), scroll with the wheel, or use the keyboard; `?` shows
all keys, `q` quits.

1. **Doctor** checks `sf` (≥ 2.150.6), then the app jumps to **Orgs**.
2. **Orgs**: your default Dev Hub and org are preselected. Select a row and
   press `h` (use as Dev Hub) or `o` (use as target org).
3. **Access**: `a` probes all orgs (`p` just the selected one) and shows what
   each org lets you read; `Enter` on a cell explains the result.
4. **2GP Packages & Versions**: packages on the left, versions on the right.
   Click a package (or move with ↑↓) to show its versions; `‹ All packages ›`
   shows everything. ←/→ or a click switches pane. Filters: `R` released,
   `L` latest per package, `V` verbose. On a version: `c` copy 04t,
   `i` copy install link, `Enter` details.
   - `a` **ancestry**: the package's version tree (released versions), with the
     selected version and its path to the root highlighted. `A` = whole tree.
   - `d` **dependencies**: what to install first, in install order (`t` for a
     tree), and whether each one is already in the selected org.
5. **Installed** and **1GP Versions** use the selected org.
6. **History** lists every `sf` call with exit code, time and stderr.

`--no-mouse` turns off mouse capture so you can select text normally.

## Command-line debug tool (`gp-atlas-beta`)

A command-line **debug build** of GP Atlas. It runs on the same core the desktop
app will use: the version gate, the read-only command set, the runner,
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

**From GitHub Actions (no Rust needed):**
open the repository's *Actions* tab → *Beta binaries* → the latest run →
*Artifacts* → download `gp-atlas-beta-<your OS>`. Unzip it (it contains another zip; unzip that too).

On macOS the download is quarantined because the binary is not signed yet:

```bash
cd ~/Downloads/gp-atlas-beta-macos-arm64     # wherever you unzipped it
xattr -d com.apple.quarantine gp-atlas-tui gp-atlas-beta
chmod +x gp-atlas-tui gp-atlas-beta
./gp-atlas-tui
```

**From source:** see [Build a debug version on another machine](#build-a-debug-version-on-another-machine).

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
git clone https://github.com/holapancho/gp-atlas
cd gp-atlas
rustup toolchain install          # installs the pinned Rust (rust-toolchain.toml)
cargo build -p gp-atlas-tui -p gp-atlas-cli   # debug build (default profile)
```

The programs are `target/debug/gp-atlas-tui` and `target/debug/gp-atlas-beta`
(`.exe` on Windows). If the repository is private, clone with an account that has
access (`gh auth login`, or an SSH key).

**3. Run with debugging on**

```bash
# macOS / Linux
RUST_BACKTRACE=1 ./target/debug/gp-atlas-tui          # clickable UI; History tab shows every sf call
RUST_BACKTRACE=1 ./target/debug/gp-atlas-beta --debug doctor
# or build-and-run in one step
RUST_BACKTRACE=1 cargo run -p gp-atlas-cli -- --debug orgs
```

```powershell
# Windows PowerShell
$env:RUST_BACKTRACE = "1"
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

```bash
cargo test                                            # unit + fake-sf tests, no org needed
cargo test -p gp-atlas-contract-tests -- --ignored    # needs sf 2.150.6 exactly
GP_ATLAS_CONTRACT_MODE=compat cargo test -p gp-atlas-contract-tests -- --ignored   # any newer sf
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
