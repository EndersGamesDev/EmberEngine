#!/usr/bin/env bash
# ship-host.sh against PATH shims for ssh, scp and curl: no machine anywhere.
#
#   bash deploy/tests/test-ship-host.sh
#
# Four things are pinned, and they are the four ways this script can be wrong
# without anybody noticing until players cannot join:
#
#   the read      it must ask the PUBLISHED version.json, cache-busted, and
#                 build the commit that answer names rather than a newer one
#   the build     the builder must be handed that commit, the configured
#                 environment and run wrapper, and a stamp taken from the
#                 commit itself
#   the copy      the products, deploy/ and the stamp must reach the host, and
#                 the entry must come back
#   the verdict   check's exit code is what a scheduler acts on: 0 running the
#                 published build, 3 redeploy due, 4 cannot tell
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
DEPLOY="$(cd "$HERE/.." && pwd)"
# shellcheck source=deploy/tests/lib.sh
. "$HERE/lib.sh"

TMP="$(mktemp -d -t ember-shiptest-XXXXXX)"
trap 'rm -rf "$TMP"' EXIT

PUB_COMMIT="129bcac4"
PUB_FULL="129bcac4000000000000000000000000000000ff"

mkdir -p "$TMP/bin" "$TMP/conf"
cp "$HERE/shims/ship-ssh.sh" "$TMP/bin/ssh"
cp "$HERE/shims/ship-scp.sh" "$TMP/bin/scp"
cp "$HERE/shims/ship-curl.sh" "$TMP/bin/curl"
chmod +x "$TMP/bin/ssh" "$TMP/bin/scp" "$TMP/bin/curl"

cat > "$TMP/version.json" <<JSON
{
  "version": "r1490",
  "commit": "$PUB_COMMIT",
  "built": "2026-09-06T16:21:51Z",
  "subject": "a published build"
}
JSON

# What the host writes for itself after a successful up: the same shape
# host.sh's write_local_entry produces.
host_entry() {
    cat > "$TMP/host.json" <<JSON
{
  "name": "lundi",
  "ws": "wss://arena.example",
  "proto": 22,
  "fire_ws": "wss://fire.example",
  "fire_proto": 1,
  "kings_ws": "wss://kings.example",
  "kings_proto": 1,
  "league_ws": "wss://league.example",
  "league_proto": 2,
  "version": "r1490",
  "commit": "$1",
  "updated": "2026-09-06T17:00:00Z",
  "by": "ember@lundi"
}
JSON
}
host_entry "$PUB_COMMIT"

export PATH="$TMP/bin:$PATH"
export SHIM_LOG="$TMP/shim.log"
export SHIP_VERSION_JSON="$TMP/version.json"
export SHIP_HOST_JSON="$TMP/host.json"
export SHIP_BUILD_SCRIPT="$TMP/build-script.sh"
export SHIP_BUILT_COMMIT="$PUB_COMMIT"
export SHIP_BUILT_FULL="$PUB_FULL"

export EMBER_CONF_DIR="$TMP/conf"
export EMBER_SHIP_HOST="lundi-ember"
export EMBER_SHIP_BUILDER="sokol-worker"
export EMBER_SHIP_BUILDER_SSH_CONFIG="$TMP/conf/builder_ssh_config"
export EMBER_SHIP_BUILD_DIR="lane-ship"
export EMBER_SHIP_BUILDER_STAGE="ember-ship-products"
export EMBER_SHIP_REPO="/workspace/git/ember.git"
export EMBER_SHIP_BUILD_ENV="export CARGO_TARGET_DIR=/workspace/target-ship"
export EMBER_SHIP_BUILD_WRAP="/workspace/run-report.sh chrt --idle 0 ionice -c3"
export EMBER_SHIP_VERSION_URL="https://example.invalid/EmberEngine/version.json"
export EMBER_SHIP_STAGE="$TMP/stage"
export EMBER_SHIP_REMOTE_ROOT="ember-prebuilt"
export EMBER_SHIP_HOST_NAME="lundi"
export EMBER_SHIP_PUBLISH=none
: > "$EMBER_SHIP_BUILDER_SSH_CONFIG"

