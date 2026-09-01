#!/usr/bin/env bash
# The OFF-HOST watchdog. Runs where the git credentials already are (the
# workstation), not on the game host, so no GitHub token ever has to be
# copied to specht.
#
#   bash deploy/watchdog.sh            # loop forever
#   bash deploy/watchdog.sh --once     # single pass, for a scheduler
#
# It closes the two gaps the systemd units cannot:
#
#   1. ADDRESS DRIFT. A Cloudflare quick tunnel mints a NEW random hostname on
#      every restart. After an unattended reboot the servers are healthy at an
#      address server.json does not name, so every player sees a dead game.
#      This probes the PUBLISHED address and redeploys when it stops answering,
#      which republishes the new hostname.
#   2. NEW COMMITS. When origin/main moves, the running servers are stale.
#      This redeploys them.
#
# Deliberately probes the PUBLISHED address rather than the server directly:
# that is the thing a player actually depends on, and it fails when either the
# server OR the tunnel OR server.json is wrong.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_DIR"

INTERVAL="${WATCHDOG_INTERVAL:-300}"
STATE="${WATCHDOG_STATE:-$REPO_DIR/.watchdog-state}"
PAGES_URL="${EMBER_PAGES_URL:-https://endersgamesdev.github.io/EmberEngine}"
ONCE=""
[ "${1:-}" = "--once" ] && ONCE=1

log() { echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] $*"; }

# A published address is healthy only if it completes a WebSocket handshake.
# A plain 200/502 from the tunnel says nothing about the game server: a dead
# origin behind a live tunnel returns 502, and a wedged server still returns
# 101, which is why the deploys probe the protocol. Here we only need to
# distinguish "reachable" from "gone", so 101 is the right bar.
probe() {
    local url="$1" host code
    [ -z "$url" ] || [ "$url" = "null" ] && return 1
    host="https://${url#wss://}"
    code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 15 \
        -H 'Connection: Upgrade' -H 'Upgrade: websocket' \
        -H 'Sec-WebSocket-Version: 13' -H 'Sec-WebSocket-Key: SGVsbG8sIHdvcmxkIQ==' \
        "$host" 2>/dev/null || true)
    [ "$code" = "101" ]
}

pass() {
    git fetch -q origin main gh-pages 2>/dev/null || { log "fetch failed; skipping pass"; return; }

    local head deployed
    head="$(git rev-parse origin/main)"
    deployed="$(cat "$STATE" 2>/dev/null || echo none)"

    local want_pong="" want_fire="" want_pages=""

    if [ "$head" != "$deployed" ]; then
        log "origin/main moved ($deployed -> ${head:0:7}); redeploying everything"
        want_pong=1; want_fire=1; want_pages=1
    fi

    # Read the addresses the PAGES are currently telling players to use.
    local sj ws fire_ws
    sj="$(curl -s --max-time 15 "$PAGES_URL/server.json?ts=$(date +%s)" 2>/dev/null || echo '{}')"
    ws="$(printf '%s' "$sj" | python -c 'import json,sys;print(json.load(sys.stdin).get("ws",""))' 2>/dev/null || echo '')"
    fire_ws="$(printf '%s' "$sj" | python -c 'import json,sys;print(json.load(sys.stdin).get("fire_ws",""))' 2>/dev/null || echo '')"

    if [ -z "$want_pong" ]; then
        if probe "$ws"; then log "arena OK   ($ws)"; else log "arena DOWN ($ws)"; want_pong=1; fi
    fi
    if [ -z "$want_fire" ]; then
        if probe "$fire_ws"; then log "fire  OK   ($fire_ws)"; else log "fire  DOWN ($fire_ws)"; want_fire=1; fi
    fi

    [ -z "$want_pong$want_fire$want_pages" ] && return

    # Only fast-forward. A dirty tree or a diverged branch means a human is
    # mid-change; redeploying over that would ship something nobody tested.
    if ! git diff --quiet || ! git diff --cached --quiet; then
        log "working tree dirty; refusing to redeploy"
        return
    fi
    git merge --ff-only origin/main -q 2>/dev/null || { log "cannot fast-forward; refusing"; return; }

    failed=""
    [ -n "$want_pong" ]  && { log "redeploying arena"; bash deploy/deploy-pong-online.sh || { log "arena deploy FAILED"; failed=1; }; }
    [ -n "$want_fire" ]  && { log "redeploying fire";  bash deploy/deploy-fire-online.sh || { log "fire deploy FAILED";  failed=1; }; }
    [ -n "$want_pages" ] && { log "redeploying pages"; bash deploy/deploy-pages.sh       || { log "pages deploy FAILED"; failed=1; }; }

    # Only record the commit as deployed if EVERY deploy that ran succeeded.
    # Recording it regardless is how a failure becomes permanent: an online
    # deploy that mints a tunnel but fails to publish leaves server.json naming
    # a DEAD domain, and a watchdog that has already written the sha sees
    # "nothing to do" on the next pass and never retries. That happened on
    # 2026-09-01 — fire's publish failed, the state file was written anyway,
    # and the published fire_ws answered 530 while the live tunnel was fine
    # under a different name. Leaving the sha unwritten makes the next pass
    # retry, and the health probe below is what stops it looping silently.
    if [ -n "$failed" ]; then
        log "one or more deploys FAILED; not recording ${head:0:7} — next pass retries"
        return
    fi
    echo "$head" > "$STATE"
    log "pass complete at ${head:0:7}"
}

if [ -n "$ONCE" ]; then
    pass
else
    log "watchdog up; interval ${INTERVAL}s; pages $PAGES_URL"
    while true; do pass; sleep "$INTERVAL"; done
fi
