#!/usr/bin/env bash
# CHANGELOG.md against the launcher, the object store and the tags.
#
#   bash deploy/tests/test-changelog.sh
#
# The changelog is a claim about history, and a claim about history rots
# silently: a version added to web/games.json with no entry here, an entry
# naming a commit that a rebase left unreachable, a tag moved to a different
# commit than the entry records. None of that shows up when the file is read,
# and all of it shows up here.
#
# The grammar this enforces is one release line per entry, immediately after
# the entry's heading, four ` · `-separated fields:
#
#   proto <n>|no proto · stamp <r…>|stamp — · <source> · <tag>
#
#   <source>  source `<sha>`
#             source `<sha>` (not on main)
#             source not in repository (`<sha>`)
#             source not recorded
#   <tag>     tag `<name>`
#             tag `<name>` (points at `<sha>`)
#             no tag
#
# Nothing here contacts a host, a tunnel or a network.
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
# shellcheck source=deploy/tests/lib.sh
. "$HERE/lib.sh"

CHANGELOG="$REPO/CHANGELOG.md"
GAMES="$REPO/web/games.json"

[ -f "$CHANGELOG" ] || { echo "test-changelog: no CHANGELOG.md at $CHANGELOG" >&2; exit 1; }
[ -f "$GAMES" ] || { echo "test-changelog: no web/games.json at $GAMES" >&2; exit 1; }

# The section heading each game's entries live under. A game whose id is not
# listed here is a game whose section nobody named, which is itself a failure.
section_for() {
    case "$1" in
        arena)        echo "Killshot (arena)" ;;
        league)       echo "UltimateLegue (league)" ;;
        fire)         echo "Fire Racer" ;;
        kings)        echo "Four Kings" ;;
        what-is-this) echo "what is this?" ;;
        julibrot)     echo "Julibrot Lab" ;;
        *)            echo "" ;;
    esac
}

PARSED="$HERE/.changelog-parsed"
LAUNCHER="$HERE/.changelog-launcher"
trap 'rm -f "$PARSED" "$LAUNCHER"' EXIT

echo "== the file parses =="

# One TSV row per entry: section, version, proto, stamp, source kind, source
# sha, tag name, tag target, date. Kinds: sha, offmain, absent, unrecorded.
# "-" is the empty field in every column, because tab is IFS whitespace and a
# genuinely empty field would shift every column after it.
"$PY" - "$CHANGELOG" > "$PARSED" <<'PY'
import re, sys

src, = sys.argv[1:]
lines = open(src, encoding="utf-8").read().split("\n")

section = ""
rows = []
bad = []
i = 0
while i < len(lines):
    line = lines[i]
    if line.startswith("## ") and not line.startswith("### "):
        section = line[3:].strip()
        i += 1
        continue
    m = re.match(r"^### (\S+) — (.+)$", line)
    if not m:
        i += 1
        continue
    version, date = m.group(1), m.group(2).strip()
    j = i + 1
    while j < len(lines) and not lines[j].strip():
        j += 1
    release = lines[j].strip() if j < len(lines) else ""
    i = j + 1

    fields = release.split(" · ")
    if len(fields) != 4:
        bad.append("%s %s: release line has %d fields, want 4: %r"
                   % (section, version, len(fields), release))
        continue
    proto, stamp, source, tag = fields

    if not (proto == "no proto" or re.match(r"^proto \d+$", proto)):
        bad.append("%s %s: bad protocol field %r" % (section, version, proto))
        continue
    if not re.match(r"^stamp (—|r\d+(\+dirty)?)$", stamp):
        bad.append("%s %s: bad stamp field %r" % (section, version, stamp))
        continue

    kind = sha = ""
    if re.match(r"^source `[0-9a-f]{7,40}`$", source):
        kind, sha = "sha", source.split("`")[1]
    elif re.match(r"^source `[0-9a-f]{7,40}` \(not on main\)$", source):
        kind, sha = "offmain", source.split("`")[1]
    elif re.match(r"^source not in repository \(`[0-9a-f]{7,40}`\)$", source):
        kind, sha = "absent", source.split("`")[1]
    elif source == "source not recorded":
        kind = "unrecorded"
    else:
        bad.append("%s %s: bad source field %r" % (section, version, source))
        continue

    name = target = ""
    if tag == "no tag":
        pass
    elif re.match(r"^tag `[^`]+`$", tag):
        name = tag.split("`")[1]
    elif re.match(r"^tag `[^`]+` \(points at `[0-9a-f]{7,40}`\)$", tag):
        parts = tag.split("`")
        name, target = parts[1], parts[3]
    else:
        bad.append("%s %s: bad tag field %r" % (section, version, tag))
        continue

    # Tab is IFS whitespace in the shell that reads this back, so runs of
    # tabs collapse and an empty field silently shifts every field after it.
    # "-" stands for empty everywhere: no sha, no tag name, no explicit tag
    # target.
    rows.append((section, version, proto, stamp, kind, sha or "-",
                 name or "-", target or "-", date))

