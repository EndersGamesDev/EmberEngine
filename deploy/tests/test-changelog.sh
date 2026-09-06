#!/usr/bin/env bash
# CHANGELOG.md against the launcher, the object store, the published branch
# and the tags.
#
#   bash deploy/tests/test-changelog.sh
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
#   <tag>       tag `<name>`
#               tag `<name>` (points at `<sha>`)
#               no tag
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

CHANGELOG="$REPO/CHANGELOG.md"
GAMES="$REPO/web/games.json"

[ -f "$CHANGELOG" ] || { echo "test-changelog: no CHANGELOG.md at $CHANGELOG" >&2; exit 1; }
[ -f "$GAMES" ] || { echo "test-changelog: no web/games.json at $GAMES" >&2; exit 1; }

# The section heading each game's entries live under, and the published path
# its page tree occupies. A game listed in the launcher and missing here is a
# game whose section nobody named, which is itself a failure.
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

# Tags that predate this ledger. They are lightweight commit refs and are
# deliberately never rewritten, so the annotated-object requirement below
# cannot apply to them.
PREEXISTING_TAGS="v20 v22"

PARSED="$HERE/.changelog-parsed"
LAUNCHER="$HERE/.changelog-launcher"
trap 'rm -f "$PARSED" "$LAUNCHER"' EXIT

echo "== the file parses =="

# One TSV row per entry. "-" is the empty value in every column, because tab
# is IFS whitespace and a genuinely empty field would shift every column
# after it.
"$PY" - "$CHANGELOG" > "$PARSED" <<'PY'
import re, sys

src, = sys.argv[1:]
lines = open(src, encoding="utf-8").read().split("\n")

section = ""
rows, bad = [], []
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
    if len(fields) not in (4, 5):
        bad.append("%s %s: release line has %d fields, want 4 or 5: %r"
                   % (section, version, len(fields), release))
        continue
    proto, stamp, source, tag = fields[:4]
    published = fields[4] if len(fields) == 5 else ""

    pnum = plauncher = "-"
    if proto == "no proto":
        pass
    elif re.match(r"^proto \d+$", proto):
        pnum = proto.split()[1]
    elif re.match(r"^proto \d+ \(launcher says proto \d+\)$", proto):
        parts = proto.replace("(", "").replace(")", "").split()
        pnum, plauncher = parts[1], parts[5]
    else:
        bad.append("%s %s: bad protocol field %r" % (section, version, proto))
        continue

    if not re.match(r"^stamp (—|r\d+(\+dirty)?)$", stamp):
        bad.append("%s %s: bad stamp field %r" % (section, version, stamp))
        continue

    kind = sha = "-"
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

    name = target = "-"
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

    pubsha, pubkind = "-", "-"
    if published:
        m2 = re.match(r"^published `([0-9a-f]{7,40})`"
                      r"(?: \((message|release stamp)\))?$", published)
        if not m2:
            bad.append("%s %s: bad published field %r"
                       % (section, version, published))
            continue
        pubsha = m2.group(1)
        pubkind = {None: "root", "message": "message",
                   "release stamp": "relstamp"}[m2.group(2)]

    rows.append((section, version, pnum, plauncher, stamp, kind, sha,
                 name, target, pubsha, pubkind, date))

for line in bad:
    print("BAD\t%s" % line)
for r in rows:
    print("ROW\t" + "\t".join(r))
PY

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
        print("%s\t%s\t%s" % (g["id"], v["v"], v.get("proto", "-")))
PY

while IFS=$'\t' read -r gid ver lproto; do
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

while IFS=$'\t' read -r _ section version _ _ _ _ _ _ _ _ _; do
    [ -n "$section" ] || continue
    found=""
    while IFS=$'\t' read -r gid ver _; do
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
while IFS=$'\t' read -r _ section version pnum plauncher _ _ _ _ _ pubsha pubkind _; do
    [ -n "$section" ] || continue
    lproto="-"
    while IFS=$'\t' read -r gid ver lp; do
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
    while IFS=$'\t' read -r _ section version _ _ _ kind sha _ _ _ _ _; do
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

echo "== the publication on the published branch says the same thing =="

# This is the check the first version of this suite did not have. The
# published branch carries the stamp that was shipped, so the entry's source
# is not taken on trust: it is read back out of the artefact.
pub_checked=0
pub_skipped=0

if [ -n "$IN_GIT" ]; then
    while IFS=$'\t' read -r _ section version pnum plauncher _ kind sha _ _ pubsha pubkind _; do
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
        while IFS=$'\t' read -r g v _; do
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
try: print(json.load(sys.stdin)["commit"])
except Exception: print("")')"
                if [ -z "$got" ]; then
                    bad "$section $version: publication $pubsha has no readable $path"
                elif [ "$kind" = "absent" ]; then
                    bad "$section $version: source is marked absent but $path names $got"
                else
                    a="$(git -C "$REPO" rev-parse "$got^{commit}" 2>/dev/null || echo A)"
                    b="$(git -C "$REPO" rev-parse "$sha^{commit}" 2>/dev/null || echo B)"
                    if [ "$a" = "$b" ]; then
                        ok "$section $version: $path at $pubsha names $sha"
                    else
                        bad "$section $version: $path at $pubsha names $got, entry says $sha"
                    fi
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

# Tags travel separately from commits: a clone, a fetch without --tags and a
# shallow CI checkout all carry the history and none of the tags. A missing
# tag is a skip with its reason; a tag that IS here and points somewhere else
# is a real defect.
checked=0
skipped=0

if [ -n "$IN_GIT" ]; then
    while IFS=$'\t' read -r _ section version _ _ _ _ sha name target _ _ _; do
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
        # A release tag carries a message and a signature, so it must be a tag
        # object. The two pre-existing lightweight tags are the stated
        # exception: they are never rewritten.
        objtype="$(git -C "$REPO" cat-file -t "refs/tags/$name" 2>/dev/null)"
        case " $PREEXISTING_TAGS " in
            *" $name "*)
                ok "$section $version: tag $name is pre-existing, its object type is left as it was ($objtype)"
                ;;
            *)
                if [ "$objtype" = "tag" ]; then
                    ok "$section $version: tag $name is an annotated tag object"
                else
                    bad "$section $version: tag $name is a $objtype, not an annotated tag object"
                fi
                ;;
        esac
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
