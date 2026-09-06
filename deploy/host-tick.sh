#!/usr/bin/env bash
# host-tick.sh - one periodic check of an ember host; the timer that
# deploy/install-host-timer.sh installs runs it every two minutes.
#
#   bash deploy/host-tick.sh
#
# Two questions, in this order:
#
#   Did something die? All three servers AND all three tunnels must still be
#   the processes host.sh started (pid + start time, host.sh's own rule). A
#   tunnel that exited leaves healthy servers at an address nobody can
#   reach, and `host.sh update` only looks at the servers - so a dead
#   tunnel or server is answered with `host.sh up`, which restarts, mints
#   and publishes new addresses.
#
#   Did EMBER_REF move? `host.sh update` rebuilds, restarts, re-proves and
#   republishes when it did, and says "nothing to do" when it did not. With
#   EMBER_REF=ci-passed that pointer is moved by .github/workflows/ci.yml
#   only when a main commit's tests are green, so a red run keeps the host
#   on the last good build.
#
# Serialised with a lock. Quiet ticks log nothing; everything else goes to
# $EMBER_HOME/log/tick.log.
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

dead=""
for id in arena fire kings; do
    alive "server-$id" || dead="$dead server-$id"
    alive "tunnel-$id" || dead="$dead tunnel-$id"
done
if [ -n "$dead" ]; then
    tlog "down:$dead - running host.sh up"
    if bash "$HOST_SH" up >> "$LOGS/tick.log" 2>&1; then
        tlog "up done"
    else
        tlog "up FAILED (rc $?)"
    fi
    exit 0
fi

out="$(bash "$HOST_SH" update 2>&1)"
rc=$?
case "$out" in
    *"nothing to do"*) ;;
    *)
        printf '%s\n' "$out" >> "$LOGS/tick.log"
        tlog "update rc=$rc"
        ;;
esac
exit 0
