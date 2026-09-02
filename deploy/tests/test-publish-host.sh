#!/usr/bin/env bash
# publish-host.sh against a scratch book: upsert, merge, the legacy recompute,
# remove, mirror format, and a real push to a local bare repository.
#
#   bash deploy/tests/test-publish-host.sh
#
# Nothing here touches a network or a real host: every "repository" is a
# `git init --bare` under a temp dir.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
DEPLOY="$(cd "$HERE/.." && pwd)"
# shellcheck source=deploy/tests/lib.sh
. "$HERE/lib.sh"

PUB="bash $DEPLOY/publish-host.sh"
TMP="$(mktemp -d -t ember-pubtest-XXXXXX)"
trap 'rm -rf "$TMP"' EXIT
BOOK="$TMP/server.json"

# Set the top-level protocol keys the way deploy-pages.sh does. Without them
# there is no protocol for a host to match, and the legacy address keys are
# deliberately left alone (see the "no top-level protocol" case below).
set_top_in() {
    "$PY" - "$1" "$2" "$3" <<'EOF'
import json, sys
p, key, val = sys.argv[1], sys.argv[2], sys.argv[3]
d = json.load(open(p, encoding="utf-8"))
d[key] = int(val)
json.dump(d, open(p, "w", encoding="utf-8"), indent=2)
EOF
}
set_top() { set_top_in "$BOOK" "$1" "$2"; }

echo "== upsert into an empty book =="
$PUB --book "$BOOK" --name amber-otter \
    --game arena --url wss://a.example --proto 12 \
    --version r211 --commit aaa1111 --by "tester@here" >/dev/null
is "$(jget "$BOOK" 'len(d["hosts"])')" "1" "one host entry"
is "$(jget "$BOOK" 'd["hosts"][0]["name"]')" "amber-otter" "entry name"
is "$(jget "$BOOK" 'd["hosts"][0]["ws"]')" "wss://a.example" "arena address key is the bare ws"
is "$(jget "$BOOK" 'd["hosts"][0]["proto"]')" "12" "arena protocol key is the bare proto"
is "$(jget "$BOOK" 'd["hosts"][0]["version"]')" "r211" "version"
is "$(jget "$BOOK" 'd["hosts"][0]["commit"]')" "aaa1111" "commit"
is "$(jget "$BOOK" 'd["hosts"][0]["by"]')" "tester@here" "by"
is "$(jget "$BOOK" 'bool(d["hosts"][0]["updated"].endswith("Z"))')" "True" "updated is UTC ISO-8601"
is "$(jget "$BOOK" 'bool(d["v"].isdigit())')" "True" "v is a unix stamp"
V1="$(jget "$BOOK" 'd["v"]')"

echo "== no top-level protocol: the legacy key is left alone =="
# The contract ties each legacy address key to the hosts whose protocol key
# EQUALS the top-level one. Before deploy-pages.sh has ever stamped a protocol
# there is no equality to test, so nothing is written — a page reading `ws`
# would otherwise be sent to a host on an unknown protocol.
is "$(jget "$BOOK" '"ws" in d')" "False" "no legacy ws without a top-level proto"

echo "== a second game merges onto the same entry =="
$PUB --book "$BOOK" --name amber-otter \
    --game fire --url wss://a-fire.example --proto 1 >/dev/null
is "$(jget "$BOOK" 'len(d["hosts"])')" "1" "still one entry"
is "$(jget "$BOOK" 'd["hosts"][0]["ws"]')" "wss://a.example" "arena address survived a fire-only publish"
is "$(jget "$BOOK" 'd["hosts"][0]["proto"]')" "12" "arena protocol survived"
is "$(jget "$BOOK" 'd["hosts"][0]["fire_ws"]')" "wss://a-fire.example" "fire address key is <id>_ws"
is "$(jget "$BOOK" 'd["hosts"][0]["fire_proto"]')" "1" "fire protocol key is <id>_proto"
is "$(jget "$BOOK" 'd["hosts"][0]["version"]')" "r211" "version untouched when not passed"

