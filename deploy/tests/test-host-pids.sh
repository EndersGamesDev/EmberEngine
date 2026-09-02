#!/usr/bin/env bash
# host.sh's process-control block, against real processes.
#
#   bash deploy/tests/test-host-pids.sh
#
# Nothing is built and no server is started: the block is lifted out of
# host.sh as it ships (between its `# --- process control` marker and
# `stop_all()`) and evaluated here with $RUN pointing at a temp directory, so
# `record_pid`, `alive` and `stop_one` can be driven against `sleep`
# processes this test owns. Lifted rather than copied, so the test cannot
# drift from what runs.
#
# What it is here to catch: a pid file that carries no identity. Nothing clears
# these files across a reboot and pids are handed out again from the bottom
# afterwards, so `update` believed a stranger was the game server and left the
# host offline, and `down` sent that stranger SIGTERM and then SIGKILL.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
DEPLOY="$(cd "$HERE/.." && pwd)"
# shellcheck source=deploy/tests/lib.sh
. "$HERE/lib.sh"

TMP="$(mktemp -d -t ember-pidtest-XXXXXX)"
trap 'rm -rf "$TMP"' EXIT
RUN="$TMP/run"
mkdir -p "$RUN"

BLOCK="$TMP/process-control.sh"
awk '/^# --- process control/{f=1} /^stop_all\(\)/{f=0} f' "$DEPLOY/host.sh" > "$BLOCK"
if grep -q 'record_pid()' "$BLOCK" && grep -q 'alive()' "$BLOCK" && grep -q 'stop_one()' "$BLOCK"; then
    ok "the process-control block was lifted out of host.sh"
else
    bad "could not lift the process-control block out of host.sh"
    summary host-pids
    exit 1
fi
# shellcheck source=/dev/null
. "$BLOCK"

have_proc=""
[ -r "/proc/$$/stat" ] && have_proc=1

echo "== a recorded process is alive, and carries its start time =="
sleep 30 &
MINE=$!
record_pid server-arena "$MINE"
is "$(cut -d' ' -f1 "$RUN/server-arena.pid")" "$MINE" "the pid is the first field"
if [ -n "$have_proc" ]; then
    is "$(cut -d' ' -s -f2 "$RUN/server-arena.pid")" "$(awk '{print $22}' "/proc/$MINE/stat")" \
        "and its start time the second"
else
    is "$(cut -d' ' -s -f2 "$RUN/server-arena.pid")" "-" \
        "and '-' where there is no /proc to read one from"
fi
if alive server-arena; then ok "alive says so"; else bad "alive said no about a process it just recorded"; fi

echo "== a reused pid is not the process we started =="
# The reboot case, without a reboot: the number is live and belongs to someone
# else, which is exactly what the recorded start time is there to catch.
sleep 30 &
STRANGER=$!
if [ -n "$have_proc" ]; then
    echo "$STRANGER 1" > "$RUN/server-fire.pid"
    if alive server-fire; then
        bad "alive believed a pid whose start time does not match"
    else
        ok "alive refuses a pid whose start time does not match"
    fi
    stop_one server-fire
    if kill -0 "$STRANGER" 2>/dev/null; then
        ok "and stop_one left that process alone"
    else
        bad "stop_one killed a process that was not ours"
    fi
    is "$(ls "$RUN"/server-fire.pid 2>/dev/null | wc -l | tr -d ' ')" "0" "and cleared the stale file"
else
    ok "skipped: no /proc on this system to read a start time from"
    ok "skipped: no /proc on this system to read a start time from"
    ok "skipped: no /proc on this system to read a start time from"
fi

echo "== a pid file from an older host.sh is not trusted =="
# No second field at all. Unverifiable reads as NOT alive, which errs towards
# redeploying a host rather than towards signalling a stranger.
echo "$STRANGER" > "$RUN/tunnel-arena.pid"
if alive tunnel-arena; then
    bad "a pid file with no identity was believed"
else
    ok "a pid file with no identity is not believed"
fi
stop_one tunnel-arena
if kill -0 "$STRANGER" 2>/dev/null; then
    ok "and stop_one did not signal it"
else
    bad "stop_one killed a process recorded by an older host.sh"
fi

echo "== '-' means 'no /proc here', and falls back to the bare pid =="
echo "$STRANGER -" > "$RUN/tunnel-fire.pid"
if alive tunnel-fire; then ok "a live pid with no readable start time is alive"; else bad "the fallback rejected a live pid"; fi
kill "$STRANGER" 2>/dev/null || true
wait "$STRANGER" 2>/dev/null || true
if alive tunnel-fire; then bad "a dead pid was called alive"; else ok "and a dead one is not"; fi

echo "== stop_one stops what we started, and always clears the file =="
if stop_one server-arena | grep -q "stopped server-arena"; then
    ok "it reports the stop"
else
    bad "stop_one did not report stopping the process it was given"
fi
if kill -0 "$MINE" 2>/dev/null; then bad "the recorded process is still running"; else ok "the process is gone"; fi
is "$(ls "$RUN"/*.pid 2>/dev/null | wc -l | tr -d ' ')" "1" "and every pid file it touched is cleared"
stop_one tunnel-fire
is "$(ls "$RUN"/*.pid 2>/dev/null | wc -l | tr -d ' ')" "0" "including the last one"
if stop_one never-started; then ok "stopping something that was never started is fine"; else bad "stop_one failed on an absent pid file"; fi

summary host-pids
