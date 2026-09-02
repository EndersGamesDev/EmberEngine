#!/usr/bin/env bash
# Run the arena as a HOST on this Windows workstation (docs/hosts.md §8), the
# way deploy/host.sh does on a Linux box:
#   1. build arena-server (release, stamped) from the COMMITTED tree
#   2. (re)start it on 127.0.0.1:7780 under this machine's host name
#   3. prove it speaks the protocol on loopback, BEFORE exposing it
#   4. (re)start a Cloudflare quick tunnel in front of it (fresh domain)
#   5. prove it again through the public address
#   6. publish this host's entry into the address book on gh-pages
#
#   bash deploy/deploy-arena-local.sh          # up
#   bash deploy/deploy-arena-local.sh down     # stop server and tunnel
#   bash deploy/deploy-arena-local.sh status   # what is running, from what
#
# Needs cargo (this workstation has one), ~/tools/cloudflared.exe (the
# official Windows build from github.com/cloudflare/cloudflared/releases),
# python, and push rights to gh-pages.
#
# WHY NOT host.sh. host.sh is written for a Linux host (nohup, pid files
# checked with kill -0, chrt) and clones its own checkout of origin/main.
# This one runs the committed tree in front of it and starts its two
# processes through PowerShell so they outlive the git-bash shell. Everything
# that decides what goes into the book is shared, not copied: the name comes
# from deploy/host-name.sh, the entry is written by deploy/publish-host.sh,
# and the build stamp is the same EMBER_BUILD_VERSION/COMMIT pair build.rs
# reads, so the Welcome this server sends is the truth the book records.
#
# WHAT IT DOES NOT SURVIVE. A workstation sleeps and reboots and nothing
# off-host restarts these two processes. Until a Linux host runs host.sh on
# the same commit, the v13 arena is only as up as this machine.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_DIR"
CMD="${1:-up}"
PORT="${EMBER_ARENA_PORT:-7780}"
BIND="127.0.0.1:$PORT"
CLOUDFLARED="${EMBER_TUNNEL_BIN:-$HOME/tools/cloudflared.exe}"
EMBER_REPO="${EMBER_REPO:-https://github.com/EndersGamesDev/EmberEngine.git}"
# none = print the entry and publish nothing; upstream = write the book.
EMBER_PUBLISH="${EMBER_PUBLISH:-upstream}"
RUN="$HOME/.ember/arena-local"
mkdir -p "$RUN"
SERVER_LOG="$RUN/arena-server.log"
TUNNEL_LOG="$RUN/tunnel.log"

say() { echo "== $* =="; }
die() { echo "FAILED: $*" >&2; exit 1; }
win() { cygpath -w "$1"; }

stop_all() {
    taskkill /IM arena-server.exe /F >/dev/null 2>&1 || true
    taskkill /IM cloudflared.exe /F >/dev/null 2>&1 || true
}

case "$CMD" in
    down)
        stop_all
        echo "stopped arena-server and cloudflared (the book still names this host; publish-host.sh --remove retires it)"
        exit 0 ;;
    status)
        tasklist | grep -i "arena-server.exe\|cloudflared.exe" || echo "nothing running"
        [ -f "$RUN/arena.url" ] && echo "last published: $(cat "$RUN/arena.url") as $(cat "$RUN/stamp" 2>/dev/null)"
        tail -3 "$SERVER_LOG.err" 2>/dev/null || true
        exit 0 ;;
    up) ;;
    *) die "usage: $0 [up|down|status]" ;;
esac

if ! git diff --quiet || ! git diff --cached --quiet; then
    git status --short >&2
    die "working tree is dirty; this deploys the committed tree"
fi
[ -x "$CLOUDFLARED" ] || die "no tunnel binary at $CLOUDFLARED (EMBER_TUNNEL_BIN)"
python -c '' 2>/dev/null || die "need a working python on PATH to publish the entry"
grep -q "^name = \"arena-server\"" crates/arena-server/Cargo.toml || die "this tree has no arena-server crate"

NAME="$(bash deploy/host-name.sh)"
VERSION="r$(git rev-list --count HEAD)"
COMMIT="$(git rev-parse --short HEAD)"
PROTO="$(grep -oE 'PROTO_VERSION: u16 = [0-9]+' crates/arena-core/src/proto.rs | grep -oE '[0-9]+$' | head -1)"
say "host $NAME · building $VERSION · $COMMIT · arena protocol $PROTO"

