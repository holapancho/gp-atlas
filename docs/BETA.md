# GP Atlas beta (`gp-atlas-beta`)

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

- Salesforce CLI **exactly 2.150.6** (`sf version`). Other versions are refused.
- macOS (Apple Silicon), Windows x64 or Linux x64.

## Install

**From GitHub Actions (no Rust needed):**
open the repository's *Actions* tab → *Beta binaries* → the latest run →
*Artifacts* → download `gp-atlas-beta-<your OS>`. Unzip it (it contains another zip; unzip that too).

On macOS the download is quarantined because the binary is not signed yet:

```bash
cd ~/Downloads/gp-atlas-beta-macos-arm64     # wherever you unzipped it
xattr -d com.apple.quarantine gp-atlas-beta
chmod +x gp-atlas-beta
./gp-atlas-beta --help
```

**From source (needs Rust):**

```bash
git clone https://github.com/holapancho/gp-atlas && cd gp-atlas
cargo build --release -p gp-atlas-cli
./target/release/gp-atlas-beta --help
```

## Commands

| Command | What it does |
|---|---|
| `gp-atlas-beta doctor` | D1–D4: `sf` found, exact version, bundled packaging plugin, command contract |
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
