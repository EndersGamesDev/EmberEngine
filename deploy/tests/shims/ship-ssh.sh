#!/usr/bin/env bash
# A fake `ssh` for deploy/tests/test-ship-host.sh.
#
# It stands in for BOTH machines ship-host.sh talks to, and answers the four
# things the script actually reads back: the builder's build report, the host's
# own entry, a probe's exit status, and silence for everything that only has to
# succeed. The remote build script arrives on stdin and is written out whole,
# because the command line the builder is given is the thing under test.
#
# What the shim exists to exercise is ship-host.sh's CONTROL FLOW — which
# commit it asks for, what it sends the builder, what it copies where, and what
# it concludes. It is not a simulation of either machine and must not grow into
# one.
#
#   SHIP_BUILD_SCRIPT   where to write the script the builder was sent
#   SHIP_BUILT_COMMIT   the short sha the builder reports (default 129bcac4)
#   SHIP_HOST_JSON      the file served as the host's run/host.json
#   SHIP_PROBE_FAIL     non-empty: every probe on the host fails
set -uo pipefail

LOG="${SHIM_LOG:-/dev/null}"
{ printf 'ssh'; for a in "$@"; do printf ' [%s]' "$a"; done; printf '\n'; } >> "$LOG"

ARGS="$*"
case "$ARGS" in
    *"bash -s"*)
        # The builder. Keep the script it was handed, then report the build.
        cat > "${SHIP_BUILD_SCRIPT:-/dev/null}"
        commit="${SHIP_BUILT_COMMIT:-129bcac4}"
        echo "SHIP version=${SHIP_BUILT_VERSION:-r1490}"
        echo "SHIP commit=$commit"
        echo "SHIP full_commit=${SHIP_BUILT_FULL:-129bcac4000000000000000000000000000000ff}"
        echo "SHIP arena_proto=22"
        echo "SHIP fire_proto=1"
        echo "SHIP kings_proto=1"
        ;;
    *ember-host/run/host.json*)
        cat "${SHIP_HOST_JSON:-/dev/null}"
        ;;
    *wsbot*|*fire-probe*|*kings-probe*)
        [ -z "${SHIP_PROBE_FAIL:-}" ] || exit 1
        ;;
esac
exit 0
