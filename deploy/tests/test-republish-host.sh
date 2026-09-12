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

echo "== --per-host writes the host's own file, so one branch carries them all =="
# A branch per host needs an owner-run ruleset change before a new machine can
# publish its address at all. One branch and one file per host needs none.
bash "$DEPLOY/republish-host.sh" sokol --repo "$MIRROR" --branch host-book --per-host > "$TMP/per-host.log"
contains "$(cat "$TMP/per-host.log")" "as quiet-egret.json" "the run says which file it wrote"
rm -rf "$CHECK"
git clone -q --branch host-book "$MIRROR" "$CHECK"
is "$(jget "$CHECK/quiet-egret.json" 'd["name"]')" "quiet-egret" "the entry landed under the host's own name"
is "$(jget "$CHECK/quiet-egret.json" 'd["kings_ws"]')" "wss://rotated-kings.example" "with the address the host is actually serving"
if [ -f "$CHECK/host.json" ]; then ok "the shared host.json is left exactly where it was"; else bad "the shared host.json disappeared"; fi

echo "== the per-host file is compared against itself, not against host.json =="
PER_HOST_FIRST="$(git --git-dir="$MIRROR" rev-parse refs/heads/host-book)"
bash "$DEPLOY/republish-host.sh" sokol --repo "$MIRROR" --branch host-book --per-host > "$TMP/per-host-again.log"
is "$(git --git-dir="$MIRROR" rev-parse refs/heads/host-book)" "$PER_HOST_FIRST" "an unchanged per-host entry does not move the branch"
contains "$(cat "$TMP/per-host-again.log")" "unchanged; nothing to push" "and says so, as the shared-file path does"

echo "== a second host shares the branch without touching the first =="
SECOND="$TMP/second-host.json"
$PUB --book "$SECOND" --file host.json --name amber-otter \
    --game arena --url wss://amber-arena.example --proto 15 \
    --version r600 --commit ccc3333 --by ember@amber >/dev/null
REPUBLISH_HOST_JSON="$SECOND" bash "$DEPLOY/republish-host.sh" amber --repo "$MIRROR" --branch host-book --per-host > "$TMP/amber.log"
rm -rf "$CHECK"
git clone -q --branch host-book "$MIRROR" "$CHECK"
is "$(jget "$CHECK/amber-otter.json" 'd["ws"]')" "wss://amber-arena.example" "the second host published its own file"
is "$(jget "$CHECK/quiet-egret.json" 'd["kings_ws"]')" "wss://rotated-kings.example" "and left the first host's file alone"

echo "== a host named 'server' cannot write the address book =="
# publish-host.sh decides mirror mode by the basename, so `server.json` would
# be read as the whole book and one host's scheduler would replace every other
# host's entry with its own single one. The name generator cannot produce it,
# but "cannot happen" is not a check and the cost of being wrong is the fleet.
SERVER_HOST="$TMP/server-host.json"
$PUB --book "$SERVER_HOST" --file host.json --name server \
    --game arena --url wss://server-arena.example --proto 15 \
    --version r700 --commit ddd4444 --by ember@server >/dev/null
BEFORE_SERVER="$(git --git-dir="$MIRROR" rev-parse refs/heads/host-book)"
if REPUBLISH_HOST_JSON="$SERVER_HOST" bash "$DEPLOY/republish-host.sh" srv --repo "$MIRROR" --branch host-book --per-host > "$TMP/server.log" 2>&1; then
    bad "a host named server was allowed to write server.json"
else
    ok "a host named server is refused under --per-host"
fi
contains "$(cat "$TMP/server.log")" "server.json is the address book itself" "the refusal says why"
is "$(git --git-dir="$MIRROR" rev-parse refs/heads/host-book)" "$BEFORE_SERVER" "and nothing was pushed"
# The refusal is specific to --per-host: the name is only dangerous when it
# becomes the file name. On its own branch the same host publishes normally.
if REPUBLISH_HOST_JSON="$SERVER_HOST" bash "$DEPLOY/republish-host.sh" srv --repo "$MIRROR" --branch server-book > "$TMP/server-shared.log" 2>&1; then
    ok "the same host still publishes normally to a shared host.json"
else
    bad "the shared-file path was refused too"
    cat "$TMP/server-shared.log" >&2
fi
rm -rf "$CHECK"
git clone -q --branch server-book "$MIRROR" "$CHECK"
is "$(jget "$CHECK/host.json" 'd["name"]')" "server" "under the shared name, where publish-host.sh reads it as one mirror entry"

summary republish-host
