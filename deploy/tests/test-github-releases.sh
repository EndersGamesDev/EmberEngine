#!/usr/bin/env bash
# Verify changelog-backed GitHub release derivation without invoking gh.
#
#   bash deploy/tests/test-github-releases.sh
#   bash deploy/tests/test-github-releases.sh --self-test
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
RELEASE_SCRIPT="$ROOT/deploy/github-releases.sh"
TEST_WORK=""
CHECKS=0
FAILURES=0

# shellcheck source=deploy/github-releases.sh
. "$RELEASE_SCRIPT"

cleanup() {
    [ -z "$TEST_WORK" ] || rm -r -- "$TEST_WORK"
}

trap cleanup EXIT

ok() {
    CHECKS=$((CHECKS + 1))
    echo "ok: $*"
}

bad() {
    CHECKS=$((CHECKS + 1))
    FAILURES=$((FAILURES + 1))
    echo "FAIL: $*" >&2
}

test_timed() {
    local label="$1" began=$SECONDS status=0
    shift
    "$@" || status=$?
    printf 'test-github-releases: %s: %ss\n' "$label" "$((SECONDS - began))" >&2
    return "$status"
}

write_forbidden_gh() {
    local path="$1"
    {
        echo '#!/usr/bin/env bash'
        echo 'echo "test-github-releases: gh must not be called during derivation" >&2'
        echo 'exit 99'
    } > "$path"
    chmod +x "$path"
}

capture_plan() {
    GH="$TEST_WORK/gh-forbidden" bash "$RELEASE_SCRIPT" \
        > "$TEST_WORK/plan.out" 2> "$TEST_WORK/plan.err"
}

derive_repository_releases() {
    local tag notes
    while IFS= read -r tag; do
        [ -n "$tag" ] || continue
        notes="$TEST_WORK/notes/$tag.md"
        derive_release "$tag" "$notes"
        DERIVED_COUNT=$((DERIVED_COUNT + 1))

        if [ -n "$RELEASE_TITLE" ]; then
            ok "$tag has title '$RELEASE_TITLE'"
        else
            bad "$tag has no title"
        fi
        if [ -s "$notes" ]; then
            ok "$tag has non-empty notes"
        else
            bad "$tag has empty notes"
        fi
        if [ "$RELEASE_HAS_ENTRY" = true ]; then
            if grep -Fqx "$RELEASE_LINE" "$notes"; then
                ok "$tag notes contain its release line verbatim"
            else
                bad "$tag notes do not contain its release line verbatim"
            fi
        fi
        if [ "$RELEASE_LATEST" = true ]; then
            LATEST_COUNT=$((LATEST_COUNT + 1))
        fi
    done < "$TEST_WORK/tags"
}

write_fixture_games() {
    local path="$1"
    while IFS= read -r line; do
        printf '%s\n' "$line"
    done > "$path" <<'JSON'
{
  "games": [
    {
      "id": "fixture",
      "title": "Fixture Game",
      "versions": [
        {
          "v": "v2",
          "version": "2.0.0",
          "path": "games/fixture/v2/"
        },
        {
          "v": "v1",
          "version": "1.0.0",
          "path": "games/fixture/v1/"
        },
        {
          "v": "v0",
          "version": "0.1.0",
          "path": "games/fixture/v0/"
        }
      ]
    }
  ]
}
JSON
}

write_fixture_changelog() {
    local path="$1" commit="$2"
    {
        printf '# Fixture changelog\n\n'
        printf '## Fixture Game\n\n'
        printf '### v1 — 2026-09-08\n\n'
        printf 'no proto · stamp — · source `%s` · tag `fixture-1.0.0`\n\n' "$commit"
        printf 'Fixture release prose, retained verbatim.\n'
    } > "$path"
}

