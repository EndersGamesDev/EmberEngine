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
#   SHIP_RESOLVED_COMMIT  what the builder resolved the ref to when building
#   SHIP_REF_COMMIT     what the builder resolves EMBER_SHIP_REF to for check
set -uo pipefail

LOG="${SHIM_LOG:-/dev/null}"
{ printf 'ssh'; for a in "$@"; do printf ' [%s]' "$a"; done; printf '\n'; } >> "$LOG"

ARGS="$*"
# Model ssh's own stdin handling: without -n it reads and forwards stdin, so a
# caller inside `while read` loses the rest of its list to it. A shim kinder
# than the real thing would let exactly that bug through.
DRAIN=1
for a in "$@"; do [ "$a" = "-n" ] && DRAIN=""; done
case "$ARGS" in
    *rev-parse*)
        # The builder resolving EMBER_SHIP_REF for `check`. Empty by default,
        # so a shim that was never told the answer produces "cannot tell"
        # rather than a confident wrong one.
        printf '%s\n' "${SHIP_REF_COMMIT:-}"
        ;;
    *"bash -s"*)
        # The builder. Keep the script it was handed, then report the build.
        cat > "${SHIP_BUILD_SCRIPT:-/dev/null}"
        commit="${SHIP_BUILT_COMMIT:-129bcac4}"
        echo "SHIP stage=${SHIP_BUILT_STAGE:-/workspace/loops/ember/ember-ship-products}"
        echo "SHIP version=${SHIP_BUILT_VERSION:-r1490}"
        echo "SHIP commit=$commit"
        echo "SHIP full_commit=${SHIP_BUILT_FULL:-129bcac4000000000000000000000000000000ff}"
        # What the builder RESOLVED the ref to, which the real script compares
        # against what it stamped. It agrees by default, so only a test about
        # that disagreement has to say anything.
        echo "SHIP ref_commit=${SHIP_RESOLVED_COMMIT:-${SHIP_BUILT_FULL:-129bcac4000000000000000000000000000000ff}}"
        echo "SHIP arena_proto=22"
        echo "SHIP fire_proto=1"
        echo "SHIP kings_proto=1"
        echo "SHIP league_proto=2"
        ;;
    *ember-host/run/host.json*)
        cat "${SHIP_HOST_JSON:-/dev/null}"
        ;;
    *wsbot*|*fire-probe*|*kings-probe*|*league-probe*)
        [ -z "$DRAIN" ] || cat >/dev/null 2>&1 || true
        [ -z "${SHIP_PROBE_FAIL:-}" ] || exit 1
        ;;
esac
exit 0
