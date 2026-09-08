#!/usr/bin/env bash

set -u

subject_pattern='^(feat|fix|refactor|docs|test|chore|build|ci|perf|style|revert)\([a-z0-9]([a-z0-9-]*[a-z0-9])?\)!?: [a-z]([[:print:]]*[^.])?$'

valid_subject() {
    subject=$1

    [ "${#subject}" -le 72 ] || return 1
    printf '%s\n' "$subject" | grep -Eq "$subject_pattern" || return 1
    ! printf '%s\n' "$subject" | grep -Eq '^[^(]+\(ember-'
}

self_test() {
    failures=0

    while IFS= read -r subject; do
        if ! valid_subject "$subject"; then
            printf 'rejected known-good subject: %s\n' "$subject" >&2
            failures=1
        fi
    done <<'GOOD'
feat(arena): add spectator controls
fix(arena-core)!: reject stale protocol frames
docs(workspace): explain release ownership
chore(deploy): refresh service metadata
GOOD

    while IFS= read -r subject; do
        if valid_subject "$subject"; then
            printf 'accepted known-bad subject: %s\n' "$subject" >&2
            failures=1
        fi
    done <<'BAD'
feature(arena): add spectator controls
feat(ember-arena): add spectator controls
feat(Arena): add spectator controls
feat(arena) add spectator controls
feat(arena): Add spectator controls
feat(arena): add spectator controls.
merge(arena): merge spectator controls
feat(arena): add spectator controls that make this deliberately overlong subject fail validation
BAD

    if [ "$failures" -ne 0 ]; then
        return 1
    fi

    printf 'commit-message self-test passed\n'
}

resolve_base() {
    candidate=${1:-origin/main}

    if git rev-parse --verify "${candidate}^{commit}" >/dev/null 2>&1; then
        printf '%s\n' "$candidate"
        return 0
    fi

    if ! git rev-parse --verify 'main^{commit}' >/dev/null 2>&1; then
        printf 'cannot resolve base %s or main\n' "$candidate" >&2
        return 1
    fi

    git merge-base main HEAD
}

if [ "${1:-}" = '--self-test' ]; then
    self_test
    exit $?
fi

base=$(resolve_base "${1:-}") || exit 2
failures=0

while IFS= read -r subject; do
    if ! valid_subject "$subject"; then
        printf '%s\n' "$subject"
        failures=1
    fi
done < <(git log --no-merges --format='%s' "$base..HEAD")

exit "$failures"
