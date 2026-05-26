#!/bin/sh
# scix-client installer
#
# Downloads the latest prebuilt `scix` binary for your platform from
# https://github.com/yipihey/scix-client/releases and installs it to
# ~/.local/bin (or $SCIX_INSTALL_DIR if set).
#
# Usage:
#   curl -sSf https://raw.githubusercontent.com/yipihey/scix-client/main/install.sh | sh
#
# Environment:
#   SCIX_INSTALL_DIR   Install directory (default: $HOME/.local/bin)
#   SCIX_VERSION       Version to install (default: latest)
#   GITHUB_TOKEN       Optional, for higher rate limits on api.github.com
#
# Exit codes:
#   1  usage / unsupported platform
#   2  download failed
#   3  install failed

set -eu

REPO="yipihey/scix-client"
INSTALL_DIR="${SCIX_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${SCIX_VERSION:-latest}"

err() { printf 'error: %s\n' "$*" >&2; exit "${2:-1}"; }
info() { printf '%s\n' "$*"; }

need_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        err "missing required command: $1"
    fi
}

# Pick curl or wget for downloads.
if command -v curl >/dev/null 2>&1; then
    DL='curl -fsSL'
    DL_OUT='curl -fsSL -o'
elif command -v wget >/dev/null 2>&1; then
    DL='wget -qO-'
    DL_OUT='wget -qO'
else
    err "need curl or wget"
fi

need_cmd uname
need_cmd tar
need_cmd mkdir
need_cmd chmod
need_cmd mv

# Detect platform target triple matching the release artifact names.
detect_target() {
    os="$(uname -s)"
    arch="$(uname -m)"
    case "$os" in
        Linux)
            case "$arch" in
                x86_64|amd64) echo "x86_64-unknown-linux-gnu" ;;
                aarch64|arm64) echo "aarch64-unknown-linux-gnu" ;;
                *) err "unsupported linux arch: $arch" ;;
            esac
            ;;
        Darwin)
            case "$arch" in
                x86_64) echo "x86_64-apple-darwin" ;;
                arm64) echo "aarch64-apple-darwin" ;;
                *) err "unsupported macos arch: $arch" ;;
            esac
            ;;
        MINGW*|MSYS*|CYGWIN*)
            err "windows: please download from https://github.com/$REPO/releases" 1
            ;;
        *)
            err "unsupported OS: $os"
            ;;
    esac
}

resolve_version() {
    if [ "$VERSION" = "latest" ]; then
        api_url="https://api.github.com/repos/$REPO/releases/latest"
        if [ -n "${GITHUB_TOKEN:-}" ]; then
            tag=$($DL -H "Authorization: Bearer $GITHUB_TOKEN" "$api_url" 2>/dev/null \
                | sed -n 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1)
        else
            tag=$($DL "$api_url" 2>/dev/null \
                | sed -n 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1)
        fi
        [ -n "$tag" ] || err "could not resolve latest release tag" 2
        echo "$tag"
    elif [ "${VERSION#v}" = "$VERSION" ]; then
        echo "v$VERSION"
    else
        echo "$VERSION"
    fi
}

TARGET="$(detect_target)"
TAG="$(resolve_version)"
VER="${TAG#v}"
ARCHIVE="scix-v${VER}-${TARGET}.tar.gz"
URL="https://github.com/$REPO/releases/download/$TAG/$ARCHIVE"

info "scix-client installer"
info "  repo:    $REPO"
info "  version: $TAG"
info "  target:  $TARGET"
info "  prefix:  $INSTALL_DIR"
info ""

TMP=$(mktemp -d 2>/dev/null || mktemp -d -t scix)
trap 'rm -rf "$TMP"' EXIT INT TERM

info "Downloading $ARCHIVE..."
$DL_OUT "$TMP/$ARCHIVE" "$URL" || err "download failed: $URL" 2

info "Extracting..."
tar -xzf "$TMP/$ARCHIVE" -C "$TMP" || err "extract failed" 2

[ -f "$TMP/scix" ] || err "archive did not contain expected binary 'scix'" 2

mkdir -p "$INSTALL_DIR" || err "cannot create $INSTALL_DIR" 3
chmod +x "$TMP/scix"
mv "$TMP/scix" "$INSTALL_DIR/scix" || err "cannot install to $INSTALL_DIR" 3

info ""
info "Installed: $INSTALL_DIR/scix"

# Friendly PATH check.
case ":$PATH:" in
    *":$INSTALL_DIR:"*)
        info ""
        info "Next: scix setup"
        ;;
    *)
        info ""
        info "Warning: $INSTALL_DIR is not on your PATH."
        info "Add this to your shell profile (.bashrc / .zshrc):"
        info "  export PATH=\"\$HOME/.local/bin:\$PATH\""
        info ""
        info "Then run: scix setup"
        ;;
esac
