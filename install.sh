#!/bin/sh
# GP Atlas installer for macOS and Linux.
#
#   curl -fsSL https://raw.githubusercontent.com/holapancho/gp-atlas/main/install.sh | sh
#
# Downloads the release archive for this system from GitHub Releases, checks
# its SHA-256 against the release's SHA256SUMS, and copies `gp-atlas` and
# `gp-atlas-beta` into GP_ATLAS_INSTALL_DIR (default: ~/.local/bin).
#
# Environment:
#   GP_ATLAS_VERSION      a tag such as v0.1.0 (default: the latest release)
#   GP_ATLAS_INSTALL_DIR  where to put the programs (default: ~/.local/bin)
#   GP_ATLAS_DOWNLOAD_URL base URL of the release files (for testing mirrors)
#
# It never uses sudo and touches nothing outside the install folder and a
# temporary folder that it removes.

set -eu

REPO="holapancho/gp-atlas"
VERSION="${GP_ATLAS_VERSION:-latest}"
INSTALL_DIR="${GP_ATLAS_INSTALL_DIR:-$HOME/.local/bin}"

say() { printf '%s\n' "gp-atlas install: $*"; }
die() { printf '%s\n' "gp-atlas install: error: $*" >&2; exit 1; }

case "$VERSION" in
  latest) BASE="https://github.com/$REPO/releases/latest/download" ;;
  v[0-9]*) BASE="https://github.com/$REPO/releases/download/$VERSION" ;;
  *) die "GP_ATLAS_VERSION must look like v0.1.0 (got '$VERSION')" ;;
esac
BASE="${GP_ATLAS_DOWNLOAD_URL:-$BASE}"

os="$(uname -s)"
arch="$(uname -m)"
case "$os/$arch" in
  Darwin/arm64 | Darwin/aarch64) target="aarch64-apple-darwin" ;;
  Darwin/x86_64) target="x86_64-apple-darwin" ;;
  Linux/x86_64 | Linux/amd64) target="x86_64-unknown-linux-musl" ;;
  *) die "no prebuilt binary for $os $arch; build from source: cargo install --locked --git https://github.com/$REPO gp-atlas-tui gp-atlas-cli" ;;
esac
asset="gp-atlas-$target.tar.gz"

fetch() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL --retry 3 -o "$2" "$1"
  elif command -v wget >/dev/null 2>&1; then
    wget -q -O "$2" "$1"
  else
    die "needs curl or wget"
  fi
}

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d ' ' -f 1
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | cut -d ' ' -f 1
  else
    die "needs sha256sum or shasum to verify the download"
  fi
}

tmp="$(mktemp -d 2>/dev/null || mktemp -d -t gp-atlas)"
trap 'rm -rf "$tmp"' EXIT INT TERM

say "downloading $asset ($VERSION)"
if ! fetch "$BASE/$asset" "$tmp/$asset"; then
  die "could not download $BASE/$asset. If no release is published yet, build from source: cargo install --locked --git https://github.com/$REPO gp-atlas-tui gp-atlas-cli"
fi
fetch "$BASE/SHA256SUMS" "$tmp/SHA256SUMS" || die "could not download SHA256SUMS"

expected="$(awk -v f="$asset" '$2 == f || $2 == "*" f { print $1 }' "$tmp/SHA256SUMS")"
[ -n "$expected" ] || die "$asset is not listed in SHA256SUMS"
actual="$(sha256 "$tmp/$asset")"
[ "$expected" = "$actual" ] || die "checksum mismatch for $asset (expected $expected, got $actual)"
say "checksum OK"

mkdir -p "$tmp/x"
tar -xzf "$tmp/$asset" -C "$tmp/x"
mkdir -p "$INSTALL_DIR"
for bin in gp-atlas gp-atlas-beta; do
  src="$(find "$tmp/x" -type f -name "$bin" | head -n 1)"
  [ -n "$src" ] || die "$bin missing from $asset"
  cp "$src" "$INSTALL_DIR/$bin.tmp"
  chmod 755 "$INSTALL_DIR/$bin.tmp"
  mv -f "$INSTALL_DIR/$bin.tmp" "$INSTALL_DIR/$bin"
done
if [ "$os" = "Darwin" ] && command -v xattr >/dev/null 2>&1; then
  xattr -d com.apple.quarantine "$INSTALL_DIR/gp-atlas" "$INSTALL_DIR/gp-atlas-beta" 2>/dev/null || true
fi

say "installed gp-atlas and gp-atlas-beta in $INSTALL_DIR"
case ":$PATH:" in
  *":$INSTALL_DIR:"*) say "run: gp-atlas" ;;
  *)
    say "$INSTALL_DIR is not on your PATH. Add this line to your shell profile (~/.zshrc or ~/.bashrc), then open a new terminal:"
    printf '\n  export PATH="%s:$PATH"\n\n' "$INSTALL_DIR"
    ;;
esac
command -v sf >/dev/null 2>&1 || say "note: the Salesforce CLI (sf) was not found; install it with: npm install --global @salesforce/cli@latest"
