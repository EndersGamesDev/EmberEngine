#!/usr/bin/env bash
# CHANGELOG.md against the launcher, the object store, the legacy published
# history and the tags.
#
#   bash deploy/tests/test-changelog.sh
#   bash deploy/tests/test-changelog.sh --tag arena-31.0.0
#   bash deploy/tests/test-changelog.sh --self-test
#
# The changelog is a claim about history, and a claim about history rots
# silently: a version added to web/games.json with no entry here, an entry
# naming a commit that a rebase left unreachable, a tag moved to a different
# commit than the entry records, a protocol that drifts from the launcher's.
# None of that shows up when the file is read, and all of it shows up here.
#
# The first version of this suite checked only the SHAPE of an entry, and an
# adversarial review broke it in one line: an arena v18 row naming a
# protocol-14 commit as the source of a protocol-15 release passed, because
# the commit existed and the tag pointed at it. Shape is not evidence. Every
# stamp row now names the publication that establishes it, and the suite reads
# that publication back off the published branch.
#
# The grammar is one release line per entry, immediately after the entry's
# heading, four or five ` · `-separated fields:
#
#   <proto> · <stamp> · <source> · <tag>[ · <published>]
#
#   <proto>     proto <n>
#               proto <n> (launcher says proto <m>)   the published release
#                                                     and web/games.json
#                                                     disagree; <m> must be
#                                                     the launcher's value and
#                                                     <n> must be the value in
#                                                     the published catalogue
#               no proto
#   <stamp>     stamp <r…>  |  stamp —
#   <source>    source `<sha>`
#               source `<sha>` (not on main)
#               source not in repository (`<sha>`)
#               source not recorded
#   <tag>       tag `<series>-MAJOR.MINOR.PATCH`
#               tag `<series>-MAJOR.MINOR.PATCH` (points at `<sha>`)
#               tag `<series>-MAJOR.MINOR.PATCH` (pending)
#               no tag
# A pending row names the authored version commit because the GitHub merge
# commit does not exist yet and must carry stamp —. Once its tag exists, that
# tag must target the two-parent merge commit that integrated the row: the
# first parent lacks the exact release line, the merge carries it, and the
# source is reachable from the second parent. An Actions-era completed row has
# no published field, names that tag commit as its source and carries the
# deterministic stamp r<rev-list count of the tag commit>. The eight Arena
# tags from arena-3.0.0 through arena-10.0.0 are immutable exceptions: they
# predate build stamps and have no publication to check. Historical rows with
# a published field retain their publication-derived stamp checks.
# The ledger is a flat document with a closed whitelist of nonblank lines:
#
#   1. exactly `# Changelog`;
#   2. `## ` plus nonempty heading text that does not start with `#` or end
#      with a whitespace-separated closing run of `#` characters;
#   3. `### ` plus the same heading text, with every non-Pending heading using
#      the `### <version> — <title>` entry form and every Pending heading
#      remaining a topical subsection from which no row is extracted;
#   4. the first nonblank line after each non-Pending entry heading, which
#      must be the single release line in the grammar above; or
#   5. prose beginning with an ASCII letter, `"` or `**`, where `**` is
#      immediately followed by a non-asterisk, non-space character.
#
# Angle brackets are forbidden anywhere, and a line made only from three or
# more asterisks and whitespace is forbidden. Release-looking text outside
# the positional line is prose. The line-oriented checker and GitHub's
# renderer must agree about which lines are entries, so every other line shape
# is invalid rather than interpreted as another Markdown construct.
# A version the launcher lists but the published branch does not carry yet is
# a legitimate state, not a gap: it takes `source not recorded · no tag` and
# omits the published field, and the selection and publication checks pass
# over it. Nothing else is relaxed for it — it still needs an entry, and its
# protocol is still compared with the launcher's.
#
#   <published> published `<sha>`                  root version.json at that
#                                                  publication names the source
#               published `<sha>` (message)        the publication's own subject
#                                                  names the source
#               published `<sha>` (release stamp)  version.json inside the
#                                                  release's page tree names it
#
# Nothing here contacts a host, a tunnel or a network.
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
# shellcheck source=deploy/tests/lib.sh
. "$HERE/lib.sh"

# The section heading each game's entries live under, and the published path
# its page tree occupies. A game listed in the launcher and missing here is a
# game whose section nobody named, which is itself a failure.
section_for() {
    case "$1" in
        arena)        echo "Killshot (arena)" ;;
        end-game)     echo "End Game" ;;
        league)       echo "UltimateLegue (league)" ;;
        fire)         echo "Fire Racer" ;;
        kings)        echo "Four Kings" ;;
        what-is-this) echo "what is this?" ;;
        julibrot)     echo "Julibrot Lab" ;;
        *)            echo "" ;;
    esac
}

parse_changelog() {
    local source="$1" parsed="$2"

    "$PY" - "$source" > "$parsed" <<'PY'
import re, sys

src, = sys.argv[1:]
lines = open(src, encoding="utf-8").read().split("\n")


def parse_release_fields(release):
    fields = release.split(" · ")
    if len(fields) not in (4, 5):
        return None, "release line has %d fields, want 4 or 5" % len(fields)
    proto, stamp, source, tag = fields[:4]
    published = fields[4] if len(fields) == 5 else ""

    pnum = plauncher = "-"
    if proto == "no proto":
        pass
    elif re.fullmatch(r"proto \d+", proto):
        pnum = proto.split()[1]
    elif re.fullmatch(r"proto \d+ \(launcher says proto \d+\)", proto):
        parts = proto.replace("(", "").replace(")", "").split()
        pnum, plauncher = parts[1], parts[5]
    else:
        return None, "bad protocol field %r" % proto

    if not re.fullmatch(r"stamp (—|r\d+(\+dirty)?)", stamp):
        return None, "bad stamp field %r" % stamp

    kind = sha = "-"
    if re.fullmatch(r"source `[0-9a-f]{7,40}`", source):
        kind, sha = "sha", source.split("`")[1]
    elif re.fullmatch(r"source `[0-9a-f]{7,40}` \(not on main\)", source):
        kind, sha = "offmain", source.split("`")[1]
    elif re.fullmatch(r"source not in repository \(`[0-9a-f]{7,40}`\)", source):
        kind, sha = "absent", source.split("`")[1]
    elif source == "source not recorded":
        kind = "unrecorded"
    else:
        return None, "bad source field %r" % source

    name = target = "-"
    if tag == "no tag":
        pass
    elif re.fullmatch(r"tag `[^`]+`", tag):
        name = tag.split("`")[1]
    elif re.fullmatch(r"tag `[^`]+` \(points at `[0-9a-f]{7,40}`\)", tag):
        parts = tag.split("`")
        name, target = parts[1], parts[3]
    elif re.fullmatch(r"tag `[^`]+` \(pending\)", tag):
        name, target = tag.split("`")[1], "pending"
    else:
        return None, "bad tag field %r" % tag
    if name != "-" and not re.fullmatch(
            r"[a-z0-9-]+-[0-9]+\.[0-9]+\.[0-9]+", name):
        return None, "tag is not a series-prefixed three-grade version: %r" % name

    pubsha, pubkind = "-", "-"
    if published:
        match = re.fullmatch(
            r"published `([0-9a-f]{7,40})`(?: \((message|release stamp)\))?",
            published,
        )
        if not match:
            return None, "bad published field %r" % published
        pubsha = match.group(1)
        pubkind = {None: "root", "message": "message",
                   "release stamp": "relstamp"}[match.group(2)]

    return (pnum, plauncher, stamp, kind, sha, name, target,
            pubsha, pubkind), None


rows, bad = [], []
excluded = [False] * len(lines)


def reject_line(index, line, reason):
    bad.append("line %d: %s: %r" % (index + 1, reason, line))
    excluded[index] = True


def allowed_heading_text(text):
    return (bool(text) and not text.startswith("#") and
            not re.search(r"\s#+\s*$", text))


validation_section = ""
for index, line in enumerate(lines):
    if not line:
        continue
    if "<" in line or ">" in line:
        reject_line(index, line, "raw HTML and angle brackets are not part of the ledger")
        continue
    if line == "# Changelog":
        continue
    if line.startswith("## ") and not line.startswith("### "):
        heading = line[3:]
        if allowed_heading_text(heading):
            validation_section = heading.strip()
        else:
            reject_line(index, line, "line does not match the closed changelog whitelist")
        continue
    if line.startswith("### "):
        heading = line[4:]
        if not allowed_heading_text(heading):
            reject_line(index, line, "line does not match the closed changelog whitelist")
        elif validation_section != "Pending" and not re.fullmatch(r"\S+ — .+", heading):
            reject_line(index, line, "non-Pending level-three heading must be a release entry")
        continue
    if line.startswith("#"):
        reject_line(index, line, "line does not match the closed changelog whitelist")
        continue
    if re.match(r"[A-Za-z\"]", line):
        continue
    if (line.count("*") >= 3 and
            all(character == "*" or character.isspace() for character in line)):
        reject_line(index, line, "asterisk-only lines are not part of the ledger")
        continue
    if (line.startswith("**") and len(line) > 2 and
            line[2] != "*" and not line[2].isspace()):
        continue
    reject_line(index, line, "line does not match the closed changelog whitelist")

section = ""
i = 0
while i < len(lines):
    if excluded[i]:
        i += 1
        continue
    line = lines[i]
    if line.startswith("## ") and not line.startswith("### "):
        section = line[3:].strip()
        i += 1
        continue
    if section == "Pending" and line.startswith("### "):
        i += 1
        continue
    m = re.match(r"^### (\S+) — (.+)$", line)
    if not m:
        i += 1
        continue
    version, date = m.group(1), m.group(2).strip()
    j = i + 1
    while j < len(lines) and not lines[j]:
        j += 1
    release = lines[j] if j < len(lines) else ""
    i = j + 1

    data, error = parse_release_fields(release)
    if error:
        bad.append("%s %s: %s: %r" % (section, version, error, release))
        continue
    pnum, plauncher, stamp, kind, sha, name, target, pubsha, pubkind = data

    rows.append((section, version, pnum, plauncher, stamp, kind, sha,
                 name, target, pubsha, pubkind, date, release))

for line in bad:
    print("BAD\t%s" % line)
for row in rows:
    print("ROW\t" + "\t".join(row))
PY
}

