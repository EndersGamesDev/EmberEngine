#!/usr/bin/env bash
# host.sh and bootstrap-host.sh in prebuilt mode: no clone, no compiler.
#
#   bash deploy/tests/test-host-prebuilt.sh
#
# The claim under test is narrow and load-bearing: a host that cannot build
# runs the shipped binaries and NEVER reaches for git or cargo. So both are on
# PATH here as shims that fail and record the attempt, and the suite asserts
# the log stayed empty of them — an assertion that a missing binary would have
# turned into a confusing failure somewhere else instead.
#
# The rest is the same contract test-host-kings.sh applies to the default
# mode, against the same fake products: the launch argv, the commit-aware
# probes, the eight pid files, the local entry, the redeploy rule and the exact
# shutdown — plus the tunnel rules, which a prebuilt host needs exactly as
# much as a building one and gets from the same code: a live tunnel kept
# across a redeploy, `tunnels` repairing one without touching a server, and
# EMBER_NO_MINT refusing to mint at all.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
DEPLOY="$(cd "$HERE/.." && pwd)"
# shellcheck source=deploy/tests/lib.sh
. "$HERE/lib.sh"

TMP="$(mktemp -d -t ember-prebuilt-hosttest-XXXXXX)"
pidof_file() { cut -d' ' -f1 "$1"; }
cleanup() {
    for file in "$TMP"/home/run/*.pid; do
        [ -f "$file" ] || continue
        kill -9 "$(pidof_file "$file")" 2>/dev/null || true
    done
    rm -rf "$TMP"
}
trap cleanup EXIT

FULL="129bcac4000000000000000000000000000000ff"
SHORT="129bcac4"

mkdir -p "$TMP/bin" "$TMP/prebuilt"
cp "$HERE/shims/cloudflared-stub.sh" "$TMP/bin/cloudflared"
chmod +x "$TMP/bin/cloudflared"
# git and cargo exist and refuse. A prebuilt host must not call either, and a
# recorded refusal names the caller where a "command not found" would not.
for tool in git cargo; do
    cat > "$TMP/bin/$tool" <<TOOL
#!/usr/bin/env bash
echo "$tool \$*" >> "\${SHIM_LOG:-/dev/null}"
exit 1
TOOL
    chmod +x "$TMP/bin/$tool"
done

write_prebuilt() {
    local full="$1"
    for product in arena-server fire-server kings-server league-server; do
        cp "$HERE/shims/host-server-stub.sh" "$TMP/prebuilt/$product"
    done
    for product in wsbot fire-probe kings-probe league-probe; do
        cp "$HERE/shims/host-probe-stub.sh" "$TMP/prebuilt/$product"
    done
    chmod +x "$TMP/prebuilt"/*
    cat > "$TMP/prebuilt/stamp" <<STAMP
version=r1490
commit=$SHORT
full_commit=$full
arena_proto=22
fire_proto=1
kings_proto=1
league_proto=2
STAMP
}
write_prebuilt "$FULL"

export PATH="$TMP/bin:$PATH"
export SHIM_LOG="$TMP/wiring.log"
export EMBER_CONF_DIR="$TMP/conf"
export EMBER_NAME_FILE="$TMP/conf/host-name"
export EMBER_HOST_NAME="quiet-egret"
export EMBER_HOME="$TMP/home"
export EMBER_PREBUILT="$TMP/prebuilt"
export EMBER_PUBLISH=none
export EMBER_TUNNEL_BIN="$TMP/bin/cloudflared"
export EMBER_ARENA_PORT=17790
export EMBER_FIRE_PORT=17791
export EMBER_KINGS_PORT=17792
export EMBER_LEAGUE_PORT=17793

echo "== an incomplete directory is refused before anything is stopped =="
rm -f "$TMP/prebuilt/league-probe"
if bash "$DEPLOY/host.sh" up > "$TMP/refuse.log" 2>&1; then
    bad "up accepted a directory with a missing product"
else
    contains "$(cat "$TMP/refuse.log")" "league-probe is missing" "the missing product is named"
fi
is "$(find "$EMBER_HOME/run" -name '*.pid' -type f 2>/dev/null | wc -l | tr -d ' ')" "0" "nothing was started on the refusal"
write_prebuilt "$FULL"

echo "== a stamp missing a game's protocol is refused the same way =="
# This is the upgrade path, not a hypothetical: a directory shipped by a
# six-product shipper carries no league_proto, and the number is the only
# place a node with no checkout can answer `proto_of` from. Refusing here
# means the host keeps serving the build it has; accepting would publish a
# league address with no protocol beside it, which the book cannot rank.
grep -v '^league_proto=' "$TMP/prebuilt/stamp" > "$TMP/stamp.old"
mv "$TMP/stamp.old" "$TMP/prebuilt/stamp"
if bash "$DEPLOY/host.sh" up > "$TMP/refuse-stamp.log" 2>&1; then
    bad "up accepted a stamp with no league_proto"
else
    contains "$(cat "$TMP/refuse-stamp.log")" "has no league_proto" "the missing stamp field is named"
fi
is "$(find "$EMBER_HOME/run" -name '*.pid' -type f 2>/dev/null | wc -l | tr -d ' ')" "0" "nothing was started on the stamp refusal"
# `status` answers from the same stamp and must stay usable: an operator
# diagnosing the refusal has nothing else to ask.
bash "$DEPLOY/host.sh" status > "$TMP/status-stamp.log" 2>&1 || bad "status failed on an incomplete stamp"
contains "$(cat "$TMP/status-stamp.log")" "prebuilt:  $TMP/prebuilt -> $SHORT" "status still names the directory and its commit"
write_prebuilt "$FULL"

echo "== prebuilt up =="
if bash "$DEPLOY/host.sh" up > "$TMP/up.log" 2>&1; then
    ok "host.sh up succeeded with no repository and no toolchain"
else
    bad "host.sh up failed"
    tail -60 "$TMP/up.log" >&2
    summary host-prebuilt
    exit 1
fi

WIRE="$(cat "$SHIM_LOG")"
contains "$WIRE" "arena-server [--bind] [127.0.0.1:17790]" "the arena launch argv is unchanged in prebuilt mode"
contains "$WIRE" "fire-server [127.0.0.1:17791]" "the fire launch argv is unchanged in prebuilt mode"
contains "$WIRE" "kings-server [127.0.0.1:17792] [--name] [quiet-egret]" "Kings still launches with its name"
contains "$WIRE" "league-server [127.0.0.1:17793] [--name] [quiet-egret]" "League still launches with its name"
contains "$WIRE" "wsbot [ws://127.0.0.1:17790]" "the arena is probed with the shipped wsbot"
contains "$WIRE" "fire-probe [ws://127.0.0.1:17791]" "fire is probed with the shipped probe"
contains "$WIRE" "kings-probe [ws://127.0.0.1:17792] [--expect-commit] [$SHORT]" "the Kings probe checks the stamp's commit"
is "$(grep -c '^kings-probe .*--expect-commit' "$SHIM_LOG")" "2" "Kings is checked locally and publicly"
contains "$WIRE" "league-probe [ws://127.0.0.1:17793] [health-local] [--expect-commit] [$SHORT]" "the League probe names its lobby and checks the stamp's commit"
is "$(grep -c '^league-probe .*--expect-commit' "$SHIM_LOG")" "2" "League is checked locally and publicly"
is "$(grep -c '^league-probe \[ws://127.0.0.1:17793\] \[health-public\]' "$SHIM_LOG")" "1" "the public League probe uses a lobby name the loopback one is not holding"
is "$(grep -cE '^(git|cargo) ' "$SHIM_LOG")" "0" "neither git nor cargo was called"
if [ -e "$EMBER_HOME/src" ]; then bad "prebuilt mode created a source checkout"; else ok "no source checkout was created"; fi

echo "== the shipped stamp is what gets published =="
LOCAL="$EMBER_HOME/run/host.json"
is "$(jget "$LOCAL" 'd["name"]')" "quiet-egret" "the local entry names the host"
is "$(jget "$LOCAL" 'd["version"]')" "r1490" "the entry carries the stamp's version"
is "$(jget "$LOCAL" 'd["commit"]')" "$SHORT" "the entry carries the stamp's commit"
is "$(jget "$LOCAL" 'd["proto"]')" "22" "the arena protocol came from the stamp"
is "$(jget "$LOCAL" 'd["fire_proto"]')" "1" "the fire protocol came from the stamp"
is "$(jget "$LOCAL" 'd["kings_proto"]')" "1" "the Kings protocol came from the stamp"
is "$(jget "$LOCAL" 'd["league_proto"]')" "2" "the League protocol came from the stamp"
is "$(cat "$EMBER_HOME/run/deployed")" "$FULL" "the full commit is what was recorded as deployed"
is "$(find "$EMBER_HOME/run" -name '*.pid' -type f | wc -l | tr -d ' ')" "8" "four server/tunnel pid pairs exist"

echo "== status reads the stamp, not a checkout =="
bash "$DEPLOY/host.sh" status > "$TMP/status.log" 2>&1 || bad "status failed"
contains "$(cat "$TMP/status.log")" "prebuilt:  $TMP/prebuilt -> $SHORT (r1490)" "status names the prebuilt directory and its stamp"
contains "$(cat "$TMP/status.log")" "arena server: running" "status still reports the servers"

echo "== an unchanged stamp is not a redeploy =="
BEFORE="$(pidof_file "$EMBER_HOME/run/server-kings.pid")"
bash "$DEPLOY/host.sh" update > "$TMP/update.log" 2>&1 || bad "unchanged update failed"
contains "$(cat "$TMP/update.log")" "all four servers are running" "update requires all four servers"
is "$(pidof_file "$EMBER_HOME/run/server-kings.pid")" "$BEFORE" "an unchanged stamp restarted nothing"
is "$(grep -cE '^(git|cargo) ' "$SHIM_LOG")" "0" "update asked no git either"

echo "== a moved stamp is a redeploy, and the live tunnels survive it =="
TUNNEL_BEFORE="$(pidof_file "$EMBER_HOME/run/tunnel-arena.pid")"
URL_BEFORE="$(cat "$EMBER_HOME/run/arena.url")"
MOVED="7500af5d000000000000000000000000000000ff"
write_prebuilt "$MOVED"
bash "$DEPLOY/host.sh" update > "$TMP/moved.log" 2>&1 || bad "update on a moved stamp failed"
contains "$(cat "$TMP/moved.log")" "-> 7500af5; redeploying" "the moved stamp is what triggered it"
if [ "$(pidof_file "$EMBER_HOME/run/server-kings.pid")" != "$BEFORE" ]; then
    ok "the moved stamp restarted the servers"
else
    bad "the moved stamp left the old servers running"
fi
is "$(cat "$EMBER_HOME/run/deployed")" "$MOVED" "the new full commit was recorded"
is "$(pidof_file "$EMBER_HOME/run/tunnel-arena.pid")" "$TUNNEL_BEFORE" "the live tunnel was kept across the redeploy"
is "$(cat "$EMBER_HOME/run/arena.url")" "$URL_BEFORE" "so the public address did not rotate"
contains "$(cat "$TMP/moved.log")" "keeping the arena tunnel: $URL_BEFORE" "and it said so"
is "$(grep '^commit=' "$EMBER_HOME/run/deployed-stamp" | cut -d= -f2)" "$SHORT" "the running stamp was recorded beside it"

echo "== tunnels repairs a tunnel without touching a server =="
SERVER_BEFORE="$(pidof_file "$EMBER_HOME/run/server-arena.pid")"
KINGS_TUNNEL="$(pidof_file "$EMBER_HOME/run/tunnel-kings.pid")"
kill -9 "$(pidof_file "$EMBER_HOME/run/tunnel-arena.pid")" 2>/dev/null || true
# A shipper leaves a NEWER directory beside the running servers. What gets
# republished must be what is answering, not what is waiting to be deployed.
write_prebuilt "aaaa1111000000000000000000000000000000ff"
bash "$DEPLOY/host.sh" tunnels > "$TMP/tunnels.log" 2>&1 || bad "tunnels failed"
is "$(pidof_file "$EMBER_HOME/run/server-arena.pid")" "$SERVER_BEFORE" "no server was restarted"
is "$(pidof_file "$EMBER_HOME/run/tunnel-kings.pid")" "$KINGS_TUNNEL" "the healthy Kings tunnel was kept"
if [ "$(pidof_file "$EMBER_HOME/run/tunnel-arena.pid")" != "$TUNNEL_BEFORE" ]; then
    ok "the dead arena tunnel was minted again"
else
    bad "the dead arena tunnel was not replaced"
fi
is "$(jget "$LOCAL" 'd["commit"]')" "$SHORT" "the republished entry names the RUNNING build, not the shipped one"
is "$(grep -cE '^(git|cargo) ' "$SHIM_LOG")" "0" "tunnels asked no git either, with no checkout to ask"

echo "== EMBER_NO_MINT refuses to mint, and leaves the servers alone =="
SERVER_BEFORE="$(pidof_file "$EMBER_HOME/run/server-fire.pid")"
kill -9 "$(pidof_file "$EMBER_HOME/run/tunnel-fire.pid")" 2>/dev/null || true
set +e
EMBER_NO_MINT=1 bash "$DEPLOY/host.sh" tunnels > "$TMP/nomint.log" 2>&1
NOMINT_RC=$?
set -e
is "$NOMINT_RC" "3" "a tunnel it may not mint is exit 3"
contains "$(cat "$TMP/nomint.log")" "EMBER_NO_MINT is set; not minting one" "and it says why"
is "$(pidof_file "$EMBER_HOME/run/server-fire.pid")" "$SERVER_BEFORE" "the fire server was not restarted for a tunnel"
if [ -e "$EMBER_HOME/run/fire.url" ]; then bad "a dead tunnel kept its address"; else ok "the dead tunnel's address was dropped"; fi
if "$PY" -c 'import json,sys; sys.exit(0 if "fire_ws" not in json.load(open(sys.argv[1])) else 1)' "$LOCAL"; then
    ok "and the game was dropped from the entry rather than published at a dead address"
else
    bad "the entry still names the dead fire address"
fi

# Put fire back so the shutdown contract below still has four pairs.
write_prebuilt "$MOVED"
bash "$DEPLOY/host.sh" tunnels > "$TMP/repair.log" 2>&1 || bad "repairing fire failed"

echo "== down stops exactly the four pairs =="
PIDS=()
for game in arena fire kings league; do
    PIDS+=("$(pidof_file "$EMBER_HOME/run/server-$game.pid")")
    PIDS+=("$(pidof_file "$EMBER_HOME/run/tunnel-$game.pid")")
done
bash "$DEPLOY/host.sh" down > "$TMP/down.log" 2>&1 || bad "down failed"
for pid in "${PIDS[@]}"; do
    if kill -0 "$pid" 2>/dev/null; then bad "pid $pid survived down"; else ok "pid $pid stopped"; fi
done
is "$(find "$EMBER_HOME/run" -name '*.pid' -type f | wc -l | tr -d ' ')" "0" "all eight pid files were removed"

echo "== bootstrap needs a toolchain, unless it is preparing a prebuilt host =="
# Only python3, curl and sha256sum on PATH: the state a bare pod is actually
# in. `cargo` and `git` above are deliberately NOT here.
mkdir -p "$TMP/bare"
for tool in bash python3 sha256sum mktemp uname chmod mkdir mv ln rm cut cat grep sed head tr; do
    real="$(command -v "$tool" || true)"
    [ -n "$real" ] && ln -sf "$real" "$TMP/bare/$tool"
done
# curl is present and reaches nothing: this suite contacts no network, and the
# run has to END at the download rather than sail past it into one.
cat > "$TMP/bare/curl" <<'CURL'
#!/usr/bin/env bash
echo "curl $*" >> "${SHIM_LOG:-/dev/null}"
exit 22
CURL
chmod +x "$TMP/bare/curl"
BOOT_HOME="$TMP/boothome"
mkdir -p "$BOOT_HOME"

set +e
DEFAULT_OUT="$(unset EMBER_PREBUILT; PATH="$TMP/bare" HOME="$BOOT_HOME" \
    bash "$DEPLOY/bootstrap-host.sh" 2>&1)"
DEFAULT_RC=$?
set -e
is "$DEFAULT_RC" "1" "the default mode refuses a host with no toolchain"
contains "$DEFAULT_OUT" "missing git" "and says which dependency is missing"

# Nothing serves the download here, so this run ends at it — which is the
# point twice over: it proves the toolchain check was skipped, and it proves
# the pinned-and-verified install is still the next thing that happens.
set +e
PRE_OUT="$(PATH="$TMP/bare" HOME="$BOOT_HOME" EMBER_PREBUILT="$TMP/prebuilt" \
    bash "$DEPLOY/bootstrap-host.sh" 2>&1)"
PRE_RC=$?
set -e
contains "$PRE_OUT" "skipping the git and Rust toolchain checks" "prebuilt mode skips the toolchain check"
contains "$PRE_OUT" "downloading cloudflared" "and goes on to the pinned cloudflared"
is "$PRE_RC" "1" "a cloudflared it cannot fetch is still fatal"
case "$PRE_OUT" in
    *"missing Rust toolchain"*|*"missing git"*) bad "prebuilt bootstrap still demanded a toolchain" ;;
    *) ok "prebuilt bootstrap demanded neither git nor a toolchain" ;;
esac

summary host-prebuilt
