#!/usr/bin/env bash
# The whole of host.sh, end to end, on loopback: clone, build, start all three
# servers, prove each one with the repo's own probes, mint an address from a
# stub tunnel, publish the entry into a local bare repository, then update,
# status and down.
#
#   bash deploy/tests/test-host-loopback.sh
#
# Nothing leaves this machine. The "remote" repositories are `git init --bare`
# directories under a temp dir and the "tunnel" is deploy/tests/shims.
#
# EMBER_TEST_REPO — a bare clone of this repo to deploy from. Needed wherever
# `git clone` cannot read the checkout directly (a git worktree whose .git
# file names a path this git cannot follow, which is the case when the tests
# run inside WSL against a Windows checkout). Without it the test makes its
# own bare clone, and skips if that fails.
# EMBER_TEST_REF  — the ref to deploy; defaults to the current HEAD.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
DEPLOY="$(cd "$HERE/.." && pwd)"
ROOT="$(cd "$DEPLOY/.." && pwd)"
# shellcheck source=deploy/tests/lib.sh
. "$HERE/lib.sh"

TMP="$(mktemp -d -t ember-hosttest-XXXXXX)"

# A pid file is "<pid> <start time>" (or "<pid> -" where there is no /proc):
# the pid alone is not an identity, so host.sh records what the number has to
# match to still be ours. Everything here wants the number.
pidof_file() { cut -d' ' -f1 "$1"; }

