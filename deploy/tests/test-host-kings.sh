#!/usr/bin/env bash
# host.sh's three-game wiring with fake build products: no compiler or network.
#
#   bash deploy/tests/test-host-kings.sh
#
# This pins the historical arena/fire argv beside Kings' port, explicit name,
# commit-aware probes, pid/url files, local host.json and exact six-process
# shutdown contract.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
DEPLOY="$(cd "$HERE/.." && pwd)"
ROOT="$(cd "$DEPLOY/.." && pwd)"
# shellcheck source=deploy/tests/lib.sh
. "$HERE/lib.sh"

TMP="$(mktemp -d -t ember-kings-hosttest-XXXXXX)"
pidof_file() { cut -d' ' -f1 "$1"; }
cleanup() {
    for file in "$TMP"/home/run/*.pid; do
        [ -f "$file" ] || continue
        kill -9 "$(pidof_file "$file")" 2>/dev/null || true
    done
    rm -rf "$TMP"
}
trap cleanup EXIT

git clone -q --bare "$ROOT" "$TMP/source.git"
mkdir -p "$TMP/bin"
cp "$HERE/shims/cargo" "$TMP/bin/cargo"
cp "$HERE/shims/cloudflared-stub.sh" "$TMP/bin/cloudflared"
chmod +x "$TMP/bin/cargo" "$TMP/bin/cloudflared"
PAGES="$TMP/pages.git"
git init -q --bare "$PAGES"

export PATH="$TMP/bin:$PATH"
export SHIM_LOG="$TMP/wiring.log"
export EMBER_FAKE_CARGO_TARGET="$TMP/target"
export EMBER_FAKE_SERVER="$HERE/shims/host-server-stub.sh"
export EMBER_FAKE_PROBE="$HERE/shims/host-probe-stub.sh"
export CARGO_TARGET_DIR="$EMBER_FAKE_CARGO_TARGET"
export EMBER_CONF_DIR="$TMP/conf"
export EMBER_NAME_FILE="$TMP/conf/host-name"
export EMBER_HOST_NAME="quiet-egret"
export EMBER_HOME="$TMP/home"
export EMBER_REPO="$TMP/source.git"
export EMBER_REF=HEAD
export EMBER_PUBLISH="$PAGES#gh-pages"
export EMBER_TUNNEL_BIN="$TMP/bin/cloudflared"
export EMBER_ARENA_PORT=17780
export EMBER_FIRE_PORT=17781
export EMBER_KINGS_PORT=17782

echo "== fake host up =="
if bash "$DEPLOY/host.sh" up > "$TMP/up.log" 2>&1; then
    ok "host.sh up succeeded"
else
    bad "host.sh up failed"
    tail -60 "$TMP/up.log" >&2
    summary host-kings
    exit 1
fi

WIRE="$(cat "$SHIM_LOG")"
contains "$WIRE" "arena-server [--bind] [127.0.0.1:17780]" "arena launch argv is unchanged"
contains "$WIRE" "fire-server [127.0.0.1:17781]" "fire launch argv is unchanged"
contains "$WIRE" "kings-server [127.0.0.1:17782] [--name] [quiet-egret]" "Kings launches on 7782's override with its name"
contains "$WIRE" "wsbot [ws://127.0.0.1:17780]" "arena keeps its wsbot probe"
contains "$WIRE" "fire-probe [ws://127.0.0.1:17781]" "fire keeps its own probe binary"
COMMIT="$(git --git-dir="$TMP/source.git" rev-parse --short HEAD)"
contains "$WIRE" "kings-probe [ws://127.0.0.1:17782] [--expect-commit] [$COMMIT]" "Kings loopback probe checks the deployed commit"
is "$(grep -c '^kings-probe .*--expect-commit' "$SHIM_LOG")" "2" "Kings checks the commit locally and publicly"

echo "== three pid/url pairs and the local entry =="
is "$(find "$EMBER_HOME/run" -name '*.pid' -type f | wc -l | tr -d ' ')" "6" "exactly three server/tunnel pid pairs exist"
is "$(find "$EMBER_HOME/run" -name '*.url' -type f | wc -l | tr -d ' ')" "3" "exactly three game URL files exist"
for game in arena fire kings; do
    if [ -s "$EMBER_HOME/run/server-$game.pid" ]; then ok "$game server pid recorded"; else bad "$game server pid missing"; fi
    if [ -s "$EMBER_HOME/run/tunnel-$game.pid" ]; then ok "$game tunnel pid recorded"; else bad "$game tunnel pid missing"; fi
done
LOCAL="$EMBER_HOME/run/host.json"
is "$(jget "$LOCAL" 'd["name"]')" "quiet-egret" "local entry names the host"
is "$(jget "$LOCAL" 'd["kings_ws"]')" "ws://127.0.0.1:17782" "local entry carries Kings' address"
is "$(jget "$LOCAL" 'd["kings_proto"]')" "1" "local entry carries Kings' protocol"
is "$(jget "$LOCAL" 'd["kings_commit"]')" "$COMMIT" "local entry carries Kings' build stamp"

echo "== unchanged update preserves all three servers and refreshes host.json =="
BEFORE="$(pidof_file "$EMBER_HOME/run/server-kings.pid")"
bash "$DEPLOY/host.sh" update > "$TMP/update.log" 2>&1 || bad "unchanged update failed"
contains "$(cat "$TMP/update.log")" "all three servers are running" "update requires all three servers"
is "$(pidof_file "$EMBER_HOME/run/server-kings.pid")" "$BEFORE" "unchanged update did not restart Kings"
if [ -s "$LOCAL" ]; then ok "unchanged update left host.json current"; else bad "unchanged update lost host.json"; fi

echo "== down stops exactly the three pairs =="
PIDS=()
for game in arena fire kings; do
    PIDS+=("$(pidof_file "$EMBER_HOME/run/server-$game.pid")")
    PIDS+=("$(pidof_file "$EMBER_HOME/run/tunnel-$game.pid")")
done
bash "$DEPLOY/host.sh" down > "$TMP/down.log" 2>&1 || bad "down failed"
for pid in "${PIDS[@]}"; do
    if kill -0 "$pid" 2>/dev/null; then bad "pid $pid survived down"; else ok "pid $pid stopped"; fi
done
is "$(find "$EMBER_HOME/run" -name '*.pid' -type f | wc -l | tr -d ' ')" "0" "all six pid files were removed"

summary host-kings