echo "== the build stamp is per game =="
# One pair for the whole entry meant a fire-only deploy claimed the host was
# also running the newest ARENA build, and the legacy recompute — which ranks
# by that number with no probe — believed it.
$PUB --book "$BOOK" --name amber-otter \
    --game fire --url wss://a-fire.example --proto 1 \
    --version r250 --commit fff9999 >/dev/null
is "$(jget "$BOOK" 'd["hosts"][0]["fire_version"]')" "r250" "fire's build lands on fire's own key"
is "$(jget "$BOOK" 'd["hosts"][0]["fire_commit"]')" "fff9999" "and so does its commit"
is "$(jget "$BOOK" 'd["hosts"][0]["version"]')" "r211" "the arena's bare version is untouched by a fire publish"
is "$(jget "$BOOK" 'd["hosts"][0]["commit"]')" "aaa1111" "and so is its commit"

echo "== a game nobody hard-coded =="
$PUB --book "$BOOK" --name amber-otter \
    --game kings --url wss://a-kings.example --proto 3 >/dev/null
is "$(jget "$BOOK" 'd["hosts"][0]["kings_ws"]')" "wss://a-kings.example" "a new game id needs no code"
set_top kings_proto 3
$PUB --book "$BOOK" --recompute >/dev/null
is "$(jget "$BOOK" 'd["kings_ws"]')" "wss://a-kings.example" "and its legacy key is maintained too"

echo "== the legacy key prefers the newest matching-protocol host =="
set_top proto 12
$PUB --book "$BOOK" --recompute >/dev/null
is "$(jget "$BOOK" 'd["ws"]')" "wss://a.example" "the only host on the live protocol wins"
$PUB --book "$BOOK" --name flint-heron \
    --game arena --url wss://b.example --proto 12 --version r300 >/dev/null
is "$(jget "$BOOK" 'd["ws"]')" "wss://b.example" "newest version on the live protocol wins"
$PUB --book "$BOOK" --name dusky-lynx \
    --game arena --url wss://c.example --proto 11 --version r999 >/dev/null
is "$(jget "$BOOK" 'd["ws"]')" "wss://b.example" "a newer host on ANOTHER protocol does not win"
is "$(jget "$BOOK" 'len(d["hosts"])')" "3" "but it is still listed"

echo "== equal versions resolve deterministically =="
$PUB --book "$BOOK" --name quiet-raven \
    --game arena --url wss://d.example --proto 12 --version r300 >/dev/null
WS="$(jget "$BOOK" 'd["ws"]')"
case "$WS" in
    wss://b.example|wss://d.example) ok "tie broken by updated then name ($WS)" ;;
    *) bad "tie resolved to an unrelated host: $WS" ;;
esac
WS_KEPT="$WS"

echo "== a one-game deploy does not steal another game's legacy key =="
# The defect this pins: host-a runs an r300 arena, host-b a newer r305 one, so
# `ws` is host-b's. Deploying only FIRE to host-a from a newer ref used to
# stamp the whole entry r310, which made host-a rank as the newest ARENA build
# and pointed every frozen page at the older of the two arenas.
STAMPS="$TMP/stamps.json"
$PUB --book "$STAMPS" --name host-a \
    --game arena --url wss://a2.example --proto 12 --version r300 --commit a300000 >/dev/null
$PUB --book "$STAMPS" --name host-b \
    --game arena --url wss://b2.example --proto 12 --version r305 --commit b305000 >/dev/null
set_top_in "$STAMPS" proto 12
$PUB --book "$STAMPS" --recompute >/dev/null
is "$(jget "$STAMPS" 'd["ws"]')" "wss://b2.example" "the newer arena owns the legacy key"
$PUB --book "$STAMPS" --name host-a \
    --game fire --url wss://a2-fire.example --proto 1 --version r310 --commit a310000 >/dev/null
is "$(jget "$STAMPS" 'd["ws"]')" "wss://b2.example" \
    "and a fire deploy on the other host does not take it away"
is "$(jget "$STAMPS" '[h for h in d["hosts"] if h["name"]=="host-a"][0]["version"]')" "r300" \
    "host-a still says which arena build it is running"
