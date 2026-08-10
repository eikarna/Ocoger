#!/usr/bin/env sh
# One-line installer for ocoger (Linux/musl, macOS, Termux/WSL).
#   curl -fsSL https://raw.githubusercontent.com/eikarna/Ocoger/main/install.sh | sh
# Env:
#   OCOGER_VERSION  — tag to install (default: latest release)
#   OCOGER_PREFIX   — install dir      (default: $HOME/.local/bin)
set -eu

REPO="eikarna/Ocoger"
PREFIX="${OCOGER_PREFIX:-$HOME/.local/bin}"
VER="${OCOGER_VERSION:-latest}"

err() { printf 'error: %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || err "missing dependency: $1"; }

OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)
case "$ARCH" in
    x86_64|amd64) TARGET_ARCH=x86_64 ;;
    aarch64|arm64) TARGET_ARCH=aarch64 ;;
    *) err "unsupported arch: $ARCH" ;;
esac
case "$OS" in
    linux*)
        # Prefer musl static build when /etc/alpine-release or ldd-musl detected.
        if [ -f /etc/alpine-release ] || ldd --version 2>&1 | grep -qi musl; then
            TARGET="$TARGET_ARCH-unknown-linux-musl"
        else
            TARGET="$TARGET_ARCH-unknown-linux-gnu"
        fi
        ;;
    darwin*) TARGET="$TARGET_ARCH-apple-darwin" ;;
    *) err "unsupported os: $OS (Windows users: run install.ps1 instead)" ;;
esac

need curl
if [ "$VER" = "latest" ]; then
    VER=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
        | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p')
    [ -n "$VER" ] || err "could not resolve latest release tag"
fi

URL="https://github.com/$REPO/releases/download/$VER/ocoger-$VER-$TARGET.tar.gz"
TMP=$(mktemp -d); trap 'rm -rf "$TMP"' EXIT
printf '==> downloading %s\n' "$URL"
curl -fsSL "$URL" -o "$TMP/ocoger.tar.gz" || err "download failed (release may not have a $TARGET asset)"

need tar
tar -xzf "$TMP/ocoger.tar.gz" -C "$TMP"
BIN=$(find "$TMP" -type f -name ocoger | head -1)
[ -n "$BIN" ] || err "archive did not contain the ocoger binary"

mkdir -p "$PREFIX"
install -m 0755 "$BIN" "$PREFIX/ocoger"
printf '==> installed %s/ocoger (%s)\n' "$PREFIX" "$VER"

case ":$PATH:" in
    *":$PREFIX:"*) ;;
    *) printf 'note: add to PATH:  export PATH="%s:$PATH"\n' "$PREFIX" ;;
esac
