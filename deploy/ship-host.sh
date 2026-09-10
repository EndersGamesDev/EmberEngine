#!/usr/bin/env bash
# Put the published build on a host that cannot build it (docs/hosts.md §8).
#
#   bash deploy/ship-host.sh deploy   build at the published commit and ship it
#   bash deploy/ship-host.sh check    is the host already running that build?
#
# The commit is the one the live pages name in version.json, not the newest one
# anywhere. A server built ahead of the published pages speaks a protocol those
# pages do not and refuses every player who arrives from them, while looking
# perfectly healthy to its own probes — so a node whose whole purpose is to be
# reachable from the live site follows the site, and follows it when it moves.
#
# Three machines, three jobs. THIS one orchestrates and holds no compiler. The
# BUILDER compiles, because compiling is all it is for. The HOST runs, and is
# never asked to build, clone or fetch anything. Nothing here pushes to a git
# remote: putting the entry into the address book needs the Pages key and stays
# with deploy/republish-host.sh on a workstation that has it.
#
# `check` reports in one exit code, so a scheduler can ask without being told:
#
#   0  the host runs the published commit and every game answered
#   3  a redeploy is due (the commit moved, or a game stopped answering)
#   4  cannot tell (version.json unreadable, or the host is out of reach)
#   2  usage or configuration
#
# Configuration lives in ~/.ember/ship.env, written on first run with every
# setting commented out at its default, exactly like host.env. A real value in
# the environment always beats the file.
set -euo pipefail

SELF_DIR="$(cd "$(dirname "$0")" && pwd)"
CMD="${1:-}"

usage() { sed -n '2,6p' "$0" >&2; exit 2; }

# --- configuration ---------------------------------------------------------
CONF_DIR="${EMBER_CONF_DIR:-$HOME/.ember}"
CONF="$CONF_DIR/ship.env"
mkdir -p "$CONF_DIR"
if [ ! -f "$CONF" ]; then
    cat > "$CONF" <<'ENV'
# ember ship-host configuration. Uncomment and edit what you want to change;
# the environment overrides anything set here.

# The host that RUNS the servers: an ssh alias, never an address. It needs
# python3, curl, sha256sum and a writable home, and nothing else.
#EMBER_SHIP_HOST=
# An ssh -F config file for that alias, when it does not live in ~/.ssh/config.
#EMBER_SHIP_HOST_SSH_CONFIG=

# The machine that BUILDS: another ssh alias, and its -F config when it needs
# one. It must run the same libc and architecture as the host, because what
# travels between them is a binary.
#EMBER_SHIP_BUILDER=
#EMBER_SHIP_BUILDER_SSH_CONFIG=

# Where the builder keeps its checkout of this repository, and where it puts
# the finished products before they are copied off. Both are the builder's
# paths, not this machine's.
# A relative path is relative to the builder's own home, which is what you
# want: this machine's $HOME is not that machine's.
#EMBER_SHIP_BUILD_DIR=ember-ship-build
#EMBER_SHIP_BUILDER_STAGE=ember-ship-products
# What the builder clones and fetches from.
#EMBER_SHIP_REPO=https://github.com/EndersGamesDev/EmberEngine.git

# One line sourced on the builder before cargo runs: its toolchain, its
# CARGO_HOME, its CARGO_TARGET_DIR. Everything the builder needs to be itself.
#EMBER_SHIP_BUILD_ENV=
# The builder's run wrapper, prefixed to every cargo invocation. Left empty,
# the build still runs at idle priority when chrt and ionice are there.
#EMBER_SHIP_BUILD_WRAP=

# Where the published build is announced.
#EMBER_SHIP_VERSION_URL=https://endersgamesdev.github.io/EmberEngine/version.json

# This machine's staging directory, and the host's root for shipped products.
#EMBER_SHIP_STAGE=$HOME/.ember/ship-stage
#EMBER_SHIP_REMOTE_ROOT=ember-prebuilt

# Passed through to host.sh on the host.
#EMBER_SHIP_HOST_NAME=
#EMBER_SHIP_PUBLISH=none
ENV
    echo "wrote $CONF (all defaults, nothing enabled)"
fi