is "$(jget "$STAMPS" '[h for h in d["hosts"] if h["name"]=="host-a"][0]["fire_version"]')" "r310" \
    "and separately which fire build"

echo "== an entry from before the per-game stamp still ranks =="
# Every entry already on gh-pages carries only the bare `version`. Reading such
# an entry as build 0 would flip each legacy key to whichever new-format host
# published last — the same defect with the sign reversed.
OLDFMT="$TMP/oldformat.json"
"$PY" - "$OLDFMT" <<'EOF'
import json, sys
json.dump({
    "fire_proto": 1,
    "hosts": [
        {"name": "old-writer", "fire_ws": "wss://old.example", "fire_proto": 1, "version": "r400"},
        {"name": "new-writer", "fire_ws": "wss://new.example", "fire_proto": 1,
         "version": "r100", "fire_version": "r350"},
    ],
}, open(sys.argv[1], "w", encoding="utf-8"), indent=2)
EOF
$PUB --book "$OLDFMT" --recompute >/dev/null
is "$(jget "$OLDFMT" 'd["fire_ws"]')" "wss://old.example" \
    "the bare version is the fallback when a game has no key of its own"

echo "== --drop-game retires one game and leaves the rest of the entry =="
# A merge can never take a key away, so a game shut down for good kept its
# address in the entry and kept winning the legacy recompute every time the
# host redeployed the OTHER game — a dead address pinned on a live machine,
# with `--remove` (which drops the still-running games too) as the only cure.
DROPB="$TMP/drop.json"
$PUB --book "$DROPB" --name amber-otter \
    --game arena --url wss://da.example --proto 12 \
    --game fire --url wss://df.example --proto 1 \
    --version r500 --commit ddd0000 >/dev/null
$PUB --book "$DROPB" --name flint-heron \
    --game fire --url wss://df2.example --proto 1 --version r400 >/dev/null
set_top_in "$DROPB" proto 12
set_top_in "$DROPB" fire_proto 1
$PUB --book "$DROPB" --recompute >/dev/null
is "$(jget "$DROPB" 'd["fire_ws"]')" "wss://df.example" "the newest fire host owns the legacy key"
DROPPED="$($PUB --book "$DROPB" --name amber-otter --drop-game fire)"
contains "$DROPPED" "dropped fire from amber-otter" "the retirement says what it did"
AO='[h for h in d["hosts"] if h["name"]=="amber-otter"][0]'
is "$(jget "$DROPB" "'fire_ws' in $AO")" "False" "fire's address is gone"
is "$(jget "$DROPB" "'fire_proto' in $AO")" "False" "and its protocol"
is "$(jget "$DROPB" "'fire_version' in $AO")" "False" "and its build stamp"
is "$(jget "$DROPB" "$AO[\"ws\"]")" "wss://da.example" "the arena still running there is untouched"
is "$(jget "$DROPB" "$AO[\"version\"]")" "r500" "and so is its stamp"
is "$(jget "$DROPB" 'd["fire_ws"]')" "wss://df2.example" \
    "and the legacy fire address moved to the host that still runs it"

echo "== --drop-game refuses the calls that would mean two things at once =="
if $PUB --book "$DROPB" --name amber-otter --drop-game arena \
        --game arena --url wss://x.example --proto 12 >/dev/null 2>&1; then
    bad "a call both published and retired the same game"
else
    ok "publishing and retiring the same game is refused"
fi
if $PUB --book "$DROPB" --name amber-otter --drop-game arena --remove >/dev/null 2>&1; then
    bad "--remove and --drop-game were accepted together"
else
    ok "--remove and --drop-game are alternatives"
fi
if $PUB --book "$DROPB" --recompute --drop-game arena >/dev/null 2>&1; then
    bad "--recompute accepted a --drop-game it cannot apply"
else
    ok "--recompute refuses --drop-game"
fi
GONE="$($PUB --book "$DROPB" --name never-published --drop-game fire)"
contains "$GONE" "no entry named never-published" "retiring a game on an unlisted host adds nothing"
is "$(jget "$DROPB" '[h["name"] for h in d["hosts"]].count("never-published")')" "0" "and really nothing"