grandfathered_unstamped_tag() {
    # These eight Arena tags were tagged before deploy/stamp-version.sh existed and have no publication to check.
    case "$1" in
        arena-3.0.0|arena-4.0.0|arena-5.0.0|arena-6.0.0|arena-7.0.0|arena-8.0.0|arena-9.0.0|arena-10.0.0) return 0 ;;
        *) return 1 ;;
    esac
}

check_named_tags() {
    local repo="$1" parsed="$2"
    local checked=0 failures_before="$TESTS_FAILED"
    local want at expect objtype tag_changelog first_changelog parents
    local tagged_commit first_parent second_parent extra expected_stamp

    while IFS=$'\t' read -r _ section version _ _ stamp _ sha name target pubsha _ _ release; do
        [ -n "$section" ] || continue
        [ "$name" != "-" ] || continue
        want="$target"
        [ "$want" != "-" ] || want="$sha"
        if [ "$target" = "pending" ]; then
            if [ "$stamp" = "stamp —" ]; then
                ok "$section $version: pending row uses stamp —"
            else
                bad "$section $version: pending row must use stamp —, got $stamp"
            fi
        fi
        if ! git -C "$repo" rev-parse -q --verify "refs/tags/$name" >/dev/null 2>&1; then
            if [ "$target" = "pending" ]; then
                ok "$section $version is pending; tag $name not yet created"
            else
                bad "$section $version: tag $name does not exist"
            fi
            continue
        fi
        checked=$((checked + 1))
        at="$(git -C "$repo" rev-parse "refs/tags/$name^{commit}" 2>/dev/null)"
        if [ "$target" = "pending" ]; then
            objtype="$(git -C "$repo" cat-file -t "refs/tags/$name" 2>/dev/null)"
            if [ "$objtype" = "tag" ]; then
                ok "$section $version: tag $name is an annotated tag object"
            else
                bad "$section $version: tag $name is a $objtype, not an annotated tag object"
            fi
            if tag_changelog="$(git -C "$repo" show "refs/tags/$name^{commit}:CHANGELOG.md" 2>/dev/null)" \
                    && grep -Fxq -- "$release" <<< "$tag_changelog"; then
                ok "$section $version: tag $name contains its exact release line"
            else
                bad "$section $version: tag $name does not contain its exact release line: $release"
            fi

            parents="$(git -C "$repo" rev-list --parents -n 1 "refs/tags/$name^{commit}" 2>/dev/null)"
            read -r tagged_commit first_parent second_parent extra <<< "$parents"
            if [ -n "$tagged_commit" ] && [ -n "$first_parent" ] \
                    && [ -n "$second_parent" ] && [ -z "$extra" ]; then
                ok "$section $version: tag $name targets a two-parent merge commit"
            else
                bad "$section $version: tag $name does not target a two-parent merge commit"
            fi
            if [ -n "$first_parent" ]; then
                first_changelog="$(git -C "$repo" show "$first_parent:CHANGELOG.md" 2>/dev/null || true)"
                if grep -Fxq -- "$release" <<< "$first_changelog"; then
                    bad "$section $version: tag $name's first parent already contains its exact release line"
                else
                    ok "$section $version: tag $name's first parent does not contain its exact release line"
                fi
            else
                bad "$section $version: tag $name has no first parent to check for the release line"
            fi
            if [ -n "$second_parent" ] \
                    && git -C "$repo" merge-base --is-ancestor "$sha" "$second_parent" 2>/dev/null; then
                ok "$section $version: source $sha is an ancestor of tag $name's second parent"
            else
                bad "$section $version: source $sha is not an ancestor of tag $name's second parent"
            fi
            continue
        fi
        expect="$(git -C "$repo" rev-parse "$want^{commit}" 2>/dev/null || echo "?")"
        if [ "$at" = "$expect" ]; then
            ok "$section $version: tag $name points at $want"
        else
            bad "$section $version: tag $name points at $at, entry says $want ($expect)"
        fi
        objtype="$(git -C "$repo" cat-file -t "refs/tags/$name" 2>/dev/null)"
        if [ "$objtype" = "tag" ]; then
            ok "$section $version: tag $name is an annotated tag object"
        else
            bad "$section $version: tag $name is a $objtype, not an annotated tag object"
        fi
        if [ "$pubsha" = "-" ]; then
            expected_stamp="stamp r$(git -C "$repo" rev-list --count "$at")"
            if [ "$stamp" = "$expected_stamp" ]; then
                ok "$section $version: $stamp matches tag $name's commit count"
            elif [ "$stamp" = "stamp —" ] && grandfathered_unstamped_tag "$name"; then
                ok "$section $version: tag $name is grandfathered without a build stamp"
            else
                bad "$section $version: $stamp does not match tag $name; expected $expected_stamp"
            fi
        fi
    done < <(grep '^ROW' "$parsed")
    echo "   ($checked tag(s) checked)"
    [ "$TESTS_FAILED" -eq "$failures_before" ]
}

tag_fixture_passes() {
    TESTS_RUN=0
    TESTS_FAILED=0
    check_named_tags "$1" "$2"
}

check_tag_fixture() {
    local expected="$1" repo="$2" rows="$3" needle="$4" label="$5"
    local output actual="reject"

    if output="$(tag_fixture_passes "$repo" "$rows" 2>&1)"; then
        actual="accept"
    fi
    case "$output" in
        *"$needle"*) ;;
        *) actual="$actual with the wrong report" ;;
    esac
    if [ "$actual" = "$expected" ]; then
        ok "$label"
    else
        bad "$label — expected $expected with '$needle', got $actual: $output"
    fi
}

check_requested_tag() {
    local parsed="$1" launcher="$2" tag="$3" count row_section row_version
    local tag_version series expected_section launcher_version
    local failures_before="$TESTS_FAILED"

    count="$(awk -F '\t' -v wanted="$tag" '$1 == "ROW" && $9 == wanted { found++ } END { print found + 0 }' "$parsed")"
    if [ "$count" -eq 1 ]; then
        ok "CHANGELOG.md has exactly one accepted entry for tag $tag"
    elif [ "$count" -eq 0 ]; then
        bad "CHANGELOG.md has no accepted entry for tag $tag"
    else
        bad "CHANGELOG.md has $count accepted entries for tag $tag, expected one"
    fi
    if [ "$count" -ne 1 ]; then
        return 1
    fi

    IFS=$'\t' read -r row_section row_version < <(
        awk -F '\t' -v wanted="$tag" '$1 == "ROW" && $9 == wanted { print $2 "\t" $3 }' "$parsed"
    )
    tag_version="${tag##*-}"
    series="${tag%-$tag_version}"
    expected_section="$(section_for "$series")"
    if [ -n "$expected_section" ] && [ "$row_section" = "$expected_section" ]; then
        ok "tag $tag has its entry under '$expected_section'"
    else
        bad "tag $tag has its entry under '$row_section', expected '$expected_section'"
    fi

    launcher_version="$(awk -F '\t' -v wanted_gid="$series" -v wanted_slot="$row_version" \
        '$1 == wanted_gid && $2 == wanted_slot { print $5 }' "$launcher")"
    if [ "$launcher_version" = "$tag_version" ]; then
        ok "tag $tag matches launcher $series $row_version version $launcher_version"
    else
        [ -n "$launcher_version" ] || launcher_version="not found"
        bad "tag $tag says version $tag_version, launcher $series $row_version says $launcher_version"
    fi

    [ "$TESTS_FAILED" -eq "$failures_before" ]
}

tag_entry_fixture_passes() {
    TESTS_RUN=0
    TESTS_FAILED=0
    check_requested_tag "$1" "$2" "$3"
}

check_tag_entry_fixture() {
    local expected="$1" rows="$2" launcher="$3" tag="$4" needle="$5" label="$6"
    local output actual="reject"

    if output="$(tag_entry_fixture_passes "$rows" "$launcher" "$tag" 2>&1)"; then
        actual="accept"
    fi
    case "$output" in
        *"$needle"*) ;;
        *) actual="$actual with the wrong report" ;;
    esac
    if [ "$actual" = "$expected" ]; then
        ok "$label"
    else
        bad "$label — expected $expected with '$needle', got $actual: $output"
    fi
}

check_parser_fixture() {
    local expected="$1" source="$2" parsed="$3" needle="$4" label="$5"
    local actual="accept"

    parse_changelog "$source" "$parsed"
    if grep -q '^BAD' "$parsed"; then
        actual="reject"
    fi
    if [ -n "$needle" ] && ! grep -Fq -- "$needle" "$parsed"; then
        actual="$actual with the wrong report"
    fi
    if [ "$actual" = "$expected" ]; then
        ok "$label"
    else
        bad "$label — expected $expected with '$needle', got $actual: $(tr '\n' ' ' < "$parsed")"
    fi
}