echo "== ship-host.sh writes its own configuration file =="
set +e
bash "$DEPLOY/ship-host.sh" >/dev/null 2>&1
USAGE_RC=$?
set -e
is "$USAGE_RC" "2" "no subcommand is a usage error"
if [ -f "$TMP/conf/ship.env" ]; then ok "ship.env was written on first run"; else bad "no ship.env"; fi
contains "$(cat "$TMP/conf/ship.env")" "#EMBER_SHIP_BUILDER=" "every setting is there, commented out at its default"

echo "== deploy =="
bash "$DEPLOY/ship-host.sh" deploy > "$TMP/deploy.log" 2>&1 || {
    bad "deploy failed"
    tail -40 "$TMP/deploy.log" >&2
    summary ship-host
    exit 1
}
ok "deploy succeeded"
LOG="$(cat "$SHIM_LOG")"
DEPLOY_OUT="$(cat "$TMP/deploy.log")"

echo "== the read =="
contains "$LOG" "[https://example.invalid/EmberEngine/version.json?ship=" "version.json was read cache-busted"
contains "$LOG" "[Cache-Control: no-cache]" "and with no-cache, because a stale answer is the one it cannot use"
contains "$LOG" "[--fail]" "a 404 fails rather than being read as a book"
contains "$DEPLOY_OUT" "the pages publish r1490 · $PUB_COMMIT" "the published build is what it announced"

echo "== the build =="
BUILD="$(cat "$SHIP_BUILD_SCRIPT")"
contains "$BUILD" "COMMIT=\"$PUB_COMMIT\"" "the builder was given the published commit"
contains "$BUILD" "export CARGO_TARGET_DIR=/workspace/target-ship" "the builder's own environment line came first"
contains "$BUILD" "git checkout -q --detach" "the builder checks the commit out detached"
contains "$BUILD" 'VERSION="r$(git rev-list --count HEAD)"' "the version is the commit's own count"
contains "$BUILD" 'export EMBER_BUILD_VERSION="$VERSION" EMBER_BUILD_COMMIT="$SHORT"' "the stamp reaches the compiler"
contains "$BUILD" 'WRAP="/workspace/run-report.sh chrt --idle 0 ionice -c3"' "the configured run wrapper is used"
contains "$BUILD" '$WRAP cargo build --release -p "$ARENA" -p fire-server -p kings-server -p league-server' "the four servers are one wrapped build"
contains "$BUILD" '$WRAP cargo build --release -p "$ARENA" --example wsbot' "the arena probe is built too"
contains "$BUILD" 'cp "$TD/release/examples/probe" "$TD/release/examples/kings-probe"' "each probe is named apart, as host.sh does"
contains "$BUILD" 'cp "$TD/release/examples/kings-probe" "$STAGE/kings-probe"' "and staged under the name the host looks for"
contains "$BUILD" '$WRAP cargo build --release -p league-server --example wsprobe' "League's probe is built from the example its own crate declares"
contains "$BUILD" 'cp "$TD/release/examples/wsprobe" "$TD/release/examples/league-probe"' "and renamed from wsprobe to the per-game name"
contains "$BUILD" 'cp "$TD/release/league-server" "$STAGE/league-server"' "the league server is staged too"
contains "$BUILD" 'cp "$TD/release/examples/league-probe" "$STAGE/league-probe"' "and its probe beside it"
is "$(grep -c '"\$STAGE/' "$SHIP_BUILD_SCRIPT")" "8" "exactly eight products are staged"
contains "$BUILD" 'if [ -d crates/arena-core ]; then ARENA=arena-server; else ARENA=pong-server; fi' "a pre-rename commit is still buildable"
contains "$BUILD" 'echo "SHIP full_commit=$FULL"' "the builder reports the full commit"
contains "$BUILD" 'echo "SHIP stage=$(cd "$STAGE" && pwd)"' "and the resolved staging path, because scp expands no remote variable"
case "$BUILD" in
    *cargo*install*|*rustup*) bad "the build script installs a toolchain" ;;
    *) ok "the build script installs nothing" ;;