echo "== no host matches: the key is left exactly as it was =="
set_top proto 99
$PUB --book "$BOOK" --recompute >/dev/null
is "$(jget "$BOOK" 'd["ws"]')" "$WS_KEPT" "legacy ws untouched when nothing matches"
is "$(jget "$BOOK" 'd["proto"]')" "99" "recompute never touches a protocol key"
is "$(jget "$BOOK" 'd["fire_proto"] if "fire_proto" in d else "absent"')" "absent" "and never invents one"

echo "== recompute after a protocol bump re-points the legacy key =="
set_top proto 11
$PUB --book "$BOOK" --recompute >/dev/null
is "$(jget "$BOOK" 'd["ws"]')" "wss://c.example" "ws now names the host that speaks v11"

echo "== remove =="
$PUB --book "$BOOK" --name dusky-lynx --remove >/dev/null
is "$(jget "$BOOK" 'len(d["hosts"])')" "3" "entry deleted"
is "$(jget "$BOOK" '[h["name"] for h in d["hosts"]].count("dusky-lynx")')" "0" "by name"
is "$(jget "$BOOK" 'd["ws"]')" "wss://c.example" "its stale legacy address is left rather than nulled"
OUT="$($PUB --book "$BOOK" --name never-published --remove)"
contains "$OUT" "UNCHANGED" "removing an absent host changes nothing"

echo "== v moves when the book moves, and only then =="
V_BEFORE="$(jget "$BOOK" 'd["v"]')"
$PUB --book "$BOOK" --recompute >/dev/null
is "$(jget "$BOOK" 'd["v"]')" "$V_BEFORE" "a no-op recompute leaves v alone"
sleep 1
$PUB --book "$BOOK" --name amber-otter --game arena --url wss://a2.example --proto 12 >/dev/null
V_AFTER="$(jget "$BOOK" 'd["v"]')"
if [ "$V_AFTER" != "$V_BEFORE" ]; then ok "v bumped on a real change"; else bad "v did not move on a change"; fi
if [ "$V1" != "$V_AFTER" ]; then ok "v is a fresh stamp"; else bad "v never changed at all"; fi

echo "== mirrors and unknown keys are left alone =="
"$PY" - "$BOOK" <<'EOF'
import json, sys
p = sys.argv[1]
d = json.load(open(p, encoding="utf-8"))
d["mirrors"] = ["https://someone.example/host.json"]
d["something_else"] = "keep me"
json.dump(d, open(p, "w", encoding="utf-8"), indent=2)
EOF
$PUB --book "$BOOK" --name amber-otter --game arena --url wss://a3.example --proto 12 >/dev/null
is "$(jget "$BOOK" 'd["mirrors"][0]')" "https://someone.example/host.json" "mirrors survive a publish"
is "$(jget "$BOOK" 'd["something_else"]')" "keep me" "unknown top-level keys survive"

echo "== a malformed book is refused, not reset =="
echo 'not json at all' > "$TMP/broken.json"
if $PUB --book "$TMP/broken.json" --name amber-otter --game arena --url wss://x --proto 1 >/dev/null 2>&1; then
    bad "a malformed book was overwritten"
else
    ok "a malformed book is refused"
fi
is "$(cat "$TMP/broken.json")" "not json at all" "and left byte-for-byte alone"
is "$(ls "$TMP"/*.tmp 2>/dev/null | wc -l | tr -d ' ')" "0" "and no half-written temp file is left behind"

echo "== deploy-pages.sh is the book's OTHER writer, and refuses one too =="
# It writes the top-level protocol keys into the same file. It used to catch a
# parse error, start from `{}`, and push the result — one bad byte on gh-pages
# became the silent loss of every host entry and every mirror. The block is
# lifted out of the script as it ships rather than copied here, so the test
# cannot drift from what actually runs.
STAMP="$TMP/pages-stamp.py"
awk '/^python - /{f=1;next} f && /^EOF$/{exit} f' "$DEPLOY/deploy-pages.sh" > "$STAMP"
if [ -s "$STAMP" ] && grep -q 'fire_proto' "$STAMP"; then
    ok "the deploy-pages.sh book writer was extracted"
