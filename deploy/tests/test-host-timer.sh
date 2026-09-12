#!/usr/bin/env bash
# bash deploy/tests/test-host-timer.sh
# Generated systemd units and real shell descriptor inheritance against shims.
# No sudo, system service, server, build or network is contacted. Only this
# test's disposable sleep children are created/stopped. Native flock exclusion
# is tested on Linux; Git Bash lacks flock and reports that portion skipped.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
DEPLOY="$(cd "$HERE/.." && pwd)"
# shellcheck source=deploy/tests/lib.sh
. "$HERE/lib.sh"
TMP="$(mktemp -d -t ember-timertest-XXXXXX)"
cleanup() {
    if [ -f "$TMP/owned-pids" ]; then
        while IFS= read -r pid; do
            case "$pid" in ''|*[!0-9]*) continue ;; esac
            kill "$pid" 2>/dev/null || true
            wait "$pid" 2>/dev/null || true
        done < "$TMP/owned-pids"
    fi
    rm -rf "${TMP:?}"
}
trap cleanup EXIT
mkdir -p "$TMP/bin" "$TMP/conf" "$TMP/units" "$TMP/host/src/deploy" "$TMP/host/run"
export EMBER_CONF_DIR="$TMP/conf" EMBER_HOME="$TMP/host" TIMER_SCRATCH="$TMP"
REAL_FLOCK="$(command -v flock || true)"
export PATH="$TMP/bin:$PATH"

cat > "$TMP/bin/sudo" <<'SHIM'
#!/usr/bin/env bash
set -euo pipefail
case "$1" in
    tee)
        case "${2:-}" in
            /etc/systemd/system/ember-host.service|/etc/systemd/system/ember-host.timer)
                cat > "$TIMER_SCRATCH/units/$(basename "$2")" ;;
            *) echo "test: refusing unexpected sudo target" >&2; exit 91 ;;
        esac ;;
    systemctl) shift; exec systemctl "$@" ;;
    *) echo "test: refusing unexpected sudo action" >&2; exit 92 ;;
esac
SHIM
cat > "$TMP/bin/systemctl" <<'SHIM'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$TIMER_SCRATCH/systemctl-calls"
printf 'NEXT LEFT LAST PASSED UNIT ACTIVATES\n'
SHIM
chmod +x "$TMP/bin/sudo" "$TMP/bin/systemctl"
# Only installer existence checks read this; run_tick uses production source.
cp "$DEPLOY/host-tick.sh" "$EMBER_HOME/src/deploy/host-tick.sh"

echo "== generated oneshot preserves the external PID owner =="
INSTALL_OUT="$(bash "$DEPLOY/install-host-timer.sh")"
SERVICE="$(cat "$TMP/units/ember-host.service")"
TIMER="$(cat "$TMP/units/ember-host.timer")"
contains "$SERVICE" 'Type=oneshot' "the updater remains a repeatable oneshot"
contains "$SERVICE" 'KillMode=process' "systemd must not kill host.sh-owned descendants on successful exit"
is "$(printf '%s\n' "$SERVICE" | grep -c '^KillMode=')" "1" "only one explicit kill mode is emitted"
if printf '%s\n' "$SERVICE" | grep -q '^RemainAfterExit=true'; then
    bad "RemainAfterExit would keep the updater active and suppress future timer invocations"
else
    ok "the unit can become inactive for its next scheduled tick"
fi
contains "$SERVICE" "User=$(id -un)" "service retains the unprivileged host identity"
contains "$SERVICE" "ExecStart=/usr/bin/bash $EMBER_HOME/src/deploy/host-tick.sh" "the configured checkout owns the tick"
contains "$TIMER" 'OnUnitActiveSec=2min' "the timer remains two-minute recurring"
contains "$INSTALL_OUT" 'Stopping the timer does not stop servers' "operator sees timer versus server lifecycle distinction"
contains "$INSTALL_OUT" 'deploy/host.sh down' "operator is told the actual server stop command"
contains "$(cat "$TMP/systemctl-calls")" 'daemon-reload' "installer requests a unit reload through the shim"
contains "$(cat "$TMP/systemctl-calls")" 'enable --now ember-host.timer' "installer enables only the timer through the shim"

if [ -z "$REAL_FLOCK" ]; then
    # The portable half proves fd9 is closed in host.sh and its descendant,
    # not kernel lock exclusion. Linux CI below uses the real flock binary.
    cat > "$TMP/bin/flock" <<'SHIM'
#!/usr/bin/env bash
test "$1" = -n && test "$2" = 9 || exit 93
{ true >&9; } 2>/dev/null
SHIM
    chmod +x "$TMP/bin/flock"
    echo "NOTE: native flock unavailable; kernel exclusion checks are skipped (descriptor and next-invocation checks still run)."
fi

cat > "$EMBER_HOME/src/deploy/host.sh" <<'SHIM'
#!/usr/bin/env bash
set -euo pipefail
mode="$1"
printf '%s\n' "$mode" >> "$TIMER_SCRATCH/host-calls"
if { true >&9; } 2>/dev/null; then fd=inherited; else fd=closed; fi
printf '%s\n' "$fd" > "$TIMER_SCRATCH/fd-$mode"
if [ "$mode" = update ] && [ "${TIMER_FAIL_UPDATE:-}" = 1 ]; then
    echo "host.sh: EMBER_REF='main' names no commit" >&2
    exit 23
fi
nohup bash -c '
    if { true >&9; } 2>/dev/null; then fd=inherited; else fd=closed; fi
    printf "%s\n" "$fd" > "$TIMER_SCRATCH/child-fd-$1"
    exec sleep 45