cleanup() {
    # Never leave a server or a stub tunnel behind, whatever failed.
    for f in "$TMP"/home/run/*.pid; do
        [ -f "$f" ] || continue
        kill -9 "$(pidof_file "$f")" 2>/dev/null || true
    done
    # EMBER_TEST_KEEP=1 leaves the logs and the checkout in place; without it
    # a failure takes its own evidence with it.
    if [ -n "${EMBER_TEST_KEEP:-}" ]; then
        echo "kept $TMP"
    else
        rm -rf "$TMP"
    fi
}
trap cleanup EXIT

SRC_REPO="${EMBER_TEST_REPO:-}"
if [ -z "$SRC_REPO" ]; then
    if git clone -q --bare "$ROOT" "$TMP/src.git" 2>/dev/null; then
        SRC_REPO="$TMP/src.git"
    fi
fi
if [ -z "$SRC_REPO" ]; then
    echo "SKIP: no repository to deploy from; set EMBER_TEST_REPO to a bare clone"
    exit 0
fi
REF="${EMBER_TEST_REF:-$(git --git-dir="$SRC_REPO" rev-parse --abbrev-ref HEAD 2>/dev/null || echo HEAD)}"

PAGES="$TMP/pages.git"
git init -q --bare "$PAGES"

mkdir -p "$TMP/bin"
cp "$HERE/shims/cloudflared-stub.sh" "$TMP/bin/cloudflared"
chmod +x "$TMP/bin/cloudflared"

export EMBER_CONF_DIR="$TMP/conf"
export EMBER_NAME_FILE="$TMP/conf/host-name"
export EMBER_HOME="$TMP/home"
export EMBER_REPO="$SRC_REPO"
export EMBER_REF="$REF"
export EMBER_PUBLISH="$PAGES#gh-pages"
export EMBER_TUNNEL_BIN="$TMP/bin/cloudflared"
# High ports, so a real host running the games on 7780/7781/7782 is not disturbed
# by a test run.
export EMBER_ARENA_PORT=17780
export EMBER_FIRE_PORT=17781
export EMBER_KINGS_PORT=17782

echo "== host.sh up (from $SRC_REPO at $REF) =="
T0="$(date +%s)"
if bash "$DEPLOY/host.sh" up > "$TMP/up.log" 2>&1; then
    ok "up succeeded in $(( $(date +%s) - T0 ))s"
else
    bad "up FAILED in $(( $(date +%s) - T0 ))s"
    tail -40 "$TMP/up.log" >&2
    summary host-loopback
    exit 1
fi
sed 's/^/    /' "$TMP/up.log" | tail -30

echo "== it proved all three servers, locally and through the address it published =="
contains "$(cat "$TMP/up.log")" "local health check for arena" "arena probed on loopback"
contains "$(cat "$TMP/up.log")" "local health check for fire" "fire probed on loopback"
contains "$(cat "$TMP/up.log")" "local health check for kings" "Kings probed on loopback"
contains "$(cat "$TMP/up.log")" "health check for arena through ws://127.0.0.1:17780" "arena probed through its address"
contains "$(cat "$TMP/up.log")" "health check for fire through ws://127.0.0.1:17781" "fire probed through its address"
contains "$(cat "$TMP/up.log")" "health check for kings through ws://127.0.0.1:17782" "Kings probed through its address"

echo "== the servers are up and named =="
NAME="$(EMBER_NAME_FILE="$TMP/conf/host-name" bash "$DEPLOY/host-name.sh")"
is "$(printf '%s' "$NAME" | grep -cE '^[a-z0-9-]{3,32}$')" "1" "the host has a generated name ($NAME)"
for id in arena fire kings; do
    PID="$(pidof_file "$EMBER_HOME/run/server-$id.pid")"
    if kill -0 "$PID" 2>/dev/null; then ok "$id server is running (pid $PID)"; else bad "$id server is not running"; fi
    if kill -0 "$(pidof_file "$EMBER_HOME/run/tunnel-$id.pid")" 2>/dev/null; then
        ok "$id tunnel is running"
    else
        bad "$id tunnel is not running"
    fi
done
# The name reaches the server through the ENVIRONMENT, never a flag, so that a
# host still on an older commit can run a binary that never heard of --name.
# This is the one assertion that would catch a well-meaning switch to --name,
# and it is why it reads the live process rather than the script.
ENVIRON="/proc/$(pidof_file "$EMBER_HOME/run/server-arena.pid")/environ"
if [ -r "$ENVIRON" ]; then
    contains "$(tr '\0' '\n' < "$ENVIRON")" "EMBER_HOST_NAME=$NAME" \
        "the running arena server carries EMBER_HOST_NAME"
else
    echo "  note: $ENVIRON is not readable here; cannot check the launch environment"
fi

echo "== a pid file carries an identity, not just a number =="
# A bare pid is not an identity: nothing clears these across a reboot and the
# numbers are handed out again from the bottom afterwards.
is "$(awk '{print NF}' "$EMBER_HOME/run/server-arena.pid")" "2" "the pid file has two fields"

echo "== what is running was recorded before the publish =="
# The record of what is UP must not depend on the publish succeeding: an
# `update` that reads "none" answers by tearing down two healthy servers.
UP_LINE="$(grep -n 'UP as' "$TMP/up.log" | head -1 | cut -d: -f1)"
PUB_LINE="$(grep -n '== publishing ==' "$TMP/up.log" | head -1 | cut -d: -f1)"
if [ -n "$UP_LINE" ] && [ -n "$PUB_LINE" ] && [ "$UP_LINE" -lt "$PUB_LINE" ]; then
    ok "the UP summary comes before publishing"
else
    bad "the host still publishes before recording what is running"
fi
if [ -s "$EMBER_HOME/run/deployed" ]; then ok "and the deployed ref is on disk"; else bad "no deployed ref was recorded"; fi

echo "== the published entry =="
ENTRY="$TMP/entry"
git clone -q --branch gh-pages "$PAGES" "$ENTRY"
BOOK="$ENTRY/host.json"
is "$(jget "$BOOK" 'd["name"]')" "$NAME" "name"
is "$(jget "$BOOK" 'd["ws"]')" "ws://127.0.0.1:17780" "ws"
is "$(jget "$BOOK" 'd["fire_ws"]')" "ws://127.0.0.1:17781" "fire_ws"
is "$(jget "$BOOK" 'd["kings_ws"]')" "ws://127.0.0.1:17782" "kings_ws"
is "$(jget "$BOOK" 'str(d["proto"]).isdigit()')" "True" "proto is a number"
is "$(jget "$BOOK" 'str(d["fire_proto"]).isdigit()')" "True" "fire_proto is a number"
is "$(jget "$BOOK" 'str(d["kings_proto"]).isdigit()')" "True" "kings_proto is a number"
is "$(jget "$BOOK" 'bool(d["version"].startswith("r"))')" "True" "version is r<N>"
is "$(jget "$BOOK" 'bool(len(d["commit"]) >= 7)')" "True" "commit is a short sha"
is "$(jget "$BOOK" 'bool(d["updated"].endswith("Z"))')" "True" "updated is UTC"
# The protocol numbers must be the ones in the checkout that was deployed,
# not whatever the machine running the test happens to have.
SRC_PROTO="$(grep -oE 'PROTO_VERSION: u16 = [0-9]+' "$EMBER_HOME/src/crates/arena-core/src/proto.rs" | grep -oE '[0-9]+$')"
is "$(jget "$BOOK" 'str(d["proto"])')" "$SRC_PROTO" "proto came from the deployed ref"

echo "== status =="
bash "$DEPLOY/host.sh" status > "$TMP/status.log" 2>&1 || bad "status exited non-zero"
sed 's/^/    /' "$TMP/status.log"
contains "$(cat "$TMP/status.log")" "arena server: running" "status sees the arena server"
contains "$(cat "$TMP/status.log")" "fire server: running" "status sees the fire server"
contains "$(cat "$TMP/status.log")" "kings server: running" "status sees the Kings server"
contains "$(cat "$TMP/status.log")" "ws://127.0.0.1:17780" "status shows the address"
contains "$(cat "$TMP/status.log")" "published: {" "status reads the published entry back"
contains "$(cat "$TMP/status.log")" "$NAME" "and it is this host's"

echo "== update on an unmoved ref does nothing =="
PID_BEFORE="$(pidof_file "$EMBER_HOME/run/server-arena.pid")"
T1="$(date +%s)"
bash "$DEPLOY/host.sh" update > "$TMP/update.log" 2>&1 || bad "update exited non-zero"
contains "$(cat "$TMP/update.log")" "up to date" "update said the ref had not moved"
is "$(pidof_file "$EMBER_HOME/run/server-arena.pid")" "$PID_BEFORE" "and did not restart the arena server"
echo "    update took $(( $(date +%s) - T1 ))s"

echo "== down leaves nothing running =="
ARENA_PID="$(pidof_file "$EMBER_HOME/run/server-arena.pid")"
FIRE_PID="$(pidof_file "$EMBER_HOME/run/server-fire.pid")"
KINGS_PID="$(pidof_file "$EMBER_HOME/run/server-kings.pid")"
TUN_A="$(pidof_file "$EMBER_HOME/run/tunnel-arena.pid")"
TUN_F="$(pidof_file "$EMBER_HOME/run/tunnel-fire.pid")"
TUN_K="$(pidof_file "$EMBER_HOME/run/tunnel-kings.pid")"
bash "$DEPLOY/host.sh" down > "$TMP/down.log" 2>&1 || bad "down exited non-zero"
sleep 1
for p in "$ARENA_PID" "$FIRE_PID" "$KINGS_PID" "$TUN_A" "$TUN_F" "$TUN_K"; do
    if kill -0 "$p" 2>/dev/null; then bad "pid $p survived down"; else ok "pid $p is gone"; fi
done
for id in arena fire kings; do
    if [ -f "$EMBER_HOME/run/server-$id.pid" ]; then bad "$id pid file left behind"; else ok "$id pid file removed"; fi
done

echo "== status after down =="
bash "$DEPLOY/host.sh" status > "$TMP/status2.log" 2>&1 || bad "status after down exited non-zero"
contains "$(cat "$TMP/status2.log")" "arena server: DOWN" "status reports the arena down"
contains "$(cat "$TMP/status2.log")" "fire tunnel: DOWN" "status reports the fire tunnel down"
contains "$(cat "$TMP/status2.log")" "kings server: DOWN" "status reports Kings down"

summary host-loopback