else
    bad "could not extract the book writer from deploy-pages.sh"
fi
echo 'not json at all' > "$TMP/pages-broken.json"
if "$PY" "$STAMP" "$TMP/pages-broken.json" 12 1 >/dev/null 2>"$TMP/pages-err"; then
    bad "deploy-pages.sh overwrote a malformed book"
else
    ok "deploy-pages.sh refuses a malformed book"
fi
contains "$(cat "$TMP/pages-err")" "refusing to overwrite" "and says so"
is "$(cat "$TMP/pages-broken.json")" "not json at all" "leaving it byte-for-byte alone"
echo '[1, 2, 3]' > "$TMP/pages-list.json"
if "$PY" "$STAMP" "$TMP/pages-list.json" 12 1 >/dev/null 2>&1; then
    bad "deploy-pages.sh overwrote a book that is not an object"
else
    ok "deploy-pages.sh refuses a book that is not an object"
fi
# The book it must NOT refuse: a real one, whose entries and mirrors survive.
"$PY" - "$TMP/pages-good.json" <<'EOF'
import json, sys
json.dump({"hosts": [{"name": "amber-otter", "ws": "wss://a.example", "proto": 12}],
           "mirrors": [{"url": "https://m.example/host.json", "name": "flint-heron"}]},
          open(sys.argv[1], "w", encoding="utf-8"), indent=2)
EOF
"$PY" "$STAMP" "$TMP/pages-good.json" 12 1 >/dev/null
is "$(jget "$TMP/pages-good.json" 'len(d["hosts"])')" "1" "a good book keeps its hosts"
is "$(jget "$TMP/pages-good.json" 'd["mirrors"][0]["name"]')" "flint-heron" "and its mirrors"
is "$(jget "$TMP/pages-good.json" 'd["fire_proto"]')" "1" "and gains the protocol keys"
is "$(ls "$TMP"/*.tmp 2>/dev/null | wc -l | tr -d ' ')" "0" "written through a temp file that is renamed away"
: > "$TMP/pages-empty.json"
"$PY" "$STAMP" "$TMP/pages-empty.json" 12 1 >/dev/null
is "$(jget "$TMP/pages-empty.json" 'd["proto"]')" "12" "an empty file is still a legitimate fresh start"

echo "== host.json is a single entry, no book scaffolding =="
MIRROR="$TMP/host.json"
$PUB --book "$MIRROR" --file host.json --name lunar-ibex \
    --game arena --url wss://m.example --proto 12 \
    --game fire --url wss://m-fire.example --proto 1 \
    --version r7 --commit bbb2222 >/dev/null
is "$(jget "$MIRROR" 'd["name"]')" "lunar-ibex" "mirror entry name"
is "$(jget "$MIRROR" 'd["ws"]')" "wss://m.example" "mirror arena address"
is "$(jget "$MIRROR" 'd["fire_ws"]')" "wss://m-fire.example" "mirror fire address"
is "$(jget "$MIRROR" 'd["fire_proto"]')" "1" "mirror fire protocol"
is "$(jget "$MIRROR" '"hosts" in d')" "False" "no hosts list in a mirror file"
is "$(jget "$MIRROR" '"v" in d')" "False" "no cache-buster in a mirror file"
if $PUB --book "$MIRROR" --file host.json --name misty-egret \
    --game arena --url wss://z.example --proto 12 >/dev/null 2>&1; then
    bad "a second host was merged into someone else's mirror file"
else
    ok "a mirror file refuses a second host"
fi

echo "== bad input is refused =="
if $PUB --book "$TMP/never.json" --name "Not A Name" --game arena --url wss://x --proto 1 >/dev/null 2>&1; then
    bad "an out-of-shape --name was accepted"
else
    ok "an out-of-shape --name is refused"
fi
if $PUB --book "$TMP/never.json" --name good-name --game arena --url wss://x >/dev/null 2>&1; then
    bad "a --game without --proto was accepted"
else
    ok "a --game without --proto is refused"