# The file supplies defaults; a variable already exported into this process
# wins, so a one-off run needs no edit.
_PRE_ENV="$(export -p | grep -E '^(declare -x |export )EMBER_' || true)"
# shellcheck source=/dev/null
. "$CONF"
eval "$_PRE_ENV"

EMBER_SHIP_HOST="${EMBER_SHIP_HOST:-}"
EMBER_SHIP_HOST_SSH_CONFIG="${EMBER_SHIP_HOST_SSH_CONFIG:-}"
EMBER_SHIP_BUILDER="${EMBER_SHIP_BUILDER:-}"
EMBER_SHIP_BUILDER_SSH_CONFIG="${EMBER_SHIP_BUILDER_SSH_CONFIG:-}"
EMBER_SHIP_BUILD_DIR="${EMBER_SHIP_BUILD_DIR:-ember-ship-build}"
EMBER_SHIP_BUILDER_STAGE="${EMBER_SHIP_BUILDER_STAGE:-ember-ship-products}"
EMBER_SHIP_REPO="${EMBER_SHIP_REPO:-https://github.com/EndersGamesDev/EmberEngine.git}"
EMBER_SHIP_BUILD_ENV="${EMBER_SHIP_BUILD_ENV:-}"
EMBER_SHIP_BUILD_WRAP="${EMBER_SHIP_BUILD_WRAP:-}"
EMBER_SHIP_VERSION_URL="${EMBER_SHIP_VERSION_URL:-https://endersgamesdev.github.io/EmberEngine/version.json}"
EMBER_SHIP_STAGE="${EMBER_SHIP_STAGE:-$HOME/.ember/ship-stage}"
EMBER_SHIP_REMOTE_ROOT="${EMBER_SHIP_REMOTE_ROOT:-ember-prebuilt}"
EMBER_SHIP_HOST_NAME="${EMBER_SHIP_HOST_NAME:-}"
EMBER_SHIP_PUBLISH="${EMBER_SHIP_PUBLISH:-none}"

say() { echo "== $* =="; }
die() { echo "ship-host.sh: $*" >&2; exit 2; }
unknown() { echo "ship-host.sh: $*" >&2; exit 4; }

# An alias, never an address: a network address in a repository is a fact that
# rots, and the ssh config is where it belongs.
alias_ok() { [[ "$1" =~ ^[a-zA-Z0-9][a-zA-Z0-9._-]*$ ]]; }

PY=""
for cand in python3 python; do
    c="$(command -v "$cand" || true)"
    [ -n "$c" ] && "$c" -c '' >/dev/null 2>&1 || continue
    PY="$c"
    break
done
[ -n "$PY" ] || die "need a working python3 (or python) on PATH"

# --- reaching the two machines ---------------------------------------------
# BatchMode, because an interactive prompt in a scheduled run is a hang; a
# connect timeout, because the same run must fail rather than sit there.
SSH_BASE=(-o BatchMode=yes -o ConnectTimeout=15)

host_ssh() {
    local opts=("${SSH_BASE[@]}")
    [ -z "$EMBER_SHIP_HOST_SSH_CONFIG" ] || opts=(-F "$EMBER_SHIP_HOST_SSH_CONFIG" "${opts[@]}")
    ssh "${opts[@]}" "$EMBER_SHIP_HOST" "$@"
}
host_scp() {
    local opts=("${SSH_BASE[@]}")
    [ -z "$EMBER_SHIP_HOST_SSH_CONFIG" ] || opts=(-F "$EMBER_SHIP_HOST_SSH_CONFIG" "${opts[@]}")
    scp "${opts[@]}" "$@"
}
builder_ssh() {
    local opts=("${SSH_BASE[@]}")
    [ -z "$EMBER_SHIP_BUILDER_SSH_CONFIG" ] || opts=(-F "$EMBER_SHIP_BUILDER_SSH_CONFIG" "${opts[@]}")
    ssh "${opts[@]}" "$EMBER_SHIP_BUILDER" "$@"
}
# A bounded ssh, for the read-only questions `check` asks: a probe that hangs
# must become an answer, not a scheduler that never returns.
#
# `-n` is load-bearing. ssh reads its own stdin and forwards it, so a call
# inside `while read` swallows the rest of the list: the first game was
# probed, the other two vanished, and check reported success over servers it
# had never spoken to.
host_ssh_t() {
    local secs="$1"; shift
    local opts=("${SSH_BASE[@]}")
    [ -z "$EMBER_SHIP_HOST_SSH_CONFIG" ] || opts=(-F "$EMBER_SHIP_HOST_SSH_CONFIG" "${opts[@]}")
    timeout "$secs" ssh -n "${opts[@]}" "$EMBER_SHIP_HOST" "$@"
}
builder_scp() {
    local opts=("${SSH_BASE[@]}")
    [ -z "$EMBER_SHIP_BUILDER_SSH_CONFIG" ] || opts=(-F "$EMBER_SHIP_BUILDER_SSH_CONFIG" "${opts[@]}")
    scp "${opts[@]}" "$@"
}

