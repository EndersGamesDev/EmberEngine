#!/usr/bin/env bash
# The OFF-HOST watchdog. Runs where the git credentials already are (the
# workstation), not on the game hosts, so no GitHub token ever has to be
# copied to one of them.
#
#   bash deploy/watchdog.sh            # loop forever
#   bash deploy/watchdog.sh --once     # single pass, for a scheduler
#
#   EMBER_HOSTS="specht adler"         # the hosts to watch (docs/hosts.md §9)
#
# It closes the two gaps the systemd units cannot:
#
#   1. ADDRESS DRIFT. A Cloudflare quick tunnel mints a NEW random hostname on
#      every restart. After an unattended reboot the servers are healthy at an
#      address the book does not name, so every player sees a dead game.
#      This probes the PUBLISHED addresses and redeploys when one stops
#      answering, which republishes the new hostname.
#   2. NEW COMMITS. When origin/main moves, the running servers are stale.
#      This redeploys them.
#
# Deliberately probes the PUBLISHED addresses rather than the servers
# directly: that is the thing a player actually depends on, and it fails when
# either the server OR the tunnel OR the book is wrong.
#
# MANY HOSTS. Each ssh name in EMBER_HOSTS is watched independently: its own
# name is resolved on the machine, its own entry is found in `hosts[]`, its own
# addresses are probed, and only that host is redeployed. State is per host, so
# one box that keeps failing cannot stop another from being retried — which is
# exactly the failure the single-host version had, where one bad deploy left
# the sha unwritten and every host was retried on the next pass.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_DIR"

INTERVAL="${WATCHDOG_INTERVAL:-300}"
STATE_DIR="${WATCHDOG_STATE_DIR:-$REPO_DIR}"
PAGES_URL="${EMBER_PAGES_URL:-https://endersgamesdev.github.io/EmberEngine}"
# One name, several names, or the old single EMBER_HOST — all three work.
HOSTS="${EMBER_HOSTS:-${EMBER_HOST:-specht}}"
PY="$(command -v python3 || command -v python)"
ONCE=""
[ "${1:-}" = "--once" ] && ONCE=1

log() { echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] $*"; }

# The games, and the address key each one uses in the book. Derived from the
# game id exactly as docs/hosts.md §3 says: the arena owns the bare `ws`,
# everyone else is prefixed. A new game is one word in GAMES plus its deploy
# script.
GAMES="arena fire"
addr_key()  { case "$1" in arena) echo ws ;; *) echo "$1_ws" ;; esac; }
deploy_for() { case "$1" in arena) echo deploy/deploy-pong-online.sh ;; fire) echo deploy/deploy-fire-online.sh ;; esac; }

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

# The address one named host publishes for one game, or "" when the host has
# no entry or does not run that game. Reads `hosts[]`, never the legacy
# top-level keys: those name whichever host is preferred, so probing them
# would test the same machine once per host in the list.
entry_addr() {
    local book="$1" name="$2" key="$3"
    printf '%s' "$book" | "$PY" -c '
import json, sys
name, key = sys.argv[1], sys.argv[2]
try:
    d = json.load(sys.stdin)
except Exception:
    print("")
    raise SystemExit(0)
for h in d.get("hosts", []) if isinstance(d, dict) else []:
    if isinstance(h, dict) and h.get("name") == name:
        print(h.get(key) or "")
        break
else:
    print("")
' "$name" "$key" 2>/dev/null || echo ""
}

# The host's own name, resolved on the machine itself (docs/hosts.md §6). It
# is the merge key of its entry, so the watchdog cannot find what to probe
# without it.
host_name_of() {
    local remote="$1" name
    name="$(ssh -o BatchMode=yes -o ConnectTimeout=10 "$remote" 'bash -s' \
        < "$REPO_DIR/deploy/host-name.sh" 2>/dev/null | tr -d '[:space:]')" || return 1
    printf '%s' "$name" | grep -qE '^[a-z0-9-]{3,32}$' || return 1
    printf '%s\n' "$name"
}

