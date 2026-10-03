# GP Atlas

*Browse 1GP and 2GP package versions and build `sf` commands.*

GP Atlas is a small, cross-platform desktop companion to the Salesforce CLI.
It lets a Salesforce developer browse second-generation (2GP) and
first-generation (1GP) package versions, see what each org lets them read, and
build exact, correctly quoted `sf` commands to copy and run themselves.

> **Not affiliated with or endorsed by Salesforce.** Salesforce and related
> marks are trademarks of Salesforce, Inc. GP Atlas is an independent tool that
> uses your locally installed Salesforce CLI as its only data source.

**Status:** early development. The desktop UI is not implemented yet; a
command-line **beta** (`gp-atlas-beta`) runs on the same core: Doctor, org
inventory, Access Matrix, package/version listings and fixture capture. See
[`docs/BETA.md`](docs/BETA.md) to try it, and [`SPEC.md`](SPEC.md) for the
full specification and milestone plan.

## Read-only guarantee

GP Atlas never runs a state-changing `sf` command. It can only run a closed
set of read-only commands (listing packages, versions, installed packages,
orgs and aliases). Anything that creates, promotes, deletes, installs,
uninstalls, converts, retrieves, schedules, logs in/out or changes
configuration is never executed. At most, GP Atlas shows such a command for
you to copy. It makes no network calls of its own, never reads auth files,
never runs `sf org display`, and never stores tokens or installation keys.

## Requirements

GP Atlas supports **exactly one** Salesforce CLI version: **`@salesforce/cli` 2.150.6**
(bundling `@salesforce/plugin-packaging` 3.0.6). Any other version is
rejected, because flags, JSON output and error shapes change between releases.

```bash
# Node.js >= 22 is required by @salesforce/cli@2.150.6
npm install --global @salesforce/cli@2.150.6
sf version --json   # "cliVersion": "@salesforce/cli/2.150.6"
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
| `crates/gp-atlas-cli` | `gp-atlas-beta`: command-line debug frontend and sanitized fixture capture ([docs/BETA.md](docs/BETA.md)). |
| `crates/gp-atlas-egui` | The `gp-atlas` desktop binary (egui UI, not started yet). |
| `manifest/sf-2.150.6.json` | **Generated** from `sf commands --json`. Never edit it by hand. |
| `tools/extract-manifest` | Regenerates the manifest from an installed `sf` 2.150.6. |
| `fixtures/sf-2.150.6/` | Sanitized real `sf` outputs used by tests. |
| `tests/fake-sf` | Test double for `sf` that replays fixtures by argv. |
| `tests/contract` | Contract tests against a real `sf` 2.150.6 (CI job `sf-contract`). |

### Regenerating the manifest

```bash
npm install --global @salesforce/cli@2.150.6
cargo run -p extract-manifest -- --out manifest/sf-2.150.6.json
```

The tool refuses to run against any other CLI version. CI regenerates the
manifest and fails if it differs from the committed copy.

### Contract tests

```bash
cargo test -p gp-atlas-contract-tests -- --ignored   # needs sf 2.150.6 on PATH or GP_ATLAS_SF_BIN
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