# --- what the pages published ----------------------------------------------
# Cache-busted and no-cache, because the whole point of the read is to notice
# that the published build MOVED, and a cached answer is the one state this
# cannot afford to be in.
published_json() {
    local url="$EMBER_SHIP_VERSION_URL" sep
    case "$url" in *\?*) sep='&' ;; *) sep='?' ;; esac
    curl --fail --location --show-error --silent --max-time 30 \
        -H 'Cache-Control: no-cache' "${url}${sep}ship=$(date +%s)"
}

# published_field <json> <key>
published_field() {
    printf '%s' "$1" | "$PY" -c '
import json, sys
d = json.load(sys.stdin)
v = d.get(sys.argv[1], "") if isinstance(d, dict) else ""
print(v if isinstance(v, str) else "")
' "$2"
}

# Two short shas agree when one is a prefix of the other. `--short` lengthens
# as a repository grows, so an r1490 stamped at seven characters and a
# version.json written at eight are the same commit, and a plain string
# comparison would call for a redeploy every single time.
sha_agrees() {
    local a="$1" b="$2"
    [ -n "$a" ] && [ -n "$b" ] || return 1
    case "$a" in "$b"*) return 0 ;; esac
    case "$b" in "$a"*) return 0 ;; esac
    return 1
}

require_host() {
    [ -n "$EMBER_SHIP_HOST" ] || die "EMBER_SHIP_HOST is not set (see $CONF)"
    alias_ok "$EMBER_SHIP_HOST" || die "EMBER_SHIP_HOST='$EMBER_SHIP_HOST' is not an ssh alias"
}
require_builder() {
    [ -n "$EMBER_SHIP_BUILDER" ] || die "EMBER_SHIP_BUILDER is not set (see $CONF)"
    alias_ok "$EMBER_SHIP_BUILDER" || die "EMBER_SHIP_BUILDER='$EMBER_SHIP_BUILDER' is not an ssh alias"
}