esac

echo "== the stamp =="
STAMP="$TMP/stage/products/stamp"
is "$(grep '^commit=' "$STAMP" | cut -d= -f2)" "$PUB_COMMIT" "the stamp carries the published short commit"
is "$(grep '^full_commit=' "$STAMP" | cut -d= -f2)" "$PUB_FULL" "and the full one, which is what update compares"
is "$(grep '^version=' "$STAMP" | cut -d= -f2)" "r1490" "and the version the pages name"
is "$(grep '^arena_proto=' "$STAMP" | cut -d= -f2)" "22" "and the arena protocol read at that commit"
is "$(grep '^league_proto=' "$STAMP" | cut -d= -f2)" "2" "and League's, which a prebuilt host has no crate to read"

echo "== the copy list =="
contains "$LOG" "scp [-F] [$EMBER_SHIP_BUILDER_SSH_CONFIG]" "the builder is reached through its own ssh config"
contains "$LOG" "[sokol-worker:/workspace/loops/ember/ember-ship-products] [$TMP/stage/products]" "the products came off the path the builder resolved"
# What landed, not just what was asked for. host.sh refuses a directory that
# is missing any product, so the set the shipper collects is the set the host
# will accept or reject; the shim fabricates the builder's output and this is
# what keeps the two descriptions of it together.
is "$(find "$TMP/stage/products" -maxdepth 1 -type f ! -name stamp | wc -l | tr -d ' ')" "8" "eight products were collected onto the workstation"
for product in arena-server fire-server kings-server league-server wsbot fire-probe kings-probe league-probe; do
    if [ -x "$TMP/stage/products/$product" ]; then ok "$product arrived executable"; else bad "$product is missing or not executable"; fi
done
contains "$LOG" "[lundi-ember:ember-prebuilt/$PUB_COMMIT.incoming/]" "and went to a sibling of the directory named by the commit"
contains "$LOG" "mv 'ember-prebuilt/$PUB_COMMIT.incoming' 'ember-prebuilt/$PUB_COMMIT'" "which is then renamed over it, because a running binary cannot be written to"
contains "$LOG" "[$DEPLOY/host.sh]" "deploy/ travelled with them"
contains "$LOG" "[$DEPLOY/bootstrap-host.sh]" "bootstrap-host.sh included"
contains "$LOG" "[$DEPLOY/publish-host.sh]" "and the book writer host.sh calls"
contains "$LOG" "[lundi-ember:ember-host/run/host.json]" "the host's own entry came back"
if [ -s "$TMP/stage/host.json" ]; then ok "and was written down"; else bad "the fetched entry is empty"; fi
contains "$DEPLOY_OUT" '"name": "lundi"' "the entry was printed"

echo "== the name is left on the machine, not only in the invocation =="
contains "$LOG" "\$HOME/.ember/host-name" "the configured name was written to the host's own name file"

echo "== what the host was told to run =="
contains "$LOG" "EMBER_PREBUILT=\"\$HOME/ember-prebuilt/$PUB_COMMIT\" bash \"\$HOME/ember-prebuilt/deploy/bootstrap-host.sh\"" "bootstrap ran in prebuilt mode, with a path the remote shell will expand"
contains "$LOG" "EMBER_HOST_NAME='lundi' EMBER_PREBUILT=\"\$HOME/ember-prebuilt/$PUB_COMMIT\" EMBER_PUBLISH='none' bash \"\$HOME/ember-prebuilt/deploy/host.sh\" up" "host.sh up ran in prebuilt mode, publishing nothing"
case "$LOG" in
    *pkill*) bad "something reached for pkill" ;;
    *) ok "nothing reached for pkill" ;;
