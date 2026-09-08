#!/usr/bin/env bash
# Every directory containing a tracked file has a tracked README.md.
#
#   bash deploy/tests/test-readmes.sh
#   bash deploy/tests/test-readmes.sh --self-test
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"

check_repo() {
    local repo="$1"
    local dir file readme
    local missing=0
    declare -A tracked=()

    while IFS= read -r file; do
        tracked["$file"]=1
    done < <(git -C "$repo" ls-files)

    while IFS= read -r dir; do
        [ -n "$dir" ] || continue
        if [ "$dir" = "." ]; then
            readme="README.md"
        else
            readme="$dir/README.md"
        fi
        if [ -z "${tracked[$readme]+tracked}" ]; then
            echo "missing README.md: $dir" >&2
            missing=1
        fi
    done < <(git -C "$repo" ls-files | xargs -n1 dirname | sort -u)

    [ "$missing" -eq 0 ]
}

self_test() {
    local started tmp first
    started="$(date +%s)"
    tmp="$(mktemp -d -t ember-readmetest-XXXXXX)"
    README_TEST_TMP="$tmp"
    trap '[ -z "${README_TEST_TMP:-}" ] || rm -rf "$README_TEST_TMP"' EXIT

    git -C "$tmp" init -q
    mkdir -p "$tmp/documented" "$tmp/generated" "$tmp/undocumented"
    printf '# Fixture\n' > "$tmp/README.md"
    printf '# Documented\n' > "$tmp/documented/README.md"
    printf 'tracked\n' > "$tmp/documented/file.txt"
    printf 'Generated from fixture-source.\n' > "$tmp/generated/README.md"
    printf 'tracked\n' > "$tmp/generated/artifact.txt"
    printf 'tracked\n' > "$tmp/undocumented/file.txt"
    git -C "$tmp" add .

    if first="$(check_repo "$tmp" 2>&1)"; then
        echo "SELF-TEST FAIL: a missing README.md was accepted" >&2
        return 1
    fi
    if [ "$first" != "missing README.md: undocumented" ]; then
        echo "SELF-TEST FAIL: wrong missing-directory report: $first" >&2
        return 1
    fi

    printf '# Undocumented no longer\n' > "$tmp/undocumented/README.md"
    git -C "$tmp" add undocumented/README.md
    if ! check_repo "$tmp"; then
        echo "SELF-TEST FAIL: a complete fixture was rejected" >&2
        return 1
    fi

    echo "SELF-TEST PASS: missing and complete fixtures, $(( $(date +%s) - started ))s"
}

if [ "${1:-}" = "--self-test" ]; then
    [ "$#" -eq 1 ] || { echo "usage: bash deploy/tests/test-readmes.sh [--self-test]" >&2; exit 2; }
    self_test
    exit $?
fi
[ "$#" -eq 0 ] || { echo "usage: bash deploy/tests/test-readmes.sh [--self-test]" >&2; exit 2; }

started="$(date +%s)"
if check_repo "$ROOT"; then
    echo "README CHECK PASS: every tracked directory is documented, $(( $(date +%s) - started ))s"
else
    status=$?
    echo "README CHECK FAIL: missing folder READMEs, $(( $(date +%s) - started ))s" >&2
    exit "$status"
fi
