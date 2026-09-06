#!/usr/bin/env bash
# host-tick.sh - one periodic check of an ember host; the timer that
# deploy/install-host-timer.sh installs runs it every two minutes.
#
#   bash deploy/host-tick.sh
#
# Three questions, in this order:
#
#   Did a SERVER die? `host.sh up`: rebuild if needed, restart the servers,
#   keep every tunnel that still answers.
#
#   Did a TUNNEL die? `host.sh tunnels`: mint only what is missing, keep the
#   rest, republish. Never by restarting healthy servers.
#
#   Did EMBER_REF move? `host.sh update` rebuilds, restarts the servers
#   behind their existing tunnels, re-proves and republishes; "nothing to
#   do" otherwise. With EMBER_REF=ci-passed that pointer is moved by
#   .github/workflows/ci.yml only when a main commit's tests are green, so
#   a red run keeps the host on the last good build.
#
# Minting is rationed. A quick tunnel is a request to Cloudflare, which
# rate-limits them per public IP (HTTP 429, error 1015) - and every host
# behind the same router shares that IP. After a run that could not prove
# all three tunnels this script backs off - 30 minutes, or 60 doubling to
# 240 when the tunnel logs show a 429 - before it lets anything mint again;
# meanwhile host.sh runs with EMBER_NO_MINT=1, so servers are still updated
# and restarted but no tunnel is requested. The first version of this file
# re-minted three tunnels every two minutes and took the whole IP out of
# quick tunnels for an afternoon (2026-09-06).
#
# Serialised with a lock on descriptor 9, which host.sh and its background
# children must not inherit (a retained server would hold it forever).
# Quiet ticks log nothing; everything else goes to $EMBER_HOME/log/tick.log.
set -uo pipefail

# The host's own configuration, read the way host.sh reads it.
CONF="${EMBER_CONF_DIR:-$HOME/.ember}/host.env"
# shellcheck source=/dev/null
[ -f "$CONF" ] && . "$CONF"
EMBER_HOME="${EMBER_HOME:-$HOME/ember-host}"
SRC="$EMBER_HOME/src"
RUN="$EMBER_HOME/run"
LOGS="$EMBER_HOME/log"
HOST_SH="$SRC/deploy/host.sh"
BACKOFF="$RUN/.mint-backoff"   # "<until, epoch seconds> <attempt> [logged]"
mkdir -p "$RUN" "$LOGS"
if [ ! -f "$HOST_SH" ]; then
    echo "host-tick: no checkout at $SRC; run deploy/host.sh up once" >&2
    exit 0
fi
# cargo lives in the user's home, and a timer has no login shell to find it.
# shellcheck source=/dev/null
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"

tlog() { echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) $*" >> "$LOGS/tick.log"; }

exec 9>"$RUN/.tick.lock"
if ! flock -n 9; then exit 0; fi

# host.sh's liveness rule: the recorded pid must exist AND carry the
# recorded start time, so a reused pid after a reboot is not "alive".
alive() {
    local f="$RUN/$1.pid" pid stamp now
    [ -f "$f" ] || return 1
    pid="$(cut -d' ' -f1 "$f")"
    stamp="$(cut -d' ' -s -f2 "$f")"
    [ -n "$pid" ] || return 1
    kill -0 "$pid" 2>/dev/null || return 1
    [ -n "$stamp" ] || return 1
    if [ "$stamp" = "-" ]; then return 0; fi
    now="$(awk '{print $22}' "/proc/$pid/stat" 2>/dev/null || true)"
    [ "$now" = "$stamp" ]
}

now="$(date +%s)"
until_=0; attempt=0; logged=""
if [ -f "$BACKOFF" ]; then read -r until_ attempt logged < "$BACKOFF" || true; fi
until_="${until_:-0}"; attempt="${attempt:-0}"
holding=""
if [ "$until_" -gt "$now" ] 2>/dev/null; then holding=1; fi
# EMBER_NO_MINT is what host.sh honours; empty means minting is allowed.
nomint="${holding:+1}"

# note_result <rc>: after a host.sh run that was ALLOWED to mint. rc 3 means
# "servers fine, not every tunnel proven": start or extend the backoff.
note_result() {
    local rc="$1" wait n
    if [ "$rc" -eq 3 ]; then
        n=$((attempt + 1))
        wait=1800
        if grep -qs -E '429|error code: 1015' "$RUN"/tunnel-*.log; then
            case "$n" in 1) wait=3600 ;; 2) wait=7200 ;; *) wait=14400 ;; esac
        fi
        echo "$((now + wait)) $n" > "$BACKOFF"
        tlog "tunnels not proven; no minting for $((wait / 60)) min (attempt $n)"
    elif [ "$rc" -eq 0 ]; then
        rm -f "$BACKOFF"
    fi
}

dead_servers=""
dead_tunnels=""
for id in arena fire kings; do
    alive "server-$id" || dead_servers="$dead_servers server-$id"
    alive "tunnel-$id" || dead_tunnels="$dead_tunnels tunnel-$id"
done

if [ -n "$dead_servers" ]; then
    tlog "down:$dead_servers$dead_tunnels - running host.sh up${nomint:+ (EMBER_NO_MINT=1: backing off from a 429)}"
    EMBER_NO_MINT="$nomint" bash "$HOST_SH" up 9>&- >> "$LOGS/tick.log" 2>&1
    rc=$?
    if [ "$rc" -eq 0 ]; then tlog "up done"; else tlog "up finished with rc $rc"; fi
    [ -n "$nomint" ] || note_result "$rc"
    exit 0
fi

if [ -n "$dead_tunnels" ]; then
    if [ -n "$holding" ]; then
        if [ -z "$logged" ]; then
            tlog "down:$dead_tunnels - not minting before $(date -u -d "@$until_" +%H:%M:%SZ 2>/dev/null || echo "$until_") (Cloudflare rate-limit backoff, attempt $attempt)"
            echo "$until_ $attempt logged" > "$BACKOFF"
        fi
        # fall through: an update may still be due, servers are restarted
        # behind whatever tunnels exist, and nothing is minted
    else
        tlog "down:$dead_tunnels - running host.sh tunnels"
        bash "$HOST_SH" tunnels 9>&- >> "$LOGS/tick.log" 2>&1
        rc=$?
        if [ "$rc" -eq 0 ]; then tlog "tunnels done"; else tlog "tunnels finished with rc $rc"; fi
        note_result "$rc"
        exit 0
    fi
fi

out="$(EMBER_NO_MINT="$nomint" bash "$HOST_SH" update 9>&- 2>&1)"
rc=$?
case "$out" in
    *"nothing to do"*) ;;
    *)
        printf '%s\n' "$out" >> "$LOGS/tick.log"
        tlog "update rc=$rc"
        [ -n "$nomint" ] || note_result "$rc"
        ;;
esac
exit 0
