#!/usr/bin/env bash
# republish-host.sh against fake ssh and a local bare Pages repository.
#
#   bash deploy/tests/test-republish-host.sh
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
DEPLOY="$(cd "$HERE/.." && pwd)"
# shellcheck source=deploy/tests/lib.sh
. "$HERE/lib.sh"

TMP="$(mktemp -d -t ember-republishtest-XXXXXX)"
trap 'rm -rf "$TMP"' EXIT
PAGES="$TMP/pages.git"
REMOTE_ENTRY="$TMP/host.json"
PUB="bash $DEPLOY/publish-host.sh"
git init -q --bare "$PAGES"

# Seed an unrelated host so the merge must preserve an existing book.
$PUB --repo "$PAGES" --branch gh-pages --name amber-otter \
    --game arena --url wss://arena.example --proto 15 \
    --version r581 --commit aaa1111 >/dev/null
$PUB --book "$REMOTE_ENTRY" --file host.json --name quiet-egret \
    --game arena --url wss://new-arena.example --proto 15 \
    --game fire --url wss://new-fire.example --proto 2 \
    --game kings --url wss://new-kings.example --proto 1 \
    --version r582 --commit bbb2222 --by ember@sokol >/dev/null

mkdir -p "$TMP/bin"
cp "$HERE/shims/ssh" "$TMP/bin/ssh"
chmod +x "$TMP/bin/ssh"
export PATH="$TMP/bin:$PATH"
export SHIM_LOG="$TMP/ssh.log"
export REPUBLISH_HOST_JSON="$REMOTE_ENTRY"

echo "== first republish merges the fetched entry =="
bash "$DEPLOY/republish-host.sh" sokol --repo "$PAGES" --branch gh-pages > "$TMP/first.log"
CHECK="$TMP/check"
git clone -q --branch gh-pages "$PAGES" "$CHECK"
is "$(jget "$CHECK/server.json" 'len(d["hosts"])')" "2" "the existing host survived the merge"
is "$(jget "$CHECK/server.json" '[h for h in d["hosts"] if h["name"] == "quiet-egret"][0]["kings_ws"]')" "wss://new-kings.example" "the fetched Kings address landed"
FIRST="$(git -C "$CHECK" rev-parse HEAD)"

echo "== an unchanged fetch does not push =="
bash "$DEPLOY/republish-host.sh" sokol --repo "$PAGES" --branch gh-pages > "$TMP/second.log"
SECOND="$(git --git-dir="$PAGES" rev-parse refs/heads/gh-pages)"
is "$SECOND" "$FIRST" "the branch did not move"
contains "$(cat "$TMP/second.log")" "unchanged; nothing to push" "the no-op is explicit"

echo "== a rotated tunnel address does push =="
$PUB --book "$REMOTE_ENTRY" --file host.json --name quiet-egret \
    --game arena --url wss://new-arena.example --proto 15 \
    --game fire --url wss://new-fire.example --proto 2 \
    --game kings --url wss://rotated-kings.example --proto 1 \
    --version r582 --commit bbb2222 --by ember@sokol >/dev/null
bash "$DEPLOY/republish-host.sh" sokol --repo "$PAGES" --branch gh-pages > "$TMP/third.log"
THIRD="$(git --git-dir="$PAGES" rev-parse refs/heads/gh-pages)"
if [ "$THIRD" != "$SECOND" ]; then ok "the changed entry produced one new commit"; else bad "the changed entry did not move the branch"; fi
rm -rf "$CHECK"
git clone -q --branch gh-pages "$PAGES" "$CHECK"
is "$(jget "$CHECK/server.json" '[h for h in d["hosts"] if h["name"] == "quiet-egret"][0]["kings_ws"]')" "wss://rotated-kings.example" "the rotated address replaced the old one"
is "$(grep -c 'ember-host/run/host.json' "$SHIM_LOG")" "3" "every run fetched host.json over ssh"

summary republish-host
