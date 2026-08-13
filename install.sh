#!/bin/sh
# udu installer — https://github.com/smaniotov/udu
#
# Downloads the released binary, verifies its SHA-256 against the checksum file
# published with the release, and installs it to ~/.local/bin.
#
# It does not use sudo, does not touch system directories, and does not start
# anything: udu asks for consent before installing its service on first launch.
#
#   curl -fsSL https://raw.githubusercontent.com/smaniotov/udu/main/install.sh | sh
#
# Environment:
#   UDU_VERSION   install a specific tag (default: latest release)
#   UDU_BIN_DIR   install directory (default: ~/.local/bin)

set -eu

REPO="smaniotov/udu"
BIN_DIR="${UDU_BIN_DIR:-$HOME/.local/bin}"
TARGET="x86_64-unknown-linux-gnu"

say() { printf '%s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

need() {
    command -v "$1" >/dev/null 2>&1 || die "$1 is required but not installed"
}

need curl
need tar

case "$(uname -s)" in
    Linux) ;;
    *) die "udu is Linux only (evdev capture, systemd user service). Detected: $(uname -s)" ;;
esac

case "$(uname -m)" in
    x86_64|amd64) ;;
    *) die "no prebuilt binary for $(uname -m); install from source with: cargo install udu" ;;
esac

if [ -n "${UDU_VERSION:-}" ]; then
    version="$UDU_VERSION"
else
    version=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
        | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -1)
    [ -n "$version" ] || die "could not determine the latest release; set UDU_VERSION"
fi

stage="udu-$version-$TARGET"
base="https://github.com/$REPO/releases/download/$version"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM

say "downloading udu $version"
curl -fsSL "$base/$stage.tar.gz" -o "$tmp/$stage.tar.gz" \
    || die "download failed — does release $version exist?"
curl -fsSL "$base/SHA256SUMS" -o "$tmp/SHA256SUMS" \
    || die "could not fetch SHA256SUMS; refusing to install unverified binary"

say "verifying checksum"
expected=$(grep " $stage.tar.gz\$" "$tmp/SHA256SUMS" | awk '{print $1}')
[ -n "$expected" ] || die "no checksum published for $stage.tar.gz"

if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "$tmp/$stage.tar.gz" | awk '{print $1}')
else
    need shasum
    actual=$(shasum -a 256 "$tmp/$stage.tar.gz" | awk '{print $1}')
fi

[ "$expected" = "$actual" ] || die "checksum mismatch — expected $expected, got $actual"

tar -xzf "$tmp/$stage.tar.gz" -C "$tmp"
[ -f "$tmp/$stage/udu" ] || die "archive did not contain the udu binary"

mkdir -p "$BIN_DIR"
install -m 755 "$tmp/$stage/udu" "$BIN_DIR/udu"

say ""
say "udu $version installed to $BIN_DIR/udu"

case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *)
        say ""
        say "$BIN_DIR is not on your PATH. Add it to your shell profile:"
        say "  export PATH=\"\$PATH:$BIN_DIR\""
        ;;
esac

say ""
say "Run 'udu' to start. It will ask before installing its background service."
say "Verify the build came from this source:"
say "  gh attestation verify $BIN_DIR/udu -R $REPO"
