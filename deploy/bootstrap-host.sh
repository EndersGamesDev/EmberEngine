#!/usr/bin/env bash
# Prepare an unprivileged Linux account to run an ember host.
#
#   bash deploy/bootstrap-host.sh
#
# cloudflared 2026.8.3 and its checksum come from Cloudflare's official release:
# https://github.com/cloudflare/cloudflared/releases/tag/2026.8.3
set -euo pipefail

CLOUDFLARED_VERSION="2026.8.3"
CLOUDFLARED_ASSET="cloudflared-linux-amd64"
CLOUDFLARED_SHA256="f29324fe934d1e100617484c78deef803c4dc2cd351d645bbde42e96b4fccc5e"
CLOUDFLARED_URL="https://github.com/cloudflare/cloudflared/releases/download/${CLOUDFLARED_VERSION}/${CLOUDFLARED_ASSET}"

die() { echo "bootstrap-host.sh: $*" >&2; exit 1; }

need() {
    command -v "$1" >/dev/null 2>&1 || die "missing $2 ($1 is not on PATH)"
}

need git git
need cargo "Rust toolchain"
need rustc "Rust toolchain"
need python3 python3
need curl curl
need sha256sum sha256sum

python3 -c '' >/dev/null 2>&1 || die "python3 is on PATH but does not run"
cargo --version >/dev/null 2>&1 || die "Rust toolchain is on PATH but cargo does not run"
rustc --version >/dev/null 2>&1 || die "Rust toolchain is on PATH but rustc does not run"

[ "$(uname -s)" = "Linux" ] || die "unsupported operating system '$(uname -s)'; need Linux"
case "$(uname -m)" in
    x86_64|amd64) ;;
    *) die "unsupported architecture '$(uname -m)'; the pinned asset is linux-amd64" ;;
esac

BIN_DIR="$HOME/bin"
TUNNEL_BIN="$BIN_DIR/cloudflared"
CONF_DIR="$HOME/.ember"
CONF="$CONF_DIR/host.env"
DOWNLOAD_TMP=""
CONF_TMP=""
cleanup() {
    [ -z "$DOWNLOAD_TMP" ] || rm -f "$DOWNLOAD_TMP"
    [ -z "$CONF_TMP" ] || rm -f "$CONF_TMP"
}
trap cleanup EXIT

mkdir -p "$BIN_DIR" "$CONF_DIR"

installed_sha=""
if [ -f "$TUNNEL_BIN" ]; then
    installed_sha="$(sha256sum "$TUNNEL_BIN" | cut -d' ' -f1)"
fi
if [ "$installed_sha" = "$CLOUDFLARED_SHA256" ]; then
    chmod 0755 "$TUNNEL_BIN"
    echo "cloudflared $CLOUDFLARED_VERSION already installed at $TUNNEL_BIN"
else
    DOWNLOAD_TMP="$(mktemp "$BIN_DIR/.cloudflared.XXXXXX")"
    echo "downloading cloudflared $CLOUDFLARED_VERSION ($CLOUDFLARED_ASSET)"
    curl --fail --location --show-error --silent --output "$DOWNLOAD_TMP" "$CLOUDFLARED_URL" \
        || die "download failed: $CLOUDFLARED_URL"
    downloaded_sha="$(sha256sum "$DOWNLOAD_TMP" | cut -d' ' -f1)"
    if [ "$downloaded_sha" != "$CLOUDFLARED_SHA256" ]; then
        rm -f "$DOWNLOAD_TMP"
        DOWNLOAD_TMP=""
        die "cloudflared checksum mismatch: expected $CLOUDFLARED_SHA256, got $downloaded_sha; deleted the download"
    fi
    chmod 0755 "$DOWNLOAD_TMP"
    mv -f "$DOWNLOAD_TMP" "$TUNNEL_BIN"
    DOWNLOAD_TMP=""
    echo "installed verified cloudflared at $TUNNEL_BIN"
fi

if [ -e "$CONF" ]; then
    echo "kept existing $CONF"
else
    CONF_TMP="$(mktemp "$CONF_DIR/.host.env.XXXXXX")"
    cat > "$CONF_TMP" <<'ENV'
# ember host configuration. Uncomment and edit what you want to change; the
# environment overrides anything set here.

#EMBER_REPO=https://github.com/EndersGamesDev/EmberEngine.git
#EMBER_REF=origin/main
#EMBER_HOST_NAME=
#EMBER_PUBLISH=none
#EMBER_ARENA_PORT=7780
#EMBER_FIRE_PORT=7781
#EMBER_HOME=$HOME/ember-host
#EMBER_TUNNEL_BIN=$HOME/bin/cloudflared
ENV
    chmod 0600 "$CONF_TMP"
    if ln "$CONF_TMP" "$CONF" 2>/dev/null; then
        echo "wrote $CONF (all defaults, nothing enabled)"
    else
        echo "kept existing $CONF"
    fi
    rm -f "$CONF_TMP"
    CONF_TMP=""
fi

echo "host bootstrap complete"