# --- the build --------------------------------------------------------------
# One remote script, sent whole. It resolves the commit, stamps from THAT
# commit rather than from whatever the builder's working tree happens to be
# (docs/hosts.md §7), builds the six products, and prints its findings as
# `SHIP key=value` lines so a run wrapper's own output can be interleaved
# without confusing the caller.
#
# The arena's package name is read from the checkout, not fixed here: it was
# pong-server until it was renamed, and a published commit is allowed to be on
# the far side of that. The product that leaves this machine is called
# arena-server either way, which is what lets the host side hold fixed names.
build_remote_script() {
    cat <<REMOTE
set -euo pipefail
${EMBER_SHIP_BUILD_ENV}
BUILD_DIR="${EMBER_SHIP_BUILD_DIR}"
STAGE="${EMBER_SHIP_BUILDER_STAGE}"
COMMIT="$1"
mkdir -p "\$(dirname "\$BUILD_DIR")"
if [ -d "\$BUILD_DIR/.git" ]; then
    git -C "\$BUILD_DIR" remote set-url origin "${EMBER_SHIP_REPO}"
    git -C "\$BUILD_DIR" fetch -q --tags --prune origin
else
    git clone -q "${EMBER_SHIP_REPO}" "\$BUILD_DIR"
fi
cd "\$BUILD_DIR"
git rev-parse --verify -q "\$COMMIT^{commit}" >/dev/null \
    || { echo "ship: the builder does not have commit \$COMMIT" >&2; exit 1; }
git checkout -q --detach "\$COMMIT"
VERSION="r\$(git rev-list --count HEAD)"
SHORT="\$(git rev-parse --short HEAD)"
FULL="\$(git rev-parse HEAD)"
TD="\${CARGO_TARGET_DIR:-\$PWD/target}"
WRAP="${EMBER_SHIP_BUILD_WRAP}"
if [ -z "\$WRAP" ] && command -v chrt >/dev/null 2>&1 && command -v ionice >/dev/null 2>&1; then
    WRAP="chrt --idle 0 ionice -c3"
fi
if [ -d crates/arena-core ]; then ARENA=arena-server; else ARENA=pong-server; fi
export EMBER_BUILD_VERSION="\$VERSION" EMBER_BUILD_COMMIT="\$SHORT"
\$WRAP cargo build --release -p "\$ARENA" -p fire-server -p kings-server
\$WRAP cargo build --release -p "\$ARENA" --example wsbot
\$WRAP cargo build --release -p fire-server --example probe
cp "\$TD/release/examples/probe" "\$TD/release/examples/fire-probe"
\$WRAP cargo build --release -p kings-server --example probe
cp "\$TD/release/examples/probe" "\$TD/release/examples/kings-probe"
rm -rf "\$STAGE"
mkdir -p "\$STAGE"
cp "\$TD/release/\$ARENA" "\$STAGE/arena-server"
cp "\$TD/release/fire-server" "\$STAGE/fire-server"
cp "\$TD/release/kings-server" "\$STAGE/kings-server"
cp "\$TD/release/examples/wsbot" "\$STAGE/wsbot"
cp "\$TD/release/examples/fire-probe" "\$STAGE/fire-probe"
cp "\$TD/release/examples/kings-probe" "\$STAGE/kings-probe"
chmod 0755 "\$STAGE"/*
proto() {
    grep -oE 'PROTO_VERSION: u16 = [0-9]+' "crates/\$1/src/proto.rs" | grep -oE '[0-9]+\$' | head -1
}
if [ -d crates/arena-core ]; then ARENA_CRATE=arena-core; else ARENA_CRATE=pong-core; fi
# The RESOLVED staging path, because scp speaks SFTP and does not expand a
# remote shell's variables: a stage configured as \$WORKSPACE/... is a real
# directory on the builder and a literal seven characters to the copy.
echo "SHIP stage=\$(cd "\$STAGE" && pwd)"
echo "SHIP version=\$VERSION"
echo "SHIP commit=\$SHORT"
echo "SHIP full_commit=\$FULL"
echo "SHIP arena_proto=\$(proto "\$ARENA_CRATE")"
echo "SHIP fire_proto=\$(proto fire-core)"
echo "SHIP kings_proto=\$(proto kings-core)"
REMOTE
}

# --- deploy -----------------------------------------------------------------
cmd_deploy() {
    local t0; t0="$(date +%s)"
    require_host
    require_builder

    say "reading $EMBER_SHIP_VERSION_URL"
    local pub; pub="$(published_json)" || unknown "could not read the published version.json"
    local pub_commit pub_version
    pub_commit="$(published_field "$pub" commit)"
    pub_version="$(published_field "$pub" version)"
    [ -n "$pub_commit" ] || unknown "version.json carries no commit"
    echo "   the pages publish $pub_version · $pub_commit"

    say "building $pub_commit on $EMBER_SHIP_BUILDER"
    local tb; tb="$(date +%s)"
    local out
    out="$(build_remote_script "$pub_commit" | builder_ssh 'bash -s')" \
        || die "the build on $EMBER_SHIP_BUILDER failed"
    printf '%s\n' "$out"
    echo "   built in $(( $(date +%s) - tb ))s"

    local version commit full arena_proto fire_proto kings_proto stage_path
    ship_field() { printf '%s' "$out" | grep -E "^SHIP $1=" | head -1 | sed "s/^SHIP $1=//"; }
    stage_path="$(ship_field stage)"
    version="$(ship_field version)"
    commit="$(ship_field commit)"
    full="$(ship_field full_commit)"
    arena_proto="$(ship_field arena_proto)"
    fire_proto="$(ship_field fire_proto)"
    kings_proto="$(ship_field kings_proto)"
    for field in stage_path version commit full arena_proto fire_proto kings_proto; do
        [ -n "${!field}" ] || die "the builder did not report $field"
    done
    # The stamp must name the commit the PAGES name. A builder that resolved
    # something else — a stale checkout, a ref that moved under it — would put
    # a build in front of players that the live pages were not made from.
    sha_agrees "$commit" "$pub_commit" \
        || die "the builder stamped $commit but version.json names $pub_commit"

    say "collecting the products"
    local stage="$EMBER_SHIP_STAGE"
    rm -rf "$stage"
    mkdir -p "$stage"
    local tc; tc="$(date +%s)"
    builder_scp -r "$EMBER_SHIP_BUILDER:$stage_path" "$stage/products" \
        || die "could not copy the products off $EMBER_SHIP_BUILDER"
    {
        echo "version=$version"
        echo "commit=$commit"
        echo "full_commit=$full"
        echo "arena_proto=$arena_proto"
        echo "fire_proto=$fire_proto"
        echo "kings_proto=$kings_proto"
        echo "built=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
        echo "builder=$EMBER_SHIP_BUILDER"
    } > "$stage/products/stamp"
    chmod 0755 "$stage/products/"*-server "$stage/products/wsbot" \
        "$stage/products"/*-probe
    echo "   collected in $(( $(date +%s) - tc ))s"
    ls -l "$stage/products"

    # A fresh directory per commit, rather than overwriting in place, and the
    # upload goes to a sibling that is then renamed over it. Both halves are
    # needed. A running binary cannot be written to at all (ETXTBSY), which is
    # exactly what re-shipping the SAME commit onto a live host does; and a
    # rename only replaces the directory entry, so the processes still holding
    # the old files keep running until host.sh stops them. Unlinking those
    # files afterwards is safe for the same reason.
    local root="$EMBER_SHIP_REMOTE_ROOT" dest="$EMBER_SHIP_REMOTE_ROOT/$commit"
    local incoming="$dest.incoming"
    say "shipping to $EMBER_SHIP_HOST:$dest"
    local ts; ts="$(date +%s)"
    host_ssh "rm -rf '$incoming' && mkdir -p '$incoming' '$root/deploy'" \
        || unknown "cannot reach $EMBER_SHIP_HOST"
    host_scp "$stage/products"/* "$EMBER_SHIP_HOST:$incoming/" \
        || die "could not copy the products to $EMBER_SHIP_HOST"
    host_scp "$SELF_DIR"/*.sh "$SELF_DIR"/*.py "$EMBER_SHIP_HOST:$root/deploy/" \
        || die "could not copy deploy/ to $EMBER_SHIP_HOST"
    host_ssh "rm -rf '$dest.old'; if [ -d '$dest' ]; then mv '$dest' '$dest.old'; fi; mv '$incoming' '$dest'; rm -rf '$dest.old'" \
        || die "could not move the shipped products into place on $EMBER_SHIP_HOST"
    echo "   shipped in $(( $(date +%s) - ts ))s"

    # The name is a property of the MACHINE, not of one invocation. host.sh
    # takes EMBER_HOST_NAME, else ~/.ember/host-name, else generates one and
    # keeps it — so a configured name passed only on the command line left the
    # box with no memory of it, and the next bare `host.sh status` or
    # republish generated a different name and would have listed the same
    # machine a second time under it. Recording it here is what makes every
    # later run on that host agree with this one.
    if [ -n "$EMBER_SHIP_HOST_NAME" ]; then
        [[ "$EMBER_SHIP_HOST_NAME" =~ ^[a-z0-9-]{3,32}$ ]] \
            || die "EMBER_SHIP_HOST_NAME='$EMBER_SHIP_HOST_NAME' is not a host name"
        say "recording the host name"
        host_ssh "mkdir -p \"\$HOME/.ember\" && printf '%s\\n' '$EMBER_SHIP_HOST_NAME' > \"\$HOME/.ember/host-name\"" \
            || die "could not record the host name on $EMBER_SHIP_HOST"
    fi

    say "bootstrapping $EMBER_SHIP_HOST"
    # Double quotes, so the REMOTE shell expands $HOME. Single ones travel as
    # seven literal characters and name no directory on any machine.
    host_ssh "EMBER_PREBUILT=\"\$HOME/$dest\" bash \"\$HOME/$root/deploy/bootstrap-host.sh\"" \
        || die "bootstrap failed on $EMBER_SHIP_HOST"

    say "host.sh up on $EMBER_SHIP_HOST"
    local name_env=""
    [ -z "$EMBER_SHIP_HOST_NAME" ] || name_env="EMBER_HOST_NAME='$EMBER_SHIP_HOST_NAME' "
    host_ssh "${name_env}EMBER_PREBUILT=\"\$HOME/$dest\" EMBER_PUBLISH='$EMBER_SHIP_PUBLISH' bash \"\$HOME/$root/deploy/host.sh\" up" \
        || die "host.sh up failed on $EMBER_SHIP_HOST"

    # Keep the three newest product directories. Older ones are the previous
    # builds; one of them is what a rollback would use, and the rest are disk.
    host_ssh "cd \"\$HOME/$root\" && ls -1dt */ 2>/dev/null | grep -v '^deploy/\$' | tail -n +4 | while read -r d; do rm -rf \"\$HOME/$root/\${d%/}\"; done" \
        || echo "ship-host.sh: could not prune old product directories" >&2

    say "the host's own entry"
    host_scp "$EMBER_SHIP_HOST:ember-host/run/host.json" "$stage/host.json" \
        || die "could not fetch run/host.json from $EMBER_SHIP_HOST"
    cat "$stage/host.json"
    echo
    say "SHIPPED $version · $commit in $(( $(date +%s) - t0 ))s"
    echo "Publishing the entry into the book is a separate, credentialed step:"
    echo "  bash deploy/republish-host.sh $EMBER_SHIP_HOST --repo <address-book repo> --branch <branch>"
}

