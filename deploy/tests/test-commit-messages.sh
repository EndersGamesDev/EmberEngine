#!/usr/bin/env bash

set -u

start_ns=$(date +%s%N)
subjects_checked=0
subjects_rejected=0
script_path="$(cd "$(dirname "$0")" && pwd)/$(basename "$0")"

report_result() {
    exit_status=$?
    end_ns=$(date +%s%N)
    wall_ms=$(((end_ns - start_ns) / 1000000))
    printf 'test-commit-messages: %d subjects checked, %d rejected, wall %d ms\n' "$subjects_checked" "$subjects_rejected" "$wall_ms"
    return "$exit_status"
}

trap report_result EXIT

subject_pattern='^(feat|fix|refactor|docs|test|chore|build|ci|perf|style|revert)\([a-z0-9]([a-z0-9-]*[a-z0-9])?\)!?: [a-z]([[:print:]]*[^.])?$'

valid_subject() {
    subject=$1

    printf '%s\n' "$subject" | grep -Eq "$subject_pattern" || return 1
    ! printf '%s\n' "$subject" | grep -Eq '^[^(]+\(ember-' || return 1
    summary=${subject#*: }
    [ "${#summary}" -le 72 ]
}

self_test() {
    failures=0

    while IFS= read -r subject; do
        subjects_checked=$((subjects_checked + 1))
        if ! valid_subject "$subject"; then
            subjects_rejected=$((subjects_rejected + 1))
            printf 'rejected known-good subject: %s\n' "$subject" >&2
            failures=1
        fi
    done <<'GOOD'
feat(arena): add spectator controls
fix(arena-core)!: reject stale protocol frames
docs(workspace): explain release ownership
chore(deploy): refresh service metadata
docs(workspace): document why verification results belong in every commit body for review
GOOD

    while IFS= read -r subject; do
        subjects_checked=$((subjects_checked + 1))
        if valid_subject "$subject"; then
            printf 'accepted known-bad subject: %s\n' "$subject" >&2
            failures=1
        else
            subjects_rejected=$((subjects_rejected + 1))
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

    fixture="$(mktemp -d "${TMPDIR:?}/ember-commit-message-fixture.XXXXXX")" || return 1
    git -C "$fixture" init -q
    git -C "$fixture" config user.name 'Commit Message Test'
    git -C "$fixture" config user.email 'commit-message-test@example.invalid'
    git -C "$fixture" config commit.gpgsign false
    git -C "$fixture" commit --allow-empty -qm 'chore(test): seed base'
    root="$(git -C "$fixture" rev-parse HEAD)"
    git -C "$fixture" switch -q -c stale
    git -C "$fixture" commit --allow-empty -qm 'chore(test): stale side'
    git -C "$fixture" switch -q -c lane "$root"
    git -C "$fixture" commit --allow-empty -qm 'fix(test): check diverged base'

    if resolved="$(cd "$fixture" && resolve_base stale)" \
            && [ "$resolved" = stale ] \
            && (cd "$fixture" && bash "$script_path" stale >/dev/null); then
        printf 'diverged-base self-test: passed\n'
    else
        printf 'diverged-base self-test: failed\n' >&2
        failures=1
    fi
    rm -rf -- "$fixture"

    if [ "$failures" -ne 0 ]; then
        return 1
    fi

    return 0
}

resolve_base() {
    requested=${1:-origin/develop}

    for candidate in "$requested" origin/develop develop origin/main main; do
        if git rev-parse --verify "${candidate}^{commit}" >/dev/null 2>&1; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done

    printf 'cannot resolve commit-message base from %s, origin/develop, develop, origin/main or main\n' "$requested" >&2
    return 1
}

if [ "${1:-}" = '--self-test' ]; then
    self_test
    exit $?
fi

base=$(resolve_base "${1:-}") || exit 2
failures=0

while IFS= read -r subject; do
    subjects_checked=$((subjects_checked + 1))
    if ! valid_subject "$subject"; then
        subjects_rejected=$((subjects_rejected + 1))
        printf '%s\n' "$subject"
        failures=1
    fi
done < <(git log --no-merges --format='%s' "$base..HEAD")

exit "$failures"
