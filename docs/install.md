---
title: Install
nav_order: 2
---

# Install

GP Atlas is two programs, installed together:

| Program | What it is |
|---|---|
| `gp-atlas` | The app: a clickable terminal UI. |
| `gp-atlas-beta` | A command-line debug tool (one command per action, `--debug`, fixture capture). See [Beta testing](beta-testing.md). |

First make sure the Salesforce CLI is there (Node.js 22 or newer):

```bash
npm install --global @salesforce/cli@latest
sf version
```

`sf version` must show `@salesforce/cli/2.150.6` or newer.

Then pick one way. Command blocks can be pasted as they are (they contain no
`#` comments, which zsh would treat as arguments).

## Install script (recommended)

Downloads the latest release for your system, checks its SHA-256 checksum and
puts both programs in a folder on your `PATH`.

**macOS / Linux** (installs to `~/.local/bin`):

```bash
curl -fsSL https://raw.githubusercontent.com/holapancho/gp-atlas/main/install.sh | sh
```

**Windows PowerShell** (installs to `%LOCALAPPDATA%\gp-atlas\bin` and adds it
to your user `PATH`):

```powershell
irm https://raw.githubusercontent.com/holapancho/gp-atlas/main/install.ps1 | iex
```

Options: a specific version with `GP_ATLAS_VERSION=v0.1.0`, another folder
with `GP_ATLAS_INSTALL_DIR=/some/dir` (both as environment variables). You can
read both scripts first: [install.sh](https://github.com/holapancho/gp-atlas/blob/main/install.sh),
[install.ps1](https://github.com/holapancho/gp-atlas/blob/main/install.ps1).

Run it:

```bash
gp-atlas
```

If your shell says `command not found`, open a new terminal, or add
`~/.local/bin` to your `PATH`.

## Download a release by hand

Open [Releases](https://github.com/holapancho/gp-atlas/releases/latest) and
download the file for your system:

| System | File |
|---|---|
| macOS, Apple Silicon | `gp-atlas-aarch64-apple-darwin.tar.gz` |
| macOS, Intel | `gp-atlas-x86_64-apple-darwin.tar.gz` |
| Linux x64 | `gp-atlas-x86_64-unknown-linux-musl.tar.gz` |
| Windows x64 | `gp-atlas-x86_64-pc-windows-msvc.zip` |

`SHA256SUMS` lists the checksum of each file. Unpack it and run `./gp-atlas`
(`.\gp-atlas.exe` on Windows).

The binaries are not signed yet. On macOS, a file downloaded with a browser is
quarantined: remove the flag once, in the unpacked folder:

```bash
xattr -d com.apple.quarantine gp-atlas gp-atlas-beta
```

On Windows, SmartScreen may ask once ("More info", then "Run anyway").

## Build from source with Cargo

Needs Rust ([rustup.rs](https://rustup.rs)) and a C linker (macOS:
`xcode-select --install`; Linux: `build-essential` or your distribution's gcc;
Windows: Visual Studio Build Tools with "Desktop development with C++").

Install straight from GitHub (both programs land in `~/.cargo/bin`):

```bash
cargo install --locked --git https://github.com/holapancho/gp-atlas gp-atlas-tui gp-atlas-cli
```

Or clone and build:

```bash
git clone https://github.com/holapancho/gp-atlas.git
cd gp-atlas
cargo build --release --locked -p gp-atlas-tui -p gp-atlas-cli
./target/release/gp-atlas
```

The pinned Rust version (`rust-toolchain.toml`) is installed automatically by
`rustup` on the first build. Update later with `git pull` and the same build
line, or by running `cargo install` again.

## Update and uninstall

- Install script: run it again. Uninstall: delete `gp-atlas` and
  `gp-atlas-beta` from `~/.local/bin` (Windows: the
  `%LOCALAPPDATA%\gp-atlas` folder).
- Cargo: `cargo install` again; `cargo uninstall gp-atlas-tui gp-atlas-cli`.
- GP Atlas keeps one cache file (Access Matrix results, 15 minutes):
  `~/Library/Caches/gp-atlas` (macOS), `~/.cache/gp-atlas` (Linux),
  `%LOCALAPPDATA%\gp-atlas` (Windows). It is safe to delete.

Next: the [user guide](guide.md).