esac
case "$LOG" in
    *"git push"*|*gh-pages*) bad "the ship script touched a git remote" ;;
    *) ok "no git remote was touched" ;;
esac
contains "$DEPLOY_OUT" "republish-host.sh lundi-ember" "and it names the separate, credentialed publish step"

echo "== a builder that stamped something else is refused =="
: > "$SHIM_LOG"
set +e
SHIP_BUILT_COMMIT="deadbee1" bash "$DEPLOY/ship-host.sh" deploy > "$TMP/wrong.log" 2>&1
WRONG_RC=$?
set -e
is "$WRONG_RC" "2" "a mismatched build is a configuration failure"
contains "$(cat "$TMP/wrong.log")" "the builder stamped deadbee1 but version.json names $PUB_COMMIT" "and says so exactly"
case "$(cat "$SHIM_LOG")" in
    *"host.sh\" up"*) bad "the mismatched build was shipped anyway" ;;
    *) ok "nothing was shipped" ;;
esac

echo "== check =="
: > "$SHIM_LOG"
set +e
bash "$DEPLOY/ship-host.sh" check > "$TMP/check-ok.log" 2>&1
OK_RC=$?
set -e
is "$OK_RC" "0" "0 when the host runs the published commit and every game answered"
contains "$(cat "$TMP/check-ok.log")" "arena answered through wss://arena.example" "each game is probed by its own address"
contains "$(cat "$TMP/check-ok.log")" "fire answered through wss://fire.example" "fire included"
contains "$(cat "$TMP/check-ok.log")" "kings answered through wss://kings.example" "Kings included"
contains "$(cat "$TMP/check-ok.log")" "league answered through wss://league.example" "League included"
is "$(grep -c 'answered through' "$TMP/check-ok.log")" "4" "all four were probed, not one and a swallowed list"
contains "$(cat "$SHIM_LOG")" "ember-prebuilt/$PUB_COMMIT/kings-probe" "the probe that runs is the one already on the host"
contains "$(cat "$SHIM_LOG")" "--expect-commit" "and Kings is asked which build answered"
contains "$(cat "$SHIM_LOG")" "ember-prebuilt/$PUB_COMMIT/league-probe" "League's probe is the shipped one too"
contains "$(cat "$SHIM_LOG")" "league-probe\" 'wss://league.example' ship-check --expect-commit" "and it is given a lobby name of its own beside the commit"

host_entry "0000aaa1"
set +e
bash "$DEPLOY/ship-host.sh" check > "$TMP/check-stale.log" 2>&1
STALE_RC=$?
set -e
is "$STALE_RC" "3" "3 when the published commit moved away from the host"
contains "$(cat "$TMP/check-stale.log")" "the host runs '0000aaa1' and the pages publish '$PUB_COMMIT'" "and names both sides"

host_entry "$PUB_COMMIT"
set +e
SHIP_PROBE_FAIL=1 bash "$DEPLOY/ship-host.sh" check > "$TMP/check-dead.log" 2>&1
DEAD_RC=$?
set -e
is "$DEAD_RC" "3" "3 when the commit is right but a game stopped answering"
contains "$(cat "$TMP/check-dead.log")" "arena did NOT answer" "and names the game"

set +e
SHIP_VERSION=missing bash "$DEPLOY/ship-host.sh" check > "$TMP/check-blind.log" 2>&1
BLIND_RC=$?
set -e
is "$BLIND_RC" "4" "4 when the pages cannot be read, which is not the same as a redeploy"

echo "== a short sha that grew a digit is still the same commit =="
host_entry "129bcac"
set +e
bash "$DEPLOY/ship-host.sh" check > "$TMP/check-short.log" 2>&1
SHORT_RC=$?
set -e
is "$SHORT_RC" "0" "seven characters against eight is not a redeploy"

summary ship-host