self_test() {
    local started=$SECONDS fixture commit notes release_line
    fixture="$(mktemp -d "${TMPDIR:?}/ember-github-release-fixture.XXXXXX")"
    TEST_WORK="$fixture"
    mkdir -p "$fixture/web" "$fixture/notes"

    git -C "$fixture" init -q
    git -C "$fixture" config user.name Fixture
    git -C "$fixture" config user.email fixture@example.invalid
    git -C "$fixture" config commit.gpgsign false
    git -C "$fixture" config tag.gpgsign false
    git -C "$fixture" commit --allow-empty -qm 'fixture source'
    commit="$(git -C "$fixture" rev-parse HEAD)"
    git -C "$fixture" tag -a fixture-1.0.0 -m 'fixture annotation with changelog' "$commit"
    git -C "$fixture" tag -a fixture-2.0.0 -m 'fixture annotation without changelog' "$commit"
    git -C "$fixture" tag -a fixture-0.1.0 -m 'fixture pre-release annotation' "$commit"
    write_fixture_games "$fixture/web/games.json"
    write_fixture_changelog "$fixture/CHANGELOG.md" "$commit"

    REPO="$fixture"
    CHANGELOG="$fixture/CHANGELOG.md"
    GAMES="$fixture/web/games.json"
    LATEST_TAG=""

    notes="$fixture/notes/fixture-1.0.0.md"
    derive_release fixture-1.0.0 "$notes"
    release_line="no proto · stamp — · source \`$commit\` · tag \`fixture-1.0.0\`"
    [ "$RELEASE_TITLE" = "Fixture Game 1.0.0" ] && ok "fixture title derives from launcher" || bad "fixture title is '$RELEASE_TITLE'"
    [ "$RELEASE_PATH" = "games/fixture/v1/" ] && ok "fixture entry selects its launcher slot" || bad "fixture path is '$RELEASE_PATH'"
    grep -Fqx "$release_line" "$notes" && ok "fixture release line is verbatim" || bad "fixture release line changed"
    grep -Fqx 'Fixture release prose, retained verbatim.' "$notes" && ok "fixture prose is verbatim" || bad "fixture prose changed"

    notes="$fixture/notes/fixture-2.0.0.md"
    derive_release fixture-2.0.0 "$notes"
    grep -Fqx 'No matching `CHANGELOG.md` entry exists for `fixture-2.0.0`.' "$notes" && ok "missing fixture entry is stated" || bad "missing fixture entry is not stated"
    grep -Fqx 'fixture annotation without changelog' "$notes" && ok "missing fixture entry names its tag annotation" || bad "missing fixture annotation is absent"
    [ "$RELEASE_PATH" = "games/fixture/v2/" ] && ok "missing fixture entry selects its version path" || bad "missing fixture path is '$RELEASE_PATH'"

    notes="$fixture/notes/fixture-0.1.0.md"
    derive_release fixture-0.1.0 "$notes"
    [ "$RELEASE_PRERELEASE" = true ] && ok "fixture 0.x tag is a pre-release" || bad "fixture 0.x tag is not a pre-release"

    if [ "$FAILURES" -eq 0 ]; then
        echo "SELF-TEST PASS: $CHECKS checks, 3 tags, 0 failures, $((SECONDS - started))s wall"
        return 0
    fi
    echo "SELF-TEST FAIL: $CHECKS checks, 3 tags, $FAILURES failure(s), $((SECONDS - started))s wall" >&2
    return 1
}

if [ "${1:-}" = "--self-test" ]; then
    [ "$#" -eq 1 ] || { echo "usage: bash deploy/tests/test-github-releases.sh [--self-test]" >&2; exit 2; }
    self_test
    exit $?
fi
[ "$#" -eq 0 ] || { echo "usage: bash deploy/tests/test-github-releases.sh [--self-test]" >&2; exit 2; }

started=$SECONDS
TEST_WORK="$(mktemp -d "${TMPDIR:?}/ember-github-release-test.XXXXXX")"
mkdir -p "$TEST_WORK/notes"
write_forbidden_gh "$TEST_WORK/gh-forbidden"

REPO="$ROOT"
CHANGELOG="$ROOT/CHANGELOG.md"
GAMES="$ROOT/web/games.json"
LATEST_TAG="$(latest_arena_tag)"
release_tags > "$TEST_WORK/tags"
tag_count="$(wc -l < "$TEST_WORK/tags")"
DERIVED_COUNT=0
LATEST_COUNT=0

if test_timed "dry-run plan" capture_plan; then
    ok "dry-run completed without invoking gh"
else
    bad "dry-run failed or invoked gh"
    sed -n '1,20p' "$TEST_WORK/plan.err" >&2
fi

awk '$1 == "github-releases:" && $2 == "plan" && $3 == "release" { print $4 }' \
    "$TEST_WORK/plan.out" > "$TEST_WORK/planned"
LC_ALL=C sort "$TEST_WORK/tags" > "$TEST_WORK/tags.sorted"
LC_ALL=C sort "$TEST_WORK/planned" > "$TEST_WORK/planned.sorted"
plan_count="$(wc -l < "$TEST_WORK/planned")"
if cmp -s "$TEST_WORK/tags.sorted" "$TEST_WORK/planned.sorted"; then
    ok "planned set equals the $tag_count-tag repository set"
else
    bad "planned set differs from the repository tag set"
    diff -u "$TEST_WORK/tags.sorted" "$TEST_WORK/planned.sorted" >&2 || true
fi

test_timed "derive repository releases" derive_repository_releases

if [ "$DERIVED_COUNT" -eq "$tag_count" ]; then
    ok "derived all $tag_count repository tags"
else
    bad "derived $DERIVED_COUNT of $tag_count repository tags"
fi
if [ "$LATEST_COUNT" -eq 1 ]; then
    ok "exactly one release is latest ($LATEST_TAG)"
else
    bad "$LATEST_COUNT releases are latest, expected one"
fi

if [ "$FAILURES" -eq 0 ]; then
    echo "github-releases: $CHECKS checks, 0 failures, $tag_count tags, $plan_count plans, $LATEST_COUNT latest, $((SECONDS - started))s wall"
    exit 0
fi
echo "github-releases: $CHECKS checks, $FAILURES failure(s), $tag_count tags, $plan_count plans, $LATEST_COUNT latest, $((SECONDS - started))s wall" >&2
exit 1
