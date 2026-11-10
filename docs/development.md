---
title: Development
nav_order: 6
---

# Development

The Rust toolchain is pinned in `rust-toolchain.toml`; `rustup` installs it on
the first build. The full design is in
[SPEC.md](https://github.com/holapancho/gp-atlas/blob/main/SPEC.md).

## Build and check

```bash
cargo build
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo deny check
```

`cargo test` needs no org and no `sf`: it uses `fake-sf` and the fixtures.

## Repository layout

| Path | Purpose |
|---|---|
| `crates/gp-atlas-core` | UI-free core: CLI contract, manifest, blocklist, validated inputs, `ReadOnlyCommand`, runner, JSON parsing and secret scrubbing, error classification, Doctor, orgs, probes and their cache, project aliases, graphs, Command Builder and shell quoting. |
| `crates/gp-atlas-tui` | The `gp-atlas` app (ratatui). |
| `crates/gp-atlas-cli` | `gp-atlas-beta`: debug tool and sanitized fixture capture. |
| `manifest/sf-2.150.6.json` | **Generated** from `sf commands --json`. Never edit it by hand. |
| `tools/extract-manifest` | Regenerates the baseline manifest from an installed `sf` 2.150.6. |
| `fixtures/sf-2.150.6/` | Sanitized real `sf` outputs used by tests ([README](https://github.com/holapancho/gp-atlas/blob/main/fixtures/sf-2.150.6/README.md)). |
| `tests/fake-sf` | Test double for `sf` that replays fixtures by argv. |
| `tests/contract` | Contract tests against a real `sf` (CI jobs `sf-contract` and `sf-latest`). |
| `docs/` | This site (GitHub Pages). `docs/ideas/` holds design notes that are not published. |

## Contract tests

Against `sf` 2.150.6 exactly (on `PATH` or `GP_ATLAS_SF_BIN`), then against
any newer `sf`:

```bash
cargo test -p gp-atlas-contract-tests -- --ignored
GP_ATLAS_CONTRACT_MODE=compat cargo test -p gp-atlas-contract-tests -- --ignored
```

## Regenerating the manifest

```bash
npm install --global @salesforce/cli@2.150.6
cargo run -p extract-manifest -- --out manifest/sf-2.150.6.json
```

The tool refuses any other CLI version. CI regenerates the manifest and fails
if it differs from the committed copy.

## Using the fake `sf`

```bash
cargo build -p fake-sf
GP_ATLAS_SF_BIN=target/debug/fake-sf cargo run -p gp-atlas-tui
```

`fake-sf` reads `fixtures/sf-2.150.6/index.json` (override with
`GP_ATLAS_FAKE_SF_INDEX`) and can log every call to `GP_ATLAS_FAKE_SF_LOG`.
See `tests/fake-sf/src/main.rs` for the index format.

## Documentation screenshots

The images in `docs/assets/images/` are real `gp-atlas` screens over fictional
demo data (Acme), captured in a virtual terminal. After a UI change, rebuild
and regenerate them:

```bash
cargo build -p gp-atlas-tui -p gp-atlas-cli -p fake-sf
pip install pyte
tools/screenshots/make.sh
```

It needs `node` with Playwright and Chromium. The demo data is in
`tools/screenshots/demo.py`, the key presses for each screen in
`tools/screenshots/shots.json`.

## CI

| Workflow | When | What |
|---|---|---|
| `ci.yml` | every push and PR | fmt, clippy, tests on Linux, macOS, Windows; `sf-contract` (2.150.6); `sf-latest` (non-blocking); `cargo deny`; `cargo audit` |
| `beta.yml` | push to `main` | Pre-release binaries as workflow artifacts |
| `release.yml` | tag `v*` | Release binaries, `SHA256SUMS`, GitHub Release |

## Releasing

1. Set `version` in the root `Cargo.toml` (`[workspace.package]`) and run
   `cargo build` so `Cargo.lock` follows.
2. Move the "Unreleased" notes in `CHANGELOG.md` under the new version.
3. Commit, then tag and push the tag:

```bash
git tag v0.1.0
git push origin v0.1.0
```

`release.yml` checks that the tag matches the `Cargo.toml` version, builds
`gp-atlas` and `gp-atlas-beta` for macOS (Apple Silicon and Intel), Linux x64
and Windows x64, and publishes them with `SHA256SUMS` on a GitHub Release. The
install scripts always fetch the latest release.

## GitHub Pages

The site is published by GitHub Pages itself (**Settings → Pages → Deploy
from a branch: `main`, `/docs`**): every push to `main` that changes `docs/`
rebuilds <https://holapancho.github.io/gp-atlas/> within a couple of minutes
(the "pages build and deployment" run in Actions). The theme is Just the Docs
(`remote_theme` in `docs/_config.yml`); custom styles are in
`docs/_sass/custom/custom.scss`.