check_published_stamp_source() {
    local kind="$1" recorded="$2" got="$3" label="${4:-the published stamp}"
    local objects a b

    if [ "$kind" = "absent" ]; then
        bad "$label names $got while the source is marked absent"
        return
    fi
    if [ "$kind" = "offmain" ]; then
        objects="$(git -C "$REPO" rev-parse --disambiguate="$recorded" 2>/dev/null)" || {
            bad "$label could not query object presence for $recorded"
            return
        }
        if [ -z "$objects" ]; then
            if [ "$got" = "$recorded" ]; then
                ok "$label names the recorded off-main source $recorded and this checkout does not carry the object"
            else
                bad "$label names $got, entry says $recorded"
            fi
            return
        fi
        if ! git -C "$REPO" rev-parse --verify "$recorded^{commit}" >/dev/null 2>&1; then
            bad "$label has recorded name $recorded, which is present but does not name a unique commit (ambiguous, or not a commit)"
            return
        fi
    fi
    a="$(git -C "$REPO" rev-parse "$got^{commit}" 2>/dev/null || echo A)"
    b="$(git -C "$REPO" rev-parse "$recorded^{commit}" 2>/dev/null || echo B)"
    if [ "$a" = "$b" ]; then
        ok "$label names $recorded"
    else
        bad "$label names $got, entry says $recorded"
    fi
}

published_stamp_fixture_passes() {
    local repo="$1"
    shift
    TESTS_RUN=0
    TESTS_FAILED=0
    REPO="$repo"
    check_published_stamp_source "$@" "the fixture stamp"
    [ "$TESTS_FAILED" -eq 0 ]
}

check_published_stamp_fixture() {
    local expected="$1" repo="$2" kind="$3" recorded="$4" got="$5"
    local needle="$6" label="$7" output actual="reject"

    if output="$(published_stamp_fixture_passes "$repo" "$kind" "$recorded" "$got" 2>&1)"; then
        actual="accept"
    fi
    case "$output" in
        *"$needle"*) ;;
        *) actual="$actual with the wrong report" ;;
    esac
    if [ "$actual" = "$expected" ]; then
        ok "$label"
    else
        bad "$label — expected $expected with '$needle', got $actual: $output"
    fi
}