# --- check ------------------------------------------------------------------
# Read-only, and it probes ON the host rather than from here: the products are
# already there, so the question needs no binary on this machine and no compute
# anywhere. It never touches a pid file, and never a pattern kill.
cmd_check() {
    require_host
    local pub; pub="$(published_json)" || { echo "check: the published version.json is unreadable" >&2; exit 4; }
    local pub_commit; pub_commit="$(published_field "$pub" commit)"
    [ -n "$pub_commit" ] || { echo "check: version.json carries no commit" >&2; exit 4; }

    local entry
    entry="$(host_ssh 'cat "$HOME/ember-host/run/host.json"' 2>/dev/null)" || {
        echo "check: $EMBER_SHIP_HOST has no run/host.json to read" >&2
        exit 3
    }

    local running; running="$(published_field "$entry" commit)"
    if ! sha_agrees "$running" "$pub_commit"; then
        echo "check: the host runs '${running:-nothing}' and the pages publish '$pub_commit'"
        exit 3
    fi

    # game<TAB>url, one per line, straight out of the entry the host wrote.
    local rows
    rows="$(printf '%s' "$entry" | "$PY" -c '
import json, re, sys
entry = json.load(sys.stdin)
for key, url in sorted(entry.items()):
    game = "arena" if key == "ws" else (key[:-3] if key.endswith("_ws") else None)
    if game is None or not isinstance(url, str) or not re.match(r"^wss?://\S+$", url):
        continue
    print(game + "\t" + url)
')" || { echo "check: could not read the host entry" >&2; exit 4; }
    [ -n "$rows" ] || { echo "check: the host entry advertises no games" >&2; exit 3; }

    local dir="$EMBER_SHIP_REMOTE_ROOT/$running" rc=0 game url
    while IFS=$'\t' read -r game url; do
        [ -n "$game" ] || continue
        local remote
        case "$game" in
            arena) remote="\"\$HOME/$dir/wsbot\" '$url' create ship-check - ship-check 6" ;;
            fire)  remote="\"\$HOME/$dir/fire-probe\" '$url'" ;;
            kings) remote="\"\$HOME/$dir/kings-probe\" '$url' --expect-commit '$running'" ;;
            *)     echo "check: no probe for '$game'; treating it as unanswered" >&2; rc=3; continue ;;
        esac
        if host_ssh_t 90 "$remote" >/dev/null 2>&1; then
            echo "check: $game answered through $url"
        else
            echo "check: $game did NOT answer through $url"
            rc=3
        fi
    done <<< "$rows"
    [ "$rc" -eq 0 ] || exit 3
    echo "check: $EMBER_SHIP_HOST runs the published $pub_commit and all games answered"
}

case "$CMD" in
    deploy) cmd_deploy ;;
    check)  cmd_check ;;
    *)      usage ;;
esac