t0=$(date +%s)
EMBER_BUILD_VERSION="$VERSION" EMBER_BUILD_COMMIT="$COMMIT" \
    nice -n 19 cargo build --release -p arena-server
EMBER_BUILD_VERSION="$VERSION" EMBER_BUILD_COMMIT="$COMMIT" \
    nice -n 19 cargo build --release -p arena-server --example wsbot
echo "   built in $(( $(date +%s) - t0 ))s"
WSBOT="$REPO_DIR/target/release/examples/wsbot.exe"
[ -x "$WSBOT" ] || die "$WSBOT was not built"

say "stopping whatever was running"
stop_all
sleep 1
if netstat -an | grep -q "TCP    $BIND .*LISTENING"; then
    die "something else is listening on $BIND"
fi

say "starting arena-server on $BIND as $NAME"
: > "$SERVER_LOG"; : > "$SERVER_LOG.err"
powershell -NoProfile -Command "
  \$env:EMBER_HOST_NAME='$NAME'; \$env:RUST_LOG='info'
  Start-Process -FilePath '$(win "$REPO_DIR/target/release/arena-server.exe")' \
    -ArgumentList '--bind','$BIND' -WindowStyle Hidden \
    -RedirectStandardOutput '$(win "$SERVER_LOG")' -RedirectStandardError '$(win "$SERVER_LOG.err")'
"
sleep 2
tasklist | grep -qi "arena-server.exe" || { tail -20 "$SERVER_LOG.err" >&2; die "arena-server did not stay up"; }
grep -m1 "build\|listening" "$SERVER_LOG.err" || true

say "local health check (before exposing it)"
"$WSBOT" "ws://$BIND" create "local-$NAME" - healthcheck 6 || die "the server is listening but did not answer wsbot"

say "starting the tunnel (fresh domain)"
: > "$TUNNEL_LOG"
powershell -NoProfile -Command "
  Start-Process -FilePath '$(win "$CLOUDFLARED")' \
    -ArgumentList 'tunnel','--url','http://$BIND','--no-autoupdate' -WindowStyle Hidden \
    -RedirectStandardOutput '$(win "$TUNNEL_LOG.out")' -RedirectStandardError '$(win "$TUNNEL_LOG")'
"
TUNNEL=""
for _ in $(seq 1 30); do
    sleep 2
    TUNNEL=$(grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' "$TUNNEL_LOG" | head -1 || true)
    [ -n "$TUNNEL" ] && break
done
[ -n "$TUNNEL" ] || die "no trycloudflare domain appeared; see $TUNNEL_LOG"
WS_URL="wss://${TUNNEL#https://}"
echo "   $WS_URL"

# The first DNS query for a brand-new name can be cached as NXDOMAIN if it
# lands before Cloudflare publishes the record (host.sh learned this on its
# first real run). Wait, then retry for up to two minutes.
say "letting the tunnel hostname propagate (15s)"
sleep 15
say "health check through $WS_URL"
ok=""
for attempt in $(seq 1 24); do
    if "$WSBOT" "$WS_URL" create "public-$NAME-$attempt" - healthcheck 6; then ok=1; break; fi
    [ "$attempt" -lt 24 ] && sleep 5
done
[ -n "$ok" ] || die "the tunnel is up but wsbot could not create a lobby through it; not publishing"

echo "$WS_URL" > "$RUN/arena.url"
echo "$VERSION $COMMIT" > "$RUN/stamp"

say "publishing host $NAME into the address book ($EMBER_PUBLISH)"
BY="$(id -un)@$(hostname)"
case "$EMBER_PUBLISH" in
    none)
        tmp="$(mktemp -d)"; : > "$tmp/host.json"
        bash deploy/publish-host.sh --book "$tmp/host.json" --file host.json --name "$NAME" \
            --game arena --url "$WS_URL" --proto "$PROTO" --version "$VERSION" --commit "$COMMIT" --by "$BY" >/dev/null
        echo "   would publish:"; cat "$tmp/host.json"; echo ;;
    upstream)
        bash deploy/publish-host.sh --repo "$EMBER_REPO" --branch gh-pages --name "$NAME" \
            --game arena --url "$WS_URL" --proto "$PROTO" --version "$VERSION" --commit "$COMMIT" --by "$BY" ;;
    *) die "EMBER_PUBLISH must be none or upstream here" ;;
esac

echo
echo "ONLINE: $NAME serves arena protocol $PROTO at $WS_URL ($VERSION · $COMMIT). Logs in $RUN."
echo "The live page picks the newest build that speaks its protocol; this host stays in the book until publish-host.sh --remove."