self_test() {
    local started tmp commit other rows output missing other_missing blob not_repo
    local collision prefix first second first_hash second_hash failures_before
    local base_branch launcher wrong_section_rows mismatched_launcher
    local whitelist_changelog whitelist_parsed
    local fenced_changelog fenced_parsed tilde_changelog nested_changelog
    local indented_changelog unclosed_changelog list_fence_changelog list_unclosed_changelog
    local quoted_changelog invalid_opener_changelog
    local completed_rows completed_bad_rows completed_release completed_bad_release completed_stamp
    local completed_dash_rows completed_dash_release grandfather_rows grandfather_release
    local pending_rows pending_bad_rows pending_release pending_bad_release
    local pending_source pending_head integrating_merge later_merge tree
    local unrelated_source unrelated_rows unrelated_release unrelated_merge
    started="$(date +%s)"
    failures_before="$TESTS_FAILED"
    tmp="$(mktemp -d -t ember-changelogtest-XXXXXX)"
    CHANGELOG_TEST_TMP="$tmp"
    trap '[ -z "${CHANGELOG_TEST_TMP:-}" ] || rm -rf "$CHANGELOG_TEST_TMP"' EXIT

    # The ledger grammar uses 40-character object names, so the fixture repository is SHA-1 regardless of Git's configured default.
    git -C "$tmp" init -q --object-format=sha1
    git -C "$tmp" config user.name Fixture
    git -C "$tmp" config user.email fixture@example.invalid
    git -C "$tmp" config commit.gpgsign false
    git -C "$tmp" config tag.gpgsign false
    git -C "$tmp" commit --allow-empty -qm 'fixture commit'
    commit="$(git -C "$tmp" rev-parse HEAD)"
    base_branch="$(git -C "$tmp" symbolic-ref --short HEAD)"
    completed_stamp="r$(git -C "$tmp" rev-list --count "$commit")"
    rows="$tmp/rows"
    printf 'ROW\tKillshot (arena)\tv20\t-\t-\tstamp %s\t-\t%s\tarena-20.0.0\t-\t-\t-\t-\t-\n' \
        "$completed_stamp" "$commit" > "$rows"
    launcher="$tmp/launcher"
    printf 'arena\tv20\t-\tgames/arena/v20/\t20.0.0\n' > "$launcher"

    git -C "$tmp" tag -a arena-20.0.0 -m 'fixture tag' "$commit"
    if ! output="$(tag_fixture_passes "$tmp" "$rows" 2>&1)"; then
        bad "SELF-TEST FAIL: an exact annotated tag was rejected: $output"
        return 1
    fi

    completed_release="no proto · stamp $completed_stamp · source \`$commit\` · tag \`arena-20.0.0\`"
    completed_rows="$tmp/completed-rows"
    printf 'ROW\tFixture\tv20\t-\t-\tstamp %s\tsha\t%s\tarena-20.0.0\t-\t-\t-\t2026-09-11\t%s\n' \
        "$completed_stamp" "$commit" "$completed_release" > "$completed_rows"
    check_tag_fixture accept "$tmp" "$completed_rows" \
        "stamp $completed_stamp matches tag arena-20.0.0's commit count" \
        "self-test accepts a completed Actions-era row with the tag commit's deterministic stamp"

    completed_bad_rows="$tmp/completed-bad-rows"
    completed_bad_release="no proto · stamp r999999 · source \`$commit\` · tag \`arena-20.0.0\`"
    printf 'ROW\tFixture\tv20\t-\t-\tstamp r999999\tsha\t%s\tarena-20.0.0\t-\t-\t-\t2026-09-11\t%s\n' \
        "$commit" "$completed_bad_release" > "$completed_bad_rows"
    check_tag_fixture reject "$tmp" "$completed_bad_rows" \
        "stamp r999999 does not match tag arena-20.0.0; expected stamp $completed_stamp" \
        "self-test rejects a completed Actions-era row with the wrong deterministic stamp"

    git -C "$tmp" tag -a julibrot-1.1.0 -m 'fixture completed Julibrot tag' "$commit"
    completed_dash_release="no proto · stamp — · source \`$commit\` · tag \`julibrot-1.1.0\`"
    completed_dash_rows="$tmp/completed-dash-rows"
    printf 'ROW\tJulibrot Lab\tv1\t-\t-\tstamp —\tsha\t%s\tjulibrot-1.1.0\t-\t-\t-\t2026-09-11\t%s\n' \
        "$commit" "$completed_dash_release" > "$completed_dash_rows"
    check_tag_fixture reject "$tmp" "$completed_dash_rows" \
        "stamp — does not match tag julibrot-1.1.0; expected stamp $completed_stamp" \
        "self-test rejects a completed Julibrot row with a dash stamp"
    git -C "$tmp" tag -d julibrot-1.1.0 >/dev/null

    git -C "$tmp" tag -a arena-3.0.0 -m 'fixture grandfathered Arena tag' "$commit"
    grandfather_release="no proto · stamp — · source \`$commit\` · tag \`arena-3.0.0\`"
    grandfather_rows="$tmp/grandfather-rows"
    printf 'ROW\tKillshot (arena)\tv3\t-\t-\tstamp —\tsha\t%s\tarena-3.0.0\t-\t-\t-\t2026-09-11\t%s\n' \
        "$commit" "$grandfather_release" > "$grandfather_rows"
    check_tag_fixture accept "$tmp" "$grandfather_rows" \
        "tag arena-3.0.0 is grandfathered without a build stamp" \
        "self-test accepts a grandfathered Arena row with a dash stamp"
    git -C "$tmp" tag -d arena-3.0.0 >/dev/null

    git -C "$tmp" tag -d arena-20.0.0 >/dev/null
    git -C "$tmp" tag v20 "$commit"
    if output="$(tag_fixture_passes "$tmp" "$rows" 2>&1)"; then
        bad "SELF-TEST FAIL: a legacy-only tag was accepted: $output"
        return 1
    fi
    case "$output" in
        *"tag arena-20.0.0 does not exist"*) ;;
        *) bad "SELF-TEST FAIL: wrong legacy-only report: $output"; return 1 ;;
    esac

    git -C "$tmp" tag -d v20 >/dev/null
    if output="$(tag_fixture_passes "$tmp" "$rows" 2>&1)"; then
        bad "SELF-TEST FAIL: an absent named tag was accepted: $output"
        return 1
    fi
    case "$output" in
        *"tag arena-20.0.0 does not exist"*) ;;
        *) bad "SELF-TEST FAIL: wrong absent-tag report: $output"; return 1 ;;
    esac

    git -C "$tmp" tag arena-20.0.0 "$commit"
    if output="$(tag_fixture_passes "$tmp" "$rows" 2>&1)"; then
        bad "SELF-TEST FAIL: a lightweight named tag was accepted: $output"
        return 1
    fi
    case "$output" in
        *"not an annotated tag object"*) ;;
        *) bad "SELF-TEST FAIL: wrong lightweight-tag report: $output"; return 1 ;;
    esac

    if ! output="$(tag_entry_fixture_passes "$rows" "$launcher" arena-20.0.0 2>&1)"; then
        bad "SELF-TEST FAIL: an exact changelog tag entry was rejected: $output"
        return 1
    fi
    if output="$(tag_entry_fixture_passes "$rows" "$launcher" arena-21.0.0 2>&1)"; then
        bad "SELF-TEST FAIL: an entry-less tag was accepted: $output"
        return 1
    fi
    case "$output" in
        *"no accepted entry for tag arena-21.0.0"*) ;;
        *) bad "SELF-TEST FAIL: wrong entry-less tag report: $output"; return 1 ;;
    esac

    wrong_section_rows="$tmp/wrong-section-rows"
    printf 'ROW\tJulibrot Lab\tv20\t-\t-\t-\t-\t%s\tarena-20.0.0\t-\t-\t-\t-\t-\n' \
        "$commit" > "$wrong_section_rows"
    check_tag_entry_fixture reject "$wrong_section_rows" "$launcher" arena-20.0.0 \
        "has its entry under 'Julibrot Lab', expected 'Killshot (arena)'" \
        "self-test rejects a requested tag under the wrong section"

    mismatched_launcher="$tmp/mismatched-launcher"
    printf 'arena\tv20\t-\tgames/arena/v20/\t20.0.1\n' > "$mismatched_launcher"
    check_tag_entry_fixture reject "$rows" "$mismatched_launcher" arena-20.0.0 \
        "tag arena-20.0.0 says version 20.0.0, launcher arena v20 says 20.0.1" \
        "self-test rejects a requested tag whose launcher version differs"

    whitelist_changelog="$tmp/whitelist-changelog"
    whitelist_parsed="$tmp/whitelist-parsed"

    printf '# Changelog\n' > "$whitelist_changelog"
    check_parser_fixture accept "$whitelist_changelog" "$whitelist_parsed" "" \
        "self-test accepts the exact title line"

    printf '# Changelog\n\n## Pending\n' > "$whitelist_changelog"
    check_parser_fixture accept "$whitelist_changelog" "$whitelist_parsed" "" \
        "self-test accepts a section heading"

    printf '# Changelog\n\n## Pending\n\n### Integration and release process\n' > "$whitelist_changelog"
    check_parser_fixture accept "$whitelist_changelog" "$whitelist_parsed" "" \
        "self-test accepts a Pending topical subsection"

    printf '# Changelog\n\n## Pending\n\n### CI — release tooling\n\nAlpha prose.\n' > "$whitelist_changelog"
    check_parser_fixture accept "$whitelist_changelog" "$whitelist_parsed" "" \
        "self-test accepts an em-dash Pending topical subsection without extracting a row"

    printf '# Changelog\n\n## C#\n' > "$whitelist_changelog"
    check_parser_fixture accept "$whitelist_changelog" "$whitelist_parsed" "" \
        "self-test accepts an attached literal hash in heading text"

    {
        printf '# Changelog\n\n## Killshot (arena)\n\n### v20 — 2026-09-11\n\n'
        printf 'no proto · stamp — · source not recorded · no tag\n'
    } > "$whitelist_changelog"
    check_parser_fixture accept "$whitelist_changelog" "$whitelist_parsed" $'ROW\t' \
        "self-test accepts a non-Pending release entry heading"
    check_parser_fixture accept "$whitelist_changelog" "$whitelist_parsed" $'ROW\t' \
        "self-test accepts a grammar-valid release line"

    printf '# Changelog\n\nAlpha prose.\n' > "$whitelist_changelog"
    check_parser_fixture accept "$whitelist_changelog" "$whitelist_parsed" "" \
        "self-test accepts prose beginning with an ASCII letter"

    printf '# Changelog\n\nproto is ordinary prose away from an entry release position.\n' > "$whitelist_changelog"
    check_parser_fixture accept "$whitelist_changelog" "$whitelist_parsed" "" \
        "self-test accepts proto-prefixed prose outside a release position"

    printf '# Changelog\n\nno proto is ordinary prose away from an entry release position.\n' > "$whitelist_changelog"
    check_parser_fixture accept "$whitelist_changelog" "$whitelist_parsed" "" \
        "self-test accepts no-proto-prefixed prose outside a release position"

    printf '# Changelog\n\n"Quoted prose."\n' > "$whitelist_changelog"
    check_parser_fixture accept "$whitelist_changelog" "$whitelist_parsed" "" \
        "self-test accepts prose beginning with a quote"

    printf '# Changelog\n\n**Bold prose.**\n' > "$whitelist_changelog"
    check_parser_fixture accept "$whitelist_changelog" "$whitelist_parsed" "" \
        "self-test accepts prose beginning with a bold marker"

    printf '# Changelog\n\n***\n' > "$whitelist_changelog"
    check_parser_fixture reject "$whitelist_changelog" "$whitelist_parsed" \
        "asterisk-only lines are not part of the ledger: '***'" \
        "self-test rejects a compact thematic break"

    printf '# Changelog\n\n* * *\n' > "$whitelist_changelog"
    check_parser_fixture reject "$whitelist_changelog" "$whitelist_parsed" \
        "asterisk-only lines are not part of the ledger: '* * *'" \
        "self-test rejects a spaced thematic break"

    printf '# Changelog\n\n**\n' > "$whitelist_changelog"
    check_parser_fixture reject "$whitelist_changelog" "$whitelist_parsed" \
        "line does not match the closed changelog whitelist: '**'" \
        "self-test rejects an empty bold marker"

    printf '# Changelog\n\n<!-- hidden -->\n' > "$whitelist_changelog"
    check_parser_fixture reject "$whitelist_changelog" "$whitelist_parsed" \
        "raw HTML and angle brackets are not part of the ledger: '<!-- hidden -->'" \
        "self-test rejects an HTML comment line"

    printf '# Changelog\n\n<pre>\n' > "$whitelist_changelog"
    check_parser_fixture reject "$whitelist_changelog" "$whitelist_parsed" \
        "raw HTML and angle brackets are not part of the ledger: '<pre>'" \
        "self-test rejects a preformatted HTML block opener"

    printf '# Changelog\n\n<details>\n' > "$whitelist_changelog"
    check_parser_fixture reject "$whitelist_changelog" "$whitelist_parsed" \
        "raw HTML and angle brackets are not part of the ledger: '<details>'" \
        "self-test rejects a details HTML block opener"

    printf '# Changelog\n\n>### v20 — 2026-09-11\n' > "$whitelist_changelog"
    check_parser_fixture reject "$whitelist_changelog" "$whitelist_parsed" \
        "raw HTML and angle brackets are not part of the ledger: '>### v20 — 2026-09-11'" \
        "self-test rejects a compact blockquoted heading"

    printf '# Changelog\n\n1) ### v20 — 2026-09-11\n' > "$whitelist_changelog"
    check_parser_fixture reject "$whitelist_changelog" "$whitelist_parsed" \
        "closed changelog whitelist: '1) ### v20 — 2026-09-11'" \
        "self-test rejects a parenthesized ordered-list heading"

    printf '# Changelog\n\n-\t### v20 — 2026-09-11\n' > "$whitelist_changelog"
    check_parser_fixture reject "$whitelist_changelog" "$whitelist_parsed" \
        "line does not match the closed changelog whitelist" \
        "self-test rejects a bullet-and-tab heading"

    printf '# Changelog\n\n\\### v20 — 2026-09-11\n' > "$whitelist_changelog"
    check_parser_fixture reject "$whitelist_changelog" "$whitelist_parsed" \
        "line does not match the closed changelog whitelist" \
        "self-test rejects a backslash-escaped heading"

    printf '# Changelog\n\nSetext text\n===\n' > "$whitelist_changelog"
    check_parser_fixture reject "$whitelist_changelog" "$whitelist_parsed" \
        "closed changelog whitelist: '==='" \
        "self-test rejects a setext underline"

    printf '# Changelog\n\n\u200b### v20 — 2026-09-11\n' > "$whitelist_changelog"
    check_parser_fixture reject "$whitelist_changelog" "$whitelist_parsed" \
        "line does not match the closed changelog whitelist" \
        "self-test rejects a zero-width-space-prefixed heading"

    printf '# Changelog\n\n\u00a0### v20 — 2026-09-11\n' > "$whitelist_changelog"
    check_parser_fixture reject "$whitelist_changelog" "$whitelist_parsed" \
        "line does not match the closed changelog whitelist" \
        "self-test rejects a non-breaking-space-prefixed heading"

    printf '# Changelog\n\n* bullet\n' > "$whitelist_changelog"
    check_parser_fixture reject "$whitelist_changelog" "$whitelist_parsed" \
        "closed changelog whitelist: '* bullet'" \
        "self-test rejects a star bullet"

    printf '# Changelog\n\n#### Hidden heading\n' > "$whitelist_changelog"
    check_parser_fixture reject "$whitelist_changelog" "$whitelist_parsed" \
        "closed changelog whitelist: '#### Hidden heading'" \
        "self-test rejects a level-four heading"

    printf '#Changelog\n' > "$whitelist_changelog"
    check_parser_fixture reject "$whitelist_changelog" "$whitelist_parsed" \
        "closed changelog whitelist: '#Changelog'" \
        "self-test rejects a title marker without a space"

    printf '# Changelog\n\n###Hidden heading\n' > "$whitelist_changelog"
    check_parser_fixture reject "$whitelist_changelog" "$whitelist_parsed" \
        "closed changelog whitelist: '###Hidden heading'" \
        "self-test rejects a level-three marker without a space"

    printf '# Changelog\n\n## Hidden heading #\n' > "$whitelist_changelog"
    check_parser_fixture reject "$whitelist_changelog" "$whitelist_parsed" \
        "closed changelog whitelist: '## Hidden heading #'" \
        "self-test rejects a heading with a trailing hash"

    printf '# Changelog\n\n## Hidden heading ##\n' > "$whitelist_changelog"
    check_parser_fixture reject "$whitelist_changelog" "$whitelist_parsed" \
        "closed changelog whitelist: '## Hidden heading ##'" \
        "self-test rejects a heading with a trailing hash run"

    printf '# Changelog\n\n## Killshot (arena)\n\n### Topical subsection\n' > "$whitelist_changelog"
    check_parser_fixture reject "$whitelist_changelog" "$whitelist_parsed" \
        "non-Pending level-three heading must be a release entry: '### Topical subsection'" \
        "self-test rejects a topical subsection outside Pending"

    printf '# Changelog\n\nAlpha <!-- hidden --> prose.\n' > "$whitelist_changelog"
    check_parser_fixture reject "$whitelist_changelog" "$whitelist_parsed" \
        "raw HTML and angle brackets are not part of the ledger: 'Alpha <!-- hidden --> prose.'" \
        "self-test rejects a mid-line HTML comment"

    {
        printf '# Changelog\n\n## Killshot (arena)\n\n### v20 — 2026-09-11\n\n'
        printf 'no proto · stamp — · source not recorded · no tag\n\n'
        printf 'Alpha <details> release notes.\n'
    } > "$whitelist_changelog"
    parse_changelog "$whitelist_changelog" "$whitelist_parsed"
    if grep -Fq "raw HTML and angle brackets are not part of the ledger: 'Alpha <details> release notes.'" \
            "$whitelist_parsed" && grep -q '^ROW' "$whitelist_parsed"; then
        ok "self-test release path rejects an inline HTML tag in an entry's notes"
    else
        bad "self-test release path allowed an inline HTML tag in notes: $(tr '\n' ' ' < "$whitelist_parsed")"
    fi

    printf '# Changelog\n\nAlpha <details>\nZulu </details>\n' > "$whitelist_changelog"
    parse_changelog "$whitelist_changelog" "$whitelist_parsed"
    if [ "$(grep -c 'raw HTML and angle brackets are not part of the ledger' "$whitelist_parsed")" -eq 2 ]; then
        ok "self-test rejects both lines of a multi-line raw HTML tag"
    else
        bad "self-test did not reject both lines of a multi-line raw HTML tag: $(tr '\n' ' ' < "$whitelist_parsed")"
    fi

    printf '# Changelog\n\n## Killshot (arena)\n\n### v20 — 2026-09-11 <details>\n' > "$whitelist_changelog"
    check_parser_fixture reject "$whitelist_changelog" "$whitelist_parsed" \
        "raw HTML and angle brackets are not part of the ledger: '### v20 — 2026-09-11 <details>'" \
        "self-test rejects an HTML tag in an entry heading"

    printf '# Changelog\n\nAlpha > beta.\n' > "$whitelist_changelog"
    check_parser_fixture reject "$whitelist_changelog" "$whitelist_parsed" \
        "raw HTML and angle brackets are not part of the ledger: 'Alpha > beta.'" \
        "self-test rejects a bare closing angle bracket in prose"

    fenced_changelog="$tmp/fenced-changelog"
    fenced_parsed="$tmp/fenced-parsed"
    {
        printf '# Changelog\n\n```text\n'
        printf '## Killshot (arena)\n\n### v20 — 2026-09-11\n\n'
        printf 'no proto · stamp — · source `%s` · tag `arena-20.0.0`\n' "$commit"
        printf '```\n'
    } > "$fenced_changelog"
    parse_changelog "$fenced_changelog" "$fenced_parsed"
    if [ "$(grep -c 'line does not match the closed changelog whitelist' "$fenced_parsed")" -eq 2 ] \
            && grep -q '^ROW' "$fenced_parsed"; then
        ok "self-test rejects backtick fence markers while parsing the whitelisted entry normally"
    else
        bad "self-test did not reject both backtick fence markers: $(tr '\n' ' ' < "$fenced_parsed")"
    fi

    tilde_changelog="$tmp/tilde-changelog"
    {
        printf '# Changelog\n\n## Killshot (arena)\n\n~~~text\n'
        printf '### v20 — 2026-09-11\n\n'
        printf 'no proto · stamp — · source `%s` · tag `arena-20.0.0`\n' "$commit"
        printf '~~~\n'
    } > "$tilde_changelog"
    parse_changelog "$tilde_changelog" "$fenced_parsed"
    if [ "$(grep -c 'line does not match the closed changelog whitelist' "$fenced_parsed")" -eq 2 ] \
            && grep -q '^ROW' "$fenced_parsed"; then
        ok "self-test rejects tilde fence markers while parsing the whitelisted entry normally"
    else
        bad "self-test did not reject both tilde fence markers: $(tr '\n' ' ' < "$fenced_parsed")"
    fi

    nested_changelog="$tmp/nested-changelog"
    {
        printf '# Changelog\n\n## Killshot (arena)\n\n````text\n'
        printf '### v20 — 2026-09-11\n\n```\n'
        printf 'no proto · stamp — · source `%s` · tag `arena-20.0.0`\n' "$commit"
        printf '````\n'
    } > "$nested_changelog"
    parse_changelog "$nested_changelog" "$fenced_parsed"
    if [ "$(grep -c 'line does not match the closed changelog whitelist' "$fenced_parsed")" -eq 3 ] \
            && grep -Fq "release line has 1 fields, want 4 or 5: '\`\`\`'" "$fenced_parsed"; then
        ok "self-test rejects every backtick run and the positional non-release line in a nested-fence-shaped entry"
    else
        bad "self-test did not reject every nested backtick run: $(tr '\n' ' ' < "$fenced_parsed")"
    fi

    indented_changelog="$tmp/indented-changelog"
    {
        printf '# Changelog\n\n## Killshot (arena)\n\n'
        printf '    ### v20 — 2026-09-11\n\n'
        printf '    no proto · stamp — · source `%s` · tag `arena-20.0.0`\n' "$commit"
    } > "$indented_changelog"
    parse_changelog "$indented_changelog" "$fenced_parsed"
    if [ "$(grep -c 'line does not match the closed changelog whitelist' "$fenced_parsed")" -eq 2 ]; then
        ok "self-test rejects a four-space-indented entry as code"
    else
        bad "self-test accepted or silently ignored an indented entry: $(tr '\n' ' ' < "$fenced_parsed")"
    fi

    unclosed_changelog="$tmp/unclosed-changelog"
    {
        printf '# Changelog\n\n## Killshot (arena)\n\n```text\n'
        printf '### v20 — 2026-09-11\n\n'
        printf 'no proto · stamp — · source `%s` · tag `arena-20.0.0`\n' "$commit"
    } > "$unclosed_changelog"
    parse_changelog "$unclosed_changelog" "$fenced_parsed"
    if grep -q 'line does not match the closed changelog whitelist' "$fenced_parsed" \
            && grep -q '^ROW' "$fenced_parsed"; then
        ok "self-test rejects an unterminated fence marker without hiding later ledger syntax"
    else
        bad "self-test did not preserve the unterminated-fence rejection: $(tr '\n' ' ' < "$fenced_parsed")"
    fi

    list_fence_changelog="$tmp/list-fence-changelog"
    {
        printf '# Changelog\n\n## Killshot (arena)\n\n- ~~~text\n'
        printf '### v20 — 2026-09-11\n\n'
        printf 'no proto · stamp — · source `%s` · tag `arena-20.0.0`\n' "$commit"
        printf -- '- ~~~\n'
    } > "$list_fence_changelog"
    parse_changelog "$list_fence_changelog" "$fenced_parsed"
    if [ "$(grep -c 'line does not match the closed changelog whitelist' "$fenced_parsed")" -eq 2 ] \
            && grep -q '^ROW' "$fenced_parsed"; then
        ok "self-test rejects closed list-item fence markers"
    else
        bad "self-test did not reject both list-item fence markers: $(tr '\n' ' ' < "$fenced_parsed")"
    fi

    list_unclosed_changelog="$tmp/list-unclosed-changelog"
    {
        printf '# Changelog\n\n## Killshot (arena)\n\n- ~~~text\n'
        printf '### v20 — 2026-09-11\n\n'
        printf 'no proto · stamp — · source `%s` · tag `arena-20.0.0`\n' "$commit"
    } > "$list_unclosed_changelog"
    parse_changelog "$list_unclosed_changelog" "$fenced_parsed"
    if [ "$(grep -c 'line does not match the closed changelog whitelist' "$fenced_parsed")" -eq 1 ] \
            && grep -q '^ROW' "$fenced_parsed"; then
        ok "self-test rejects an unclosed list-item fence marker"
    else
        bad "self-test did not reject the unclosed list-item fence marker: $(tr '\n' ' ' < "$fenced_parsed")"
    fi

    quoted_changelog="$tmp/quoted-changelog"
    {
        printf '# Changelog\n\n## Killshot (arena)\n\n'
        printf '> ### v20 — 2026-09-11\n\n'
        printf 'no proto · stamp — · source `%s` · tag `arena-20.0.0`\n' "$commit"
    } > "$quoted_changelog"
    parse_changelog "$quoted_changelog" "$fenced_parsed"
    if grep -q 'raw HTML and angle brackets are not part of the ledger' "$fenced_parsed"; then
        ok "self-test rejects a blockquoted entry heading"
    else
        bad "self-test did not reject a blockquoted entry heading: $(tr '\n' ' ' < "$fenced_parsed")"
    fi

    invalid_opener_changelog="$tmp/invalid-opener-changelog"
    {
        printf '# Changelog\n\n## Killshot (arena)\n\n```bad`info\n'
        printf '### v20 — 2026-09-11\n\n'
        printf 'no proto · stamp — · source `%s` · tag `arena-20.0.0`\n' "$commit"
    } > "$invalid_opener_changelog"
    parse_changelog "$invalid_opener_changelog" "$fenced_parsed"
    if grep -q 'line does not match the closed changelog whitelist' "$fenced_parsed" \
            && grep -q '^ROW' "$fenced_parsed"; then
        ok "self-test rejects a backtick opener with an info string and parses the following heading"
    else
        bad "self-test let an invalid opener hide the following heading: $(tr '\n' ' ' < "$fenced_parsed")"
    fi

    pending_rows="$tmp/pending-rows"
    git -C "$tmp" switch -q -c fixture-release
    git -C "$tmp" commit --allow-empty -qm 'fixture pending source'
    pending_source="$(git -C "$tmp" rev-parse HEAD)"
    pending_release="no proto · stamp — · source \`$pending_source\` · tag \`arena-20.1.0\` (pending)"
    printf 'ROW\tFixture\tv20\t-\t-\tstamp —\tsha\t%s\t%s\tpending\t-\t-\t2026-09-11\t%s\n' \
        "$pending_source" arena-20.1.0 "$pending_release" > "$pending_rows"
    check_tag_fixture accept "$tmp" "$pending_rows" \
        "is pending; tag arena-20.1.0 not yet created" \
        "self-test accepts a pending row before its tag exists"

    pending_bad_release="no proto · stamp r999999 · source \`$pending_source\` · tag \`arena-20.1.0\` (pending)"
    pending_bad_rows="$tmp/pending-bad-rows"
    printf 'ROW\tFixture\tv20\t-\t-\tstamp r999999\tsha\t%s\t%s\tpending\t-\t-\t2026-09-11\t%s\n' \
        "$pending_source" arena-20.1.0 "$pending_bad_release" > "$pending_bad_rows"
    check_tag_fixture reject "$tmp" "$pending_bad_rows" \
        "pending row must use stamp —, got stamp r999999" \
        "self-test rejects a pending row with a non-dash stamp"

    printf '# Changelog\n\n%s\n' "$pending_release" > "$tmp/CHANGELOG.md"
    git -C "$tmp" add CHANGELOG.md
    git -C "$tmp" commit -qm 'fixture pending row'
    pending_head="$(git -C "$tmp" rev-parse HEAD)"
    git -C "$tmp" switch -q "$base_branch"
    git -C "$tmp" merge --no-ff -qm 'fixture integrating merge' fixture-release
    integrating_merge="$(git -C "$tmp" rev-parse HEAD)"
    git -C "$tmp" tag -a arena-20.1.0 -m 'fixture pending tag' "$integrating_merge"
    check_tag_fixture accept "$tmp" "$pending_rows" \
        "source $pending_source is an ancestor of tag arena-20.1.0's second parent" \
        "self-test accepts the merge commit that integrates a pending row"

    git -C "$tmp" tag -d arena-20.1.0 >/dev/null
    git -C "$tmp" tag -a arena-20.1.0 -m 'fixture linear pending tag' "$pending_head"
    check_tag_fixture reject "$tmp" "$pending_rows" \
        "does not target a two-parent merge commit" \
        "self-test rejects a single-parent descendant containing the pending row"

    git -C "$tmp" tag -d arena-20.1.0 >/dev/null
    git -C "$tmp" tag -a arena-20.1.0 -m 'fixture missing-row tag' "$commit"
    check_tag_fixture reject "$tmp" "$pending_rows" \
        "tag arena-20.1.0 does not contain its exact release line" \
        "self-test rejects a pending tag whose commit omits the row"

    git -C "$tmp" tag -d arena-20.1.0 >/dev/null
    git -C "$tmp" switch -q -c fixture-later
    git -C "$tmp" commit --allow-empty -qm 'fixture later side'
    git -C "$tmp" switch -q "$base_branch"
    git -C "$tmp" merge --no-ff -qm 'fixture later merge' fixture-later
    later_merge="$(git -C "$tmp" rev-parse HEAD)"
    git -C "$tmp" tag -a arena-20.1.0 -m 'fixture later pending tag' "$later_merge"
    check_tag_fixture reject "$tmp" "$pending_rows" \
        "first parent already contains its exact release line" \
        "self-test rejects a later merge whose first parent already contains the row"

    git -C "$tmp" tag -d arena-20.1.0 >/dev/null
    tree="$(git -C "$tmp" rev-parse "$commit^{tree}")"
    unrelated_source="$(printf 'fixture unrelated source\n' | git -C "$tmp" commit-tree "$tree")"
    unrelated_release="no proto · stamp — · source \`$unrelated_source\` · tag \`arena-20.1.0\` (pending)"
    git -C "$tmp" switch -q -c fixture-unrelated
    printf '# Changelog\n\n%s\n' "$unrelated_release" > "$tmp/CHANGELOG.md"
    git -C "$tmp" add CHANGELOG.md
    git -C "$tmp" commit -qm 'fixture unrelated pending row'
    git -C "$tmp" switch -q "$base_branch"
    git -C "$tmp" merge --no-ff -qm 'fixture unrelated-source merge' fixture-unrelated
    unrelated_merge="$(git -C "$tmp" rev-parse HEAD)"
    git -C "$tmp" tag -a arena-20.1.0 -m 'fixture unrelated-source tag' "$unrelated_merge"
    unrelated_rows="$tmp/unrelated-rows"
    printf 'ROW\tFixture\tv20\t-\t-\tstamp —\tsha\t%s\t%s\tpending\t-\t-\t2026-09-11\t%s\n' \
        "$unrelated_source" arena-20.1.0 "$unrelated_release" > "$unrelated_rows"
    check_tag_fixture reject "$tmp" "$unrelated_rows" \
        "source $unrelated_source is not an ancestor of tag arena-20.1.0's second parent" \
        "self-test rejects a pending merge whose second parent omits its source"

    git -C "$tmp" tag -d arena-20.1.0 >/dev/null
    git -C "$tmp" tag arena-20.1.0 "$integrating_merge"
    check_tag_fixture reject "$tmp" "$pending_rows" \
        "not an annotated tag object" \
        "self-test rejects a lightweight pending tag"
    git -C "$tmp" tag -d arena-20.1.0 >/dev/null

    git -C "$tmp" commit --allow-empty -qm 'fixture other commit'
    other="$(git -C "$tmp" rev-parse HEAD)"
    missing="ffffffffffffffffffffffffffffffffffffffff"
    other_missing="eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
    blob="$(printf 'fixture blob\n' | git -C "$tmp" hash-object -w --stdin)"
    collision="$("$PY" -c 'import hashlib
