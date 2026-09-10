#!/usr/bin/env bash
# republish-host.sh against fake ssh and a local bare mirror repository.
#
#   bash deploy/tests/test-republish-host.sh
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
DEPLOY="$(cd "$HERE/.." && pwd)"
# shellcheck source=deploy/tests/lib.sh
. "$HERE/lib.sh"

TMP="$(mktemp -d -t ember-republishtest-XXXXXX)"
trap 'rm -rf "$TMP"' EXIT
MIRROR="$TMP/mirror.git"
REMOTE_ENTRY="$TMP/host.json"
PUB="bash $DEPLOY/publish-host.sh"
git init -q --bare "$MIRROR"

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

echo "== a destination must be explicit =="
if bash "$DEPLOY/republish-host.sh" sokol > "$TMP/refuse.log" 2>&1; then
    bad "a missing mirror destination was accepted"
else
    ok "a missing mirror destination is refused"
fi
contains "$(cat "$TMP/refuse.log")" "explicit --repo is required" "the refusal names the missing setting"

echo "== first republish writes the fetched entry =="
bash "$DEPLOY/republish-host.sh" sokol --repo "$MIRROR" --branch host-book > "$TMP/first.log"
CHECK="$TMP/check"
git clone -q --branch host-book "$MIRROR" "$CHECK"
is "$(jget "$CHECK/host.json" 'd["name"]')" "quiet-egret" "the mirror is bound to the fetched host"
is "$(jget "$CHECK/host.json" 'd["kings_ws"]')" "wss://new-kings.example" "the fetched Kings address landed"
FIRST="$(git -C "$CHECK" rev-parse HEAD)"

echo "== an unchanged fetch does not push =="
bash "$DEPLOY/republish-host.sh" sokol --repo "$MIRROR" --branch host-book > "$TMP/second.log"
SECOND="$(git --git-dir="$MIRROR" rev-parse refs/heads/host-book)"
is "$SECOND" "$FIRST" "the branch did not move"
contains "$(cat "$TMP/second.log")" "unchanged; nothing to push" "the no-op is explicit"

echo "== a rotated tunnel address does push =="
$PUB --book "$REMOTE_ENTRY" --file host.json --name quiet-egret \
    --game arena --url wss://new-arena.example --proto 15 \
    --game fire --url wss://new-fire.example --proto 2 \
    --game kings --url wss://rotated-kings.example --proto 1 \
    --version r582 --commit bbb2222 --by ember@sokol >/dev/null
bash "$DEPLOY/republish-host.sh" sokol --repo "$MIRROR" --branch host-book > "$TMP/third.log"
THIRD="$(git --git-dir="$MIRROR" rev-parse refs/heads/host-book)"
if [ "$THIRD" != "$SECOND" ]; then ok "the changed entry produced one new commit"; else bad "the changed entry did not move the branch"; fi
rm -rf "$CHECK"
git clone -q --branch host-book "$MIRROR" "$CHECK"
is "$(jget "$CHECK/host.json" 'd["kings_ws"]')" "wss://rotated-kings.example" "the rotated address replaced the old one"
is "$(grep -c 'ember-host/run/host.json' "$SHIM_LOG")" "3" "every run fetched host.json over ssh"

summary republish-host