pass() {
    git fetch -q origin main gh-pages 2>/dev/null || { log "fetch failed; skipping pass"; return; }

    local head
    head="$(git rev-parse origin/main)"

    # Read the book the PAGES are currently serving, once for the whole pass.
    local book
    book="$(curl -s --max-time 15 "$PAGES_URL/server.json?ts=$(date +%s)" 2>/dev/null || echo '{}')"

    # Work out what each host needs before touching anything, so the global
    # refusals below are evaluated once and a host that needs nothing costs
    # one ssh and two probes.
    local remote name state deployed wants game url todo="" \
          pages_state pages_deployed want_pages=""
    for remote in $HOSTS; do
        state="$STATE_DIR/.watchdog-state-$remote"
        deployed="$(cat "$state" 2>/dev/null || echo none)"
        if ! name="$(host_name_of "$remote")"; then
            log "$remote: cannot resolve its host name over ssh; skipping"
            continue
        fi
        wants=""
        if [ "$head" != "$deployed" ]; then
            log "$remote ($name): origin/main moved (${deployed:0:7} -> ${head:0:7})"
            wants="$GAMES"
        else
            for game in $GAMES; do
                url="$(entry_addr "$book" "$name" "$(addr_key "$game")")"
                if [ -z "$url" ]; then
                    log "$remote ($name): $game has no published address; will deploy it"
                    wants="$wants $game"
                elif probe "$url"; then
                    log "$remote ($name): $game OK   ($url)"
                else
                    log "$remote ($name): $game DOWN ($url)"
                    wants="$wants $game"
                fi
            done
        fi
        # One "<ssh name>:<game>,<game>" line per host that needs work. Written
        # as an `if` rather than `[ … ] && …` because this is the last thing
        # the loop body evaluates, and a form whose exit status depends on
        # "nothing to do" is not one to leave under `set -e`.
        # shellcheck disable=SC2086
        wants="$(echo $wants)"
        if [ -n "$wants" ]; then
            todo="$todo$remote:$(echo "$wants" | tr ' ' ',')
"
        fi
    done

    pages_state="$STATE_DIR/.watchdog-state-pages"
    pages_deployed="$(cat "$pages_state" 2>/dev/null || echo none)"
    [ "$head" != "$pages_deployed" ] && want_pages=1

    [ -z "$todo" ] && [ -z "$want_pages" ] && return

    # Only fast-forward. A dirty tree or a diverged branch means a human is
    # mid-change; redeploying over that would ship something nobody tested.
    if ! git diff --quiet || ! git diff --cached --quiet; then
        log "working tree dirty; refusing to redeploy"
        return
    fi
    git merge --ff-only origin/main -q 2>/dev/null || { log "cannot fast-forward; refusing"; return; }

    local line games failed busy
    while IFS= read -r line; do
        [ -n "$line" ] || continue
        remote="${line%%:*}"
        games="$(echo "${line#*:}" | tr ',' ' ')"

        # NEVER redeploy on top of people who are playing. Restarting a server
        # drops every connected client instantly, and the watchdog is
        # unattended, so without this it will happily kick a full lobby to
        # ship a commit. On 2026-09-01 a manual run did exactly that: two
        # players had joined lobby `ender` at 11:20:57 and 11:21:07 and the
        # arena was restarted at 11:22:19, about 72 seconds later. pong-server
        # logs a health tick every 30 s carrying players_in_game, so the
        # answer is already on the host. A commit is never so urgent that it
        # cannot wait one poll interval.
        #
        # Asked of THIS host only: a full lobby on one machine must not stop
        # another machine from being brought back.
        busy="$(ssh -o BatchMode=yes -o ConnectTimeout=8 "$remote" \
            'grep -ao "players_in_game=[0-9]*" ~/pong-server.log 2>/dev/null | tail -1 | cut -d= -f2' \
            2>/dev/null || echo 0)"
        if [ "${busy:-0}" -gt 0 ] 2>/dev/null; then
            log "$remote: $busy player(s) in game; deferring to the next pass"
            continue
        fi

        failed=""
        for game in $games; do
            log "$remote: redeploying $game"
            EMBER_HOST="$remote" bash "$(deploy_for "$game")" \
                || { log "$remote: $game deploy FAILED"; failed=1; }
        done

        # Only record the commit as deployed if EVERY deploy that ran
        # succeeded. Recording it regardless is how a failure becomes
        # permanent: an online deploy that mints a tunnel but fails to publish
        # leaves the book naming a DEAD domain, and a watchdog that has already
        # written the sha sees "nothing to do" on the next pass and never
        # retries. That happened on 2026-09-01 — fire's publish failed, the
        # state file was written anyway, and the published fire_ws answered 530
        # while the live tunnel was fine under a different name.
        if [ -n "$failed" ]; then
            log "$remote: not recording ${head:0:7} — the next pass retries"
        else
            echo "$head" > "$STATE_DIR/.watchdog-state-$remote"
            log "$remote: at ${head:0:7}"
        fi
    done <<< "$todo"

    if [ -n "$want_pages" ]; then
        log "redeploying pages"
        if bash deploy/deploy-pages.sh; then
            echo "$head" > "$pages_state"
        else
            log "pages deploy FAILED; not recording ${head:0:7}"
        fi
    fi
}

if [ -n "$ONCE" ]; then
    pass
else
    log "watchdog up; interval ${INTERVAL}s; pages $PAGES_URL; hosts: $HOSTS"
    while true; do pass; sleep "$INTERVAL"; done
fi