seen = {}
for n in range(1000000):
    data = f"ambiguous-{n}\n".encode()
    digest = hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()
    prefix = digest[:7]
    if prefix in seen:
        print(prefix, seen[prefix], n)
        break
    seen[prefix] = n
else:
    raise SystemExit(1)')"
    read -r prefix first second <<< "$collision"
    first_hash="$(printf 'ambiguous-%s\n' "$first" | git -C "$tmp" hash-object -w --stdin)"
    second_hash="$(printf 'ambiguous-%s\n' "$second" | git -C "$tmp" hash-object -w --stdin)"
    [ "$first_hash" != "$second_hash" ] \
        && [ "${first_hash:0:7}" = "$prefix" ] \
        && [ "${second_hash:0:7}" = "$prefix" ] \
        || { bad "SELF-TEST FAIL: ambiguous-object fixture did not produce two objects at $prefix"; return 1; }

    check_published_stamp_fixture accept "$tmp" offmain "$missing" "$missing" \
        "this checkout does not carry the object" \
        "self-test accepts an identical stamp for an unavailable off-main source"
    check_published_stamp_fixture reject "$tmp" offmain "$missing" "$other_missing" \
        "entry says $missing" \
        "self-test rejects a different stamp for an unavailable off-main source"
    check_published_stamp_fixture reject "$tmp" absent "$missing" "$missing" \
        "source is marked absent" \
        "self-test rejects every stamp for a source marked absent"
    check_published_stamp_fixture accept "$tmp" offmain "$commit" "${commit:0:12}" \
        "names $commit" \
        "self-test accepts an abbreviated stamp for the same present commit"
    check_published_stamp_fixture reject "$tmp" offmain "$commit" "$other" \
        "entry says $commit" \
        "self-test rejects a stamp for a different present commit"
    check_published_stamp_fixture reject "$tmp" offmain "$prefix" "$prefix" \
        "present but does not name a unique commit" \
        "self-test rejects an ambiguous present object prefix"
    check_published_stamp_fixture reject "$tmp" offmain "$blob" "$blob" \
        "present but does not name a unique commit" \
        "self-test rejects a present blob as an off-main commit"
    not_repo="$tmp/not-a-repository"
    check_published_stamp_fixture reject "$not_repo" offmain "$missing" "$missing" \
        "could not query object presence for $missing" \
        "self-test rejects the unavailable-object fallback when the repository query fails"

    rm -rf "$tmp"
    CHANGELOG_TEST_TMP=""
    trap - EXIT
    if [ "$TESTS_FAILED" -eq "$failures_before" ]; then
        echo "SELF-TEST PASS: integrating-merge pending tags, closed-whitelist ledger syntax, requested-tag identity and deterministic and published-stamp cases passed; invalid forms and histories rejected, $(( $(date +%s) - started ))s"
        return 0
    fi
    echo "SELF-TEST FAIL: one or more changelog contract cases failed, $(( $(date +%s) - started ))s" >&2
    return 1
}