' _ "$mode" </dev/null > "$TIMER_SCRATCH/child-$mode.log" 2>&1 &
child=$!
printf '%s\n' "$child" >> "$TIMER_SCRATCH/owned-pids"
if [ "$mode" = up ]; then
    # All eight liveness checks intentionally refer to one disposable child.
    # '-' is the existing no-/proc fallback, usable by Git Bash as well.
    # The list must cover every game host-tick.sh enumerates: a pid file it
    # looks for and does not find reads as a dead server, and the tick would
    # run `host.sh up` instead of the path under test.
    for id in arena fire kings league; do
        printf '%s -\n' "$child" > "$EMBER_HOME/run/server-$id.pid"
        printf '%s -\n' "$child" > "$EMBER_HOME/run/tunnel-$id.pid"
    done
else
    echo 'nothing to do'
fi
SHIM
run_tick() { bash "$DEPLOY/host-tick.sh"; }
await_child() {
    for _ in $(seq 1 100); do
        [ -f "$TMP/child-fd-$1" ] && return 0
        sleep .02
    done
    bad "fixture child for $1 did not initialize"
    return 1
}

echo "== repair path closes the tick lock before launching descendants =="
run_tick
await_child up
is "$(cat "$TMP/host-calls")" "up" "missing services request the repair path"
is "$(cat "$TMP/fd-up")" "closed" "host.sh up cannot inherit descriptor9"
is "$(cat "$TMP/child-fd-up")" "closed" "the retained repair child cannot inherit descriptor9"
MINE="$(head -1 "$TMP/owned-pids")"
if kill -0 "$MINE" 2>/dev/null; then ok "repair child remains alive after its tick exits"; else bad "repair child exited"; fi
if [ -n "$REAL_FLOCK" ]; then
    if "$REAL_FLOCK" -n "$EMBER_HOME/run/.tick.lock" true; then
        ok "kernel lock is released while the retained repair child is alive"
    else
        bad "repair child retained the kernel lock"
    fi
fi

echo "== next healthy tick runs and its update path also closes descriptor9 =="
run_tick
await_child update
is "$(tail -1 "$TMP/host-calls")" "update" "the next tick reaches the healthy update path"
is "$(wc -l < "$TMP/host-calls" | tr -d ' ')" "2" "both ticks invoked host.sh"
is "$(cat "$TMP/fd-update")" "closed" "host.sh update cannot inherit descriptor9"
is "$(cat "$TMP/child-fd-update")" "closed" "the retained update child cannot inherit descriptor9"
if kill -0 "$MINE" 2>/dev/null; then ok "the original child stayed alive during the next tick"; else bad "original child exited"; fi
if [ -n "$REAL_FLOCK" ]; then
    if "$REAL_FLOCK" -n "$EMBER_HOME/run/.tick.lock" true; then
        ok "kernel lock is released after the update tick too"
    else
        bad "update child retained the kernel lock"
    fi
    (
        exec 9> "$EMBER_HOME/run/.tick.lock"
        "$REAL_FLOCK" -n 9
        touch "$TMP/lock-held"
        exec sleep 45
    ) &
    HOLDER=$!
    printf '%s\n' "$HOLDER" >> "$TMP/owned-pids"
    for _ in $(seq 1 100); do [ -f "$TMP/lock-held" ] && break; sleep .02; done
    [ -f "$TMP/lock-held" ] || { bad "lock holder did not initialize"; summary host-timer; exit 1; }
    run_tick
    is "$(wc -l < "$TMP/host-calls" | tr -d ' ')" "2" "an overlapping tick remains excluded by the real lock"
    kill "$HOLDER" 2>/dev/null || true
    wait "$HOLDER" 2>/dev/null || true
else
    echo "SKIP: three native flock release/exclusion assertions require Linux; no kernel-lock claim is made on Git Bash."
fi

echo "== a dead tunnel on the newest game reaches the tunnel repair path =="
# The regression this catches is a game the tick does not enumerate. A dead
# SERVER is caught anyway, by cmd_update's own game_ids loop inside host.sh;
# a dead TUNNEL is not, because nothing outside this script's list ever looks
# at it. So the assertion has to be that a tunnel belonging to the last game
# added is what sends the tick down the repair path. A pid no process holds
# is the cheapest dead tunnel: `alive` reads pid and start time, and finds
# neither.
echo '2147483647 -' > "$EMBER_HOME/run/tunnel-league.pid"
CALLS_BEFORE="$(wc -l < "$TMP/host-calls" | tr -d ' ')"
run_tick
is "$(tail -1 "$TMP/host-calls")" "tunnels" "a dead league tunnel is repaired rather than republished"
is "$(wc -l < "$TMP/host-calls" | tr -d ' ')" "$((CALLS_BEFORE + 1))" "and it took exactly one host.sh call to do it"
contains "$(cat "$EMBER_HOME/log/tick.log")" "tunnel-league" "the tick log names the dead tunnel"
printf '%s -\n' "$MINE" > "$EMBER_HOME/run/tunnel-league.pid"

echo "== an update resolution failure remains a timer failure =="
if TIMER_FAIL_UPDATE=1 run_tick; then
    bad "the timer hid host.sh's update failure"
else
    is "$?" "23" "the timer returns host.sh's update failure"
fi
contains "$(cat "$EMBER_HOME/log/tick.log")" "EMBER_REF='main' names no commit" "the timer log names the failed release ref"

summary host-timer
