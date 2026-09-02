#!/usr/bin/env bash
# Shared helpers for deploy/tests. Sourced, never run.
#
# Deliberately tiny: these tests exist to prove the deploy scripts behave, and
# a test harness with its own bugs would prove nothing. No framework, no
# discovery, four helpers.

# The first candidate that actually RUNS wins, not the first that resolves.
# On a Windows box `command -v python3` finds the Microsoft Store app-execution
# alias, a stub that exits 49 with an install prompt, and a `||` chain that
# stops at the first hit never reaches the real `python` next to it. `|| true`
# on the lookup so the loop is reachable under `set -e`: an assignment whose
# command substitution fails is itself a failing command.
PY=""
for candidate in python3 python; do
    p="$(command -v "$candidate" || true)"
    [ -n "$p" ] && "$p" -c '' >/dev/null 2>&1 || continue
    PY="$p"
    break
done
[ -n "$PY" ] || { echo "tests: no working python3/python on PATH" >&2; exit 1; }

TESTS_RUN=0
TESTS_FAILED=0
TESTS_T0="$(date +%s)"

ok() {
    TESTS_RUN=$((TESTS_RUN + 1))
    echo "  ok   $*"
}

bad() {
    TESTS_RUN=$((TESTS_RUN + 1))
    TESTS_FAILED=$((TESTS_FAILED + 1))
    echo "  FAIL $*" >&2
}

# is <actual> <expected> <label>
is() {
    if [ "$1" = "$2" ]; then
        ok "$3"
    else
        bad "$3 — got '$1', want '$2'"
    fi
}

# contains <haystack> <needle> <label>
contains() {
    case "$1" in
        *"$2"*) ok "$3" ;;
        *)      bad "$3 — '$2' not found in: $1" ;;
    esac
}

# jget <json file> <python expression over `d`>
# `eval` in a test is not a hazard; the expressions are all written here.
jget() {
    "$PY" -c 'import json,sys;d=json.load(open(sys.argv[1]));print(eval(sys.argv[2]))' "$1" "$2"
}

# summary <suite name>; exits non-zero if anything failed.
summary() {
    local wall=$(( $(date +%s) - TESTS_T0 ))
    echo "SUITE $1: $((TESTS_RUN - TESTS_FAILED))/$TESTS_RUN passed, ${wall}s"
    [ "$TESTS_FAILED" -eq 0 ]
}