if [ "${1:-}" = "--self-test" ]; then
    [ "$#" -eq 1 ] || { echo "usage: bash deploy/tests/test-changelog.sh [--self-test | --tag TAG]" >&2; exit 2; }
    self_test
    self_test_status=$?
    summary "changelog self-test"
    self_test_summary_status=$?
    if [ "$self_test_status" -ne 0 ]; then
        exit "$self_test_status"
    fi
    exit "$self_test_summary_status"
fi
REQUIRED_TAG=""
if [ "${1:-}" = "--tag" ]; then
    [ "$#" -eq 2 ] && [ -n "${2:-}" ] || { echo "usage: bash deploy/tests/test-changelog.sh [--self-test | --tag TAG]" >&2; exit 2; }
    REQUIRED_TAG="$2"
else
    [ "$#" -eq 0 ] || { echo "usage: bash deploy/tests/test-changelog.sh [--self-test | --tag TAG]" >&2; exit 2; }
fi

self_test || exit $?

CHANGELOG="$REPO/CHANGELOG.md"
GAMES="$REPO/web/games.json"

[ -f "$CHANGELOG" ] || { echo "test-changelog: no CHANGELOG.md at $CHANGELOG" >&2; exit 1; }
[ -f "$GAMES" ] || { echo "test-changelog: no web/games.json at $GAMES" >&2; exit 1; }