for line in bad:
    print("BAD\t%s" % line)
for r in rows:
    print("ROW\t" + "\t".join(r))
PY

grep -c '^ROW' "$PARSED" > /dev/null 2>&1 || true
entries="$(grep -c '^ROW' "$PARSED" || true)"
malformed="$(grep -c '^BAD' "$PARSED" || true)"
is "$malformed" "0" "every entry's release line parses"
if [ "$malformed" != "0" ]; then
    grep '^BAD' "$PARSED" >&2
fi
if [ "${entries:-0}" -gt 0 ]; then
    ok "$entries entries found"
else
    bad "no entries found in CHANGELOG.md"
fi

echo "== every launcher version has an entry =="

"$PY" - "$GAMES" > "$LAUNCHER" <<'PY'
import json, sys
d = json.load(open(sys.argv[1], encoding="utf-8"))
for g in d["games"]:
    for v in g["versions"]:
        print("%s\t%s" % (g["id"], v["v"]))
PY

while IFS=$'\t' read -r gid ver; do
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

# The reverse direction: an entry for a version the launcher dropped is a
# stale row, not a harmless one — it says the site serves something it does
# not.
while IFS=$'\t' read -r _ section version _ _ _ _ _ _; do
    [ -n "$section" ] || continue
    found=""
    while IFS=$'\t' read -r gid ver; do
        [ "$(section_for "$gid")" = "$section" ] && [ "$ver" = "$version" ] && found=1
    done < "$LAUNCHER"
    if [ -n "$found" ]; then
        ok "$section $version is still in the launcher"
    else
        bad "$section $version is in the changelog but not in the launcher"
    fi
done < <(grep '^ROW' "$PARSED")

echo "== every named source commit is real =="

if git -C "$REPO" rev-parse --git-dir >/dev/null 2>&1; then
    IN_GIT=1
else
    IN_GIT=""
    ok "SKIP commit and tag checks: $REPO is not a git checkout"
fi

if [ -n "$IN_GIT" ]; then
    while IFS=$'\t' read -r _ section version _ _ kind sha _ _; do
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
                # a clone of this repository never receives it, because clone
                # walks refs. Absence is therefore expected away from the
                # original checkout and is not a defect; presence with
                # ancestry would be, because the entry would be wrong.
                if ! git -C "$REPO" cat-file -e "$sha^{commit}" 2>/dev/null; then
                    ok "SKIP $section $version: source $sha is marked '(not on main)' and this checkout does not carry the object (a clone walks refs, so an unreachable commit does not travel)"
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

echo "== every tag named points where the entry says =="

# Tags travel separately from commits: a clone, a fetch without --tags and a
# shallow CI checkout all carry the history and none of the tags. A missing
# tag is therefore reported as a skip with its reason rather than a failure,
# and the aggregate line below says how much of the section actually ran. A
# tag that IS here and points somewhere else is a real defect and fails.
checked=0
skipped=0

if [ -n "$IN_GIT" ]; then
    while IFS=$'\t' read -r _ section version _ _ kind sha name target _; do
        [ -n "$section" ] || continue
        [ "$name" != "-" ] || continue
        want="$target"
        [ "$want" != "-" ] || want="$sha"
        if ! git -C "$REPO" rev-parse -q --verify "refs/tags/$name" >/dev/null 2>&1; then
            skipped=$((skipped + 1))
            ok "SKIP $section $version: tag $name is not in this checkout (tags are fetched separately from commits)"
            continue
        fi
        checked=$((checked + 1))
        at="$(git -C "$REPO" rev-parse "refs/tags/$name^{commit}")"
        expect="$(git -C "$REPO" rev-parse "$want^{commit}" 2>/dev/null || echo "?")"
        if [ "$at" = "$expect" ]; then
            ok "$section $version: tag $name points at $want"
        else
            bad "$section $version: tag $name points at $at, entry says $want ($expect)"
        fi
    done < <(grep '^ROW' "$PARSED")
    echo "   ($checked tag(s) checked, $skipped absent from this checkout)"
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
