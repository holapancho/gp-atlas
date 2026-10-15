# GP Atlas

*Browse 1GP and 2GP package versions and build `sf` commands.*

GP Atlas is a small, cross-platform **clickable terminal app** that works
alongside the Salesforce CLI.
It lets a Salesforce developer browse second-generation (2GP) and
first-generation (1GP) package versions, see what each org lets them read, and
build exact, correctly quoted `sf` commands to copy and run themselves.

> **Not affiliated with or endorsed by Salesforce.** Salesforce and related
> marks are trademarks of Salesforce, Inc. GP Atlas is an independent tool that
> uses your locally installed Salesforce CLI as its only data source.

**Status:** beta. The app is the clickable **terminal UI** (`gp-atlas`:
mouse and keyboard). A command-line debug tool (`gp-atlas-beta`) runs on the
same core. [`docs/BETA.md`](docs/BETA.md) is the user guide;
[`SPEC.md`](SPEC.md) is the full specification and milestone plan.

## Quick start

You need the Salesforce CLI **2.150.6 or newer** (`sf version`) and at least
one org you are logged in to (`sf org list`). Pick one way:

| I want to… | Do this |
|---|---|
| **Use it, no Rust** | [Download the beta binaries](https://github.com/holapancho/gp-atlas/actions/workflows/beta.yml) → latest green run → *Artifacts* → `gp-atlas-beta-<your OS>` ([details](docs/BETA.md#download-no-rust-needed)) |
| **Clone and use** (optimized build) | [Clone and use](docs/BETA.md#clone-and-use) |
| **Clone and beta-test** (debug build, backtraces, debug tool) | [Clone and beta-test](docs/BETA.md#clone-and-beta-test) |

Clone and use, in short (needs [Rust](https://rustup.rs) and `git`):

```bash
git clone https://github.com/holapancho/gp-atlas.git
cd gp-atlas
cargo build --release -p gp-atlas-tui -p gp-atlas-cli
./target/release/gp-atlas
```

On Windows run `.\target\release\gp-atlas.exe`. Inside the app: `1` Doctor
checks `sf`, `2` Orgs (`h` picks the Dev Hub, `o` the org), `4` packages and
versions, `8` Command Builder, `?` all keys, `q` quits. Update later with
`git pull` and the same `cargo build` line.

| Area | State |
|---|---|
| Doctor: `sf` found, version ≥ 2.150.6, packaging plugin, command contract | ✅ |
| Orgs: Dev Hubs and other orgs, default hub/org | ✅ |
| Access Matrix: what each org lets you read, with reasons; results kept 15 min across restarts (`A` re-tests) | ✅ |
| 2GP packages & versions: split view, filters, latest released, details, copy 04t / install link | ✅ |
| 2GP ancestry tree (`a` / `A`), checked against a real capture | ✅ |
| 2GP dependencies in install order with "In org" (`d`); falls back to direct dependencies when `sf` cannot build the graph | ✅ (a real successful graph capture is still wanted) |
| Installed packages, History of every `sf` call | ✅ |
| 1GP versions with released / latest-per-package / package filters | ✅ |
| Project aliases from `sfdx-project.json`: Alias column, orphan / duplicate / unaliased warnings, copyable `packageAliases` snippets | ✅ |
| Copy IDs: `c` or right-click a row (OS clipboard, or the terminal's over SSH) | ✅ |
| Command Builder (tab 8): read-only commands from manifest-generated forms, validated fields, live preview quoted for bash/zsh, PowerShell or cmd; copy, or run in GP Atlas | ✅ |
| Installation-key prompt for key-protected versions | ⏳ not started |
| Signed release builds | ⏳ not started; unsigned beta binaries come from CI |
| Write actions (create / promote / delete versions) and push upgrades (2GP CLI, 1GP Apex) | 💡 idea only: [docs/ideas/WRITE_ACTIONS_AND_PUSH_UPGRADES.md](docs/ideas/WRITE_ACTIONS_AND_PUSH_UPGRADES.md); GP Atlas stays read-only |

## Read-only guarantee

GP Atlas never runs a state-changing `sf` command. It can only run a closed
set of read-only commands (listing packages, versions, installed packages,
orgs and aliases). Anything that creates, promotes, deletes, installs,
uninstalls, converts, retrieves, schedules, logs in/out or changes
configuration is never executed. At most, GP Atlas shows such a command for
you to copy. It makes no network calls of its own, never reads auth files,
never runs `sf org display`, and never stores tokens or installation keys.

## Requirements

GP Atlas requires **`@salesforce/cli` 2.150.6 or newer**. Older versions are
rejected. 2.150.6 is the *baseline*: the command manifest and test fixtures are
generated from it. On a newer CLI, GP Atlas compares each command it uses with
the baseline and disables only a command whose flags changed (Doctor shows
which). CI also checks the latest CLI release against the baseline.

```bash
# Node.js >= 22
npm install --global @salesforce/cli@latest
sf version --json   # "cliVersion" must be @salesforce/cli/2.150.6 or newer
```

GP Atlas locates `sf` via the `GP_ATLAS_SF_BIN` environment variable, then the
app settings, then `PATH`.

## Building

The Rust toolchain is pinned in `rust-toolchain.toml`.

```bash
cargo build
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Repository layout

| Path | Purpose |
|---|---|
| `crates/gp-atlas-core` | UI-free core: CLI contract, manifest, blocklist, validated inputs, `ReadOnlyCommand`, runner, JSON parsing and secret scrubbing, error classification, Doctor, orgs, probes. |
| `crates/gp-atlas-tui` | `gp-atlas`: clickable terminal UI (ratatui). |
| `crates/gp-atlas-cli` | `gp-atlas-beta`: command-line debug frontend and sanitized fixture capture ([docs/BETA.md](docs/BETA.md)). |
| `manifest/sf-2.150.6.json` | **Generated** from `sf commands --json`. Never edit it by hand. |
| `tools/extract-manifest` | Regenerates the baseline manifest from an installed `sf` 2.150.6. |
| `fixtures/sf-2.150.6/` | Sanitized real `sf` outputs used by tests. |
| `tests/fake-sf` | Test double for `sf` that replays fixtures by argv. |
| `tests/contract` | Contract tests against a real `sf`: the 2.150.6 baseline (CI job `sf-contract`) and the latest release (`sf-latest`, `GP_ATLAS_CONTRACT_MODE=compat`). |

### Regenerating the manifest

```bash
npm install --global @salesforce/cli@2.150.6
cargo run -p extract-manifest -- --out manifest/sf-2.150.6.json
```

The tool refuses to run against any other CLI version. CI regenerates the
manifest and fails if it differs from the committed copy.

### Contract tests

```bash
cargo test -p gp-atlas-contract-tests -- --ignored   # baseline: needs sf 2.150.6 on PATH or GP_ATLAS_SF_BIN
GP_ATLAS_CONTRACT_MODE=compat cargo test -p gp-atlas-contract-tests -- --ignored   # any newer sf
```

### Using the fake `sf`

```bash
cargo build -p fake-sf
GP_ATLAS_SF_BIN=target/debug/fake-sf ...
```

`fake-sf` reads `fixtures/sf-2.150.6/index.json` (override with
`GP_ATLAS_FAKE_SF_INDEX`) and can log every invocation to
`GP_ATLAS_FAKE_SF_LOG`. See `tests/fake-sf/src/main.rs` for the index format.

## License

Licensed under the [MIT license](LICENSE).