PARSED="$HERE/.changelog-parsed"
LAUNCHER="$HERE/.changelog-launcher"
trap 'rm -f "$PARSED" "$LAUNCHER"' EXIT

echo "== the file parses =="

# One TSV row per entry. "-" is the empty value in every column, because tab
# is IFS whitespace and a genuinely empty field would shift every column
# after it.
parse_changelog "$CHANGELOG" "$PARSED"

entries="$(grep -c '^ROW' "$PARSED" || true)"
malformed="$(grep -c '^BAD' "$PARSED" || true)"
is "$malformed" "0" "every entry's release line parses"
[ "$malformed" = "0" ] || grep '^BAD' "$PARSED" >&2
if [ "${entries:-0}" -gt 0 ]; then
    ok "$entries entries found"
else
    bad "no entries found in CHANGELOG.md"
fi
echo "== every launcher version has an entry, and every entry a launcher version =="

"$PY" - "$GAMES" > "$LAUNCHER" <<'PY'
import json, sys
d = json.load(open(sys.argv[1], encoding="utf-8"))
for g in d["games"]:
    for v in g["versions"]:
        print("%s\t%s\t%s\t%s\t%s" % (g["id"], v["v"], v.get("proto", "-"),
                                        v.get("path", "-"), v.get("version", "-")))
PY

if [ -n "$REQUIRED_TAG" ]; then
    check_requested_tag "$PARSED" "$LAUNCHER" "$REQUIRED_TAG"
fi

while IFS=$'\t' read -r gid ver lproto _ _; do
    [ -n "$gid" ] || continue
    want="$(section_for "$gid")"
    if [ -z "$want" ]; then
        bad "launcher game '$gid' has no section named in this test"
        continue
    fi
    if grep -qF "$(printf 'ROW\t%s\t%s\t' "$want" "$ver")" "$PARSED"; then
        ok "$gid $ver has an entry under '$want'"
    else
        bad "$gid $ver has no entry under '$want'"
    fi
done < "$LAUNCHER"

while IFS=$'\t' read -r _ section version _ _ _ _ _ _ _ _ _ _ _; do
    [ -n "$section" ] || continue
    found=""
    while IFS=$'\t' read -r gid ver _ _ _; do
        [ "$(section_for "$gid")" = "$section" ] && [ "$ver" = "$version" ] && found=1
    done < "$LAUNCHER"
    if [ -n "$found" ]; then
        ok "$section $version is still in the launcher"
    else
        bad "$section $version is in the changelog but not in the launcher"
    fi
done < <(grep '^ROW' "$PARSED")

echo "== the protocol matches the launcher =="

# The launcher is the protocol of record. A row may state a different one only
# by declaring the launcher's value beside it, which is how a launcher entry
# that has fallen behind its own published release stays visible instead of
# being quietly overwritten here. Such a row must also carry publication
# evidence, checked below, so an arbitrary number cannot buy itself an
# exemption.
while IFS=$'\t' read -r _ section version pnum plauncher _ _ _ _ _ pubsha pubkind _ _; do
    [ -n "$section" ] || continue
    lproto="-"
    while IFS=$'\t' read -r gid ver lp _ _; do
        [ "$(section_for "$gid")" = "$section" ] && [ "$ver" = "$version" ] && lproto="$lp"
    done < "$LAUNCHER"
    if [ "$plauncher" != "-" ]; then
        if [ "$plauncher" != "$lproto" ]; then
            bad "$section $version: entry says the launcher has proto $plauncher, launcher has $lproto"
        elif [ "$pubkind" = "-" ]; then
            bad "$section $version: claims a protocol the launcher does not have, with no publication to prove it"
        else
            ok "$section $version: proto $pnum against launcher proto $lproto, discrepancy declared"
        fi
    elif [ "$lproto" = "-" ] && [ "$pnum" = "-" ]; then
        ok "$section $version: no protocol on either side"
    elif [ "$pnum" = "$lproto" ]; then
        ok "$section $version: proto $pnum matches the launcher"
    else
        bad "$section $version: entry proto $pnum, launcher proto $lproto"
    fi
done < <(grep '^ROW' "$PARSED")

echo "== every named source commit is real =="

if git -C "$REPO" rev-parse --git-dir >/dev/null 2>&1; then
    IN_GIT=1
else
    IN_GIT=""
    ok "SKIP commit, publication and tag checks: $REPO is not a git checkout"
fi

if [ -n "$IN_GIT" ]; then
    while IFS=$'\t' read -r _ section version _ _ _ kind sha _ _ _ _ _ _; do
        [ -n "$section" ] || continue
        case "$kind" in
            sha)
                if ! git -C "$REPO" cat-file -e "$sha^{commit}" 2>/dev/null; then
                    bad "$section $version: source $sha is not a commit here"
                elif git -C "$REPO" merge-base --is-ancestor "$sha" HEAD 2>/dev/null; then
                    ok "$section $version: source $sha is an ancestor of HEAD"
                else
                    bad "$section $version: source $sha is not an ancestor of HEAD, and the entry does not say so"
                fi
                ;;
            offmain)
                # An unreachable object lives only in the store that made it:
                # a clone walks refs, so it does not travel. Absence away from
                # the original checkout is expected; presence with ancestry
                # would mean the entry is wrong.
                if ! git -C "$REPO" cat-file -e "$sha^{commit}" 2>/dev/null; then
                    ok "SKIP $section $version: source $sha is marked '(not on main)' and this checkout does not carry the object"
                elif git -C "$REPO" merge-base --is-ancestor "$sha" HEAD 2>/dev/null; then
                    bad "$section $version: source $sha is marked '(not on main)' but IS an ancestor of HEAD"
                else
                    ok "$section $version: source $sha exists and is off main, as recorded"
                fi
                ;;
            absent)
                if git -C "$REPO" cat-file -e "$sha^{commit}" 2>/dev/null; then
                    bad "$section $version: source $sha is marked 'not in repository' but resolves here"
                else
                    ok "$section $version: source $sha is not in this repository, as recorded"
                fi
                ;;
            unrecorded)
                ok "$section $version: no source commit claimed"
                ;;
        esac
    done < <(grep '^ROW' "$PARSED")
fi

echo "== the named publication is the one the rule selects =="

# Reading the right publication is the whole of the rule, and naming a
# publication is not the same as naming the right one. A review broke the
# previous suite by moving v22 back to its FIRST publication: source, stamp,
# tag, protocol and ancestry all agreed, and only the existence of a later
# byte-changing publication made the row wrong. So a named publication must
# clear three bars. It must be reachable from the retired Pages history,
# because a commit absent from that history was never served. It must appear
# in the history of that entry's served path — the launcher's own `path`, so a lab
# under labs/ is found where it actually lives — because a publication that
# left the page untouched did not cut the release. And it must be the newest
# such publication whose applicable stamp names an ancestor of HEAD, the
# applicable stamp being the release's own version.json if it has one, else
# the sha its message names, else the root ticker: exactly the precedence the
# changelog states. A row that declares its source is not in this repository
# is asserting that no on-main stamp applies to it, so the third bar is not
# applied there; the first two still are.
sel_checked=0
sel_skipped=0

PAGES="refs/remotes/origin/gh-pages"
if [ -n "$IN_GIT" ] && git -C "$REPO" rev-parse -q --verify "$PAGES" >/dev/null 2>&1; then
    HAVE_PAGES=1
else
    HAVE_PAGES=""
fi