fi
if $PUB --book "$TMP/never.json" --name good-name --game arena --url http://x --proto 1 >/dev/null 2>&1; then
    bad "a non-websocket --url was accepted"
else
    ok "a non-websocket --url is refused"
fi
if [ -e "$TMP/never.json" ]; then bad "a refused call still created the book"; else ok "a refused call creates nothing"; fi

echo "== no interpreter: it says so, rather than exiting silently =="
# The guard used to be dead code. `PY="$(command -v python3 || command -v
# python)"` under `set -e` is itself a failing command when neither exists, so
# the script exited 1 with no output and the operator saw a deploy stop
# mid-sentence. Run it with an empty PATH — `bash` is invoked by absolute path
# so the shell still starts, and `command -v` is a builtin.
EMPTY="$TMP/no-path"
mkdir -p "$EMPTY"
NOPY="$(PATH="$EMPTY" "$BASH" "$DEPLOY/publish-host.sh" --book "$TMP/never.json" \
    --name good-name --game arena --url wss://x --proto 1 2>&1 || true)"
contains "$NOPY" "need a working python3" "a host with no python is told why"

echo "== push to a real repository =="
BARE="$TMP/pages.git"
git init -q --bare "$BARE"
$PUB --repo "$BARE" --branch gh-pages --name misty-egret \
    --game arena --url wss://p.example --proto 12 --version r42 --commit ccc3333 >/dev/null
CHECK="$TMP/check"
git clone -q --branch gh-pages "$BARE" "$CHECK"
is "$(jget "$CHECK/server.json" 'd["hosts"][0]["name"]')" "misty-egret" "pushed entry landed on the branch"
is "$(git -C "$CHECK" log -1 --pretty=%s)" "Publish host misty-egret" "commit subject"

echo "== a second push merges rather than replaces =="
$PUB --repo "$BARE" --branch gh-pages --name misty-egret \
    --game fire --url wss://p-fire.example --proto 1 >/dev/null
$PUB --repo "$BARE" --branch gh-pages --name olive-gecko \
    --game arena --url wss://q.example --proto 12 --version r43 >/dev/null
rm -rf "$CHECK"
git clone -q --branch gh-pages "$BARE" "$CHECK"
is "$(jget "$CHECK/server.json" 'len(d["hosts"])')" "2" "two hosts on the branch"
is "$(jget "$CHECK/server.json" 'd["hosts"][0]["fire_ws"]')" "wss://p-fire.example" "the first host kept and gained keys"
is "$(jget "$CHECK/server.json" 'd["hosts"][0]["ws"]')" "wss://p.example" "and kept its arena address"

echo "== the fetch is by branch name, not the repository default =="
# A shallow `git clone` takes the remote's DEFAULT branch, so a book branch
# that is not the default would look missing and be restarted from nothing.
# Give this bare repo a default branch that is not gh-pages and publish again.
git -C "$BARE" symbolic-ref HEAD refs/heads/main
git -C "$CHECK" checkout -q -b main
git -C "$CHECK" push -q origin main
$PUB --repo "$BARE" --branch gh-pages --name rapid-tapir \
    --game arena --url wss://s.example --proto 12 --version r44 >/dev/null
rm -rf "$CHECK"
git clone -q --branch gh-pages "$BARE" "$CHECK"
is "$(jget "$CHECK/server.json" 'len(d["hosts"])')" "3" "the existing book was extended, not replaced"

echo "== a mirror push starts a branch that did not exist =="
MBARE="$TMP/mirror.git"
git init -q --bare "$MBARE"
$PUB --repo "$MBARE" --branch gh-pages --file host.json --name rapid-tapir \
    --game fire --url wss://r.example --proto 1 --version r9 >/dev/null
MCHECK="$TMP/mcheck"
git clone -q --branch gh-pages "$MBARE" "$MCHECK"
is "$(jget "$MCHECK/host.json" 'd["name"]')" "rapid-tapir" "mirror file pushed"
is "$(jget "$MCHECK/host.json" 'd["fire_ws"]')" "wss://r.example" "with its address"
is "$(ls "$MCHECK" | tr '\n' ' ')" "host.json " "and nothing else"

summary publish-host