# A publication's version.json names the commit it was built from. The hub
# publisher writes that under "commit"; End Game's own publisher writes the
# same sha under "source". Both are read wherever a stamp is resolved, so
# those releases are checkable at all; every sha comparison downstream is
# unchanged, so this verifies more, not less.
# The applicable stamp of one publication, by the changelog's precedence.
applicable_source() {
    local c="$1" path="$2" got tok
    got="$(git -C "$REPO" show "$c:$path/version.json" 2>/dev/null \
           | "$PY" -c 'import json,sys
try: stamp = json.load(sys.stdin); print(stamp.get("commit") or stamp.get("source") or "")
except Exception: print("")')"
    if [ -n "$got" ]; then echo "$got"; return; fi
    for tok in $(git -C "$REPO" log -1 --format=%s "$c" | grep -oE '[0-9a-f]{7,40}' || true); do
        if git -C "$REPO" cat-file -e "$tok^{commit}" 2>/dev/null; then echo "$tok"; return; fi
    done
    git -C "$REPO" show "$c:version.json" 2>/dev/null \
        | "$PY" -c 'import json,sys
try: stamp = json.load(sys.stdin); print(stamp.get("commit") or stamp.get("source") or "")
except Exception: print("")'
}

if [ -n "$IN_GIT" ] && [ -z "$HAVE_PAGES" ]; then
    sel_skipped="$(grep -c '^ROW' "$PARSED" || true)"
    ok "SKIP selection checks: $PAGES is not in this checkout, so no publication can be located or compared ($sel_skipped rows)"
elif [ -n "$IN_GIT" ]; then
    while IFS=$'\t' read -r _ section version _ _ _ kind sha _ _ pubsha pubkind _ _; do
        [ -n "$section" ] || continue
        [ "$pubkind" != "-" ] || continue
        path=""
        while IFS=$'\t' read -r g v _ pth _; do
            [ "$(section_for "$g")" = "$section" ] && [ "$v" = "$version" ] && path="${pth%/}"
        done < "$LAUNCHER"
        if [ -z "$path" ]; then
            bad "$section $version: the launcher gives no served path"
            continue
        fi
        if ! git -C "$REPO" cat-file -e "$pubsha^{commit}" 2>/dev/null; then
            sel_skipped=$((sel_skipped + 1))
            ok "SKIP $section $version: publication $pubsha is not in this checkout"
            continue
        fi
        sel_checked=$((sel_checked + 1))
        if git -C "$REPO" merge-base --is-ancestor "$pubsha" "$PAGES" 2>/dev/null; then
            ok "$section $version: publication $pubsha is reachable from the published branch"
        else
            bad "$section $version: publication $pubsha is not reachable from $PAGES, so it published nothing"
            continue
        fi
        cands="$(git -C "$REPO" log "$PAGES" --format=%H -- "$path")"
        case "$cands" in
            *"$pubsha"*) ;;
            *)
                full="$(git -C "$REPO" rev-parse "$pubsha")"
                case "$cands" in
                    *"$full"*) ;;
                    *)  bad "$section $version: publication $pubsha does not change $path"
                        continue ;;
                esac
                ;;
        esac
        ok "$section $version: publication $pubsha changes $path"
        if [ "$kind" != "sha" ]; then
            ok "$section $version: no on-main stamp applies, so no newer eligible publication is required"
            continue
        fi
        selected=""
        for c in $cands; do
            asrc="$(applicable_source "$c" "$path")"
            [ -n "$asrc" ] || continue
            git -C "$REPO" cat-file -e "$asrc^{commit}" 2>/dev/null || continue
            if git -C "$REPO" merge-base --is-ancestor "$asrc" HEAD 2>/dev/null; then
                selected="$c"
                selsrc="$asrc"
                break
            fi
        done
        if [ -z "$selected" ]; then
            bad "$section $version: no publication of $path has a stamp naming an ancestor of HEAD"
        elif [ "$selected" != "$(git -C "$REPO" rev-parse "$pubsha")" ]; then
            bad "$section $version: entry names publication $pubsha, but ${selected:0:8} is a newer publication of $path whose stamp names ${selsrc:0:8}"
        elif [ "$(git -C "$REPO" rev-parse "$selsrc^{commit}")" != "$(git -C "$REPO" rev-parse "$sha^{commit}")" ]; then
            bad "$section $version: the selected publication's stamp names $selsrc, entry says $sha"
        else
            ok "$section $version: $pubsha is the newest publication of $path with an on-main stamp"
        fi
    done < <(grep '^ROW' "$PARSED")
    echo "   ($sel_checked selection(s) checked, $sel_skipped skipped)"
fi

echo "== the publication on the published branch says the same thing =="

# This is the check the first version of this suite did not have. The
# published branch carries the stamp that was shipped, so the entry's source
# is not taken on trust: it is read back out of the artefact.
pub_checked=0
pub_skipped=0

if [ -n "$IN_GIT" ]; then
    while IFS=$'\t' read -r _ section version pnum plauncher _ kind sha _ _ pubsha pubkind _ _; do
        [ -n "$section" ] || continue
        if [ "$pubkind" = "-" ]; then
            continue
        fi
        if ! git -C "$REPO" cat-file -e "$pubsha^{commit}" 2>/dev/null; then
            pub_skipped=$((pub_skipped + 1))
            ok "SKIP $section $version: publication $pubsha is not in this checkout (the published branch is fetched separately)"
            continue
        fi
        pub_checked=$((pub_checked + 1))
        # The section heading is prose; the launcher carries the game id, and
        # the id is what the published layout is keyed on.
        gid=""
        while IFS=$'\t' read -r g v _ _ _; do
            [ "$(section_for "$g")" = "$section" ] && [ "$v" = "$version" ] && gid="$g"
        done < "$LAUNCHER"
        case "$pubkind" in
            root|relstamp)
                if [ "$pubkind" = root ]; then
                    path="version.json"
                else
                    # A release's own stamp lives beside its page: games/<id>/<v>/
                    # for a game, labs/<id>/ for a lab.
                    path="games/$gid/$version/version.json"
                    git -C "$REPO" cat-file -e "$pubsha:$path" 2>/dev/null \
                        || path="labs/$gid/version.json"
                fi
                if ! git -C "$REPO" cat-file -e "$pubsha:$path" 2>/dev/null; then
                    bad "$section $version: publication $pubsha carries no $path"
                    continue
                fi
                got="$(git -C "$REPO" show "$pubsha:$path" 2>/dev/null \
                       | "$PY" -c 'import json,sys
try: stamp = json.load(sys.stdin); print(stamp.get("commit") or stamp.get("source") or "")
except Exception: print("")')"
                if [ -z "$got" ]; then
                    bad "$section $version: publication $pubsha has no readable $path"
                else
                    check_published_stamp_source "$kind" "$sha" "$got" \
                        "$section $version: the $path stamp at $pubsha"
                fi
                ;;
            message)
                subj="$(git -C "$REPO" log -1 --format=%s "$pubsha")"
                case "$subj" in
                    *"$sha"*) ok "$section $version: publication $pubsha names $sha in its message" ;;
                    *)        bad "$section $version: publication $pubsha message does not name $sha: $subj" ;;
                esac
                ;;
        esac
        # A row that overrides the launcher's protocol must be able to point
        # at the catalogue that carries the protocol it claims.
        if [ "$plauncher" != "-" ]; then
            cat_proto="$(git -C "$REPO" show "$pubsha:games.json" 2>/dev/null \
                | "$PY" -c 'import json,sys
try: d=json.load(sys.stdin)
except Exception: print(""); raise SystemExit
for g in d.get("games", []):
    if g["id"] != sys.argv[1]: continue
    for v in g["versions"]:
        if v["v"] == sys.argv[2]:
            print(v.get("proto","")); raise SystemExit
print("")' "$gid" "$version")"
            if [ "$cat_proto" = "$pnum" ]; then
                ok "$section $version: the published catalogue at $pubsha carries proto $pnum"
            else
                bad "$section $version: the published catalogue at $pubsha does not carry proto $pnum"
            fi
        fi
    done < <(grep '^ROW' "$PARSED")
    echo "   ($pub_checked publication(s) checked, $pub_skipped absent from this checkout)"
fi

echo "== every tag named points where the entry says =="

# Tags travel separately from commits, so a clone or shallow checkout must
# fetch them before running this suite. Every name claimed here must exist
# exactly as written and resolve to an annotated tag object.
if [ -n "$IN_GIT" ]; then
    check_named_tags "$REPO" "$PARSED"
fi

echo "== no URL, no banned token =="

# A changelog is quoted, pasted and diffed; a link in it goes stale in a way a
# sha does not, and the repository's own address moves.
if grep -nE '(https?:)?//|www\.' "$CHANGELOG" >/dev/null; then
    bad "CHANGELOG.md contains a URL: $(grep -nE '(https?:)?//|www\.' "$CHANGELOG" | head -3 | tr '\n' ' ')"
else
    ok "no URL in CHANGELOG.md"
fi

# The five-character angle-bracket token is assembled here rather than
# written, so this test does not itself carry what it forbids.
BANNED="<$(printf 'ch'; printf 'ar')>"
if grep -nF "$BANNED" "$CHANGELOG" >/dev/null; then
    bad "CHANGELOG.md contains the banned token"
else
    ok "no banned token in CHANGELOG.md"
fi

if grep -qU $'\r' "$CHANGELOG" 2>/dev/null; then
    bad "CHANGELOG.md contains CR"
else
    ok "CHANGELOG.md is LF-only"
fi

summary changelog
