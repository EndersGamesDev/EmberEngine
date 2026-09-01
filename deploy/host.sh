#!/usr/bin/env bash
# Run an ember host on this machine (docs/hosts.md §8).
#
#   bash deploy/host.sh up        clone/update, build, start, prove, publish
#   bash deploy/host.sh update    rebuild and restart only if the ref moved
#   bash deploy/host.sh status    what is running, from what, at what address
#   bash deploy/host.sh down      stop the servers and their tunnels
#
# Everything is configured through ~/.ember/host.env, which this writes on
# first run with every setting commented out at its default. A real value in
# the environment always beats the file, so a one-off run needs no edit:
#
#   EMBER_REF=v12 bash deploy/host.sh up
#
# Needs git, a Rust toolchain, cloudflared, python3 and curl. The first build
# takes minutes; later ones seconds.
#
# WHY THE NAME GOES THROUGH THE ENVIRONMENT. The servers are started with
# EMBER_HOST_NAME in their environment rather than a `--name` flag, because a
# host is allowed to stay on an older commit (§7) and an older binary that has
# never heard of the flag would refuse to start. An unknown environment
# variable is ignored by every build there has ever been; an unknown flag is
# a crash loop.
set -euo pipefail

SELF_DIR="$(cd "$(dirname "$0")" && pwd)"
CMD="${1:-}"

# --- configuration ---------------------------------------------------------
CONF_DIR="${EMBER_CONF_DIR:-$HOME/.ember}"
CONF="$CONF_DIR/host.env"
mkdir -p "$CONF_DIR"
if [ ! -f "$CONF" ]; then
    cat > "$CONF" <<'ENV'
# ember host configuration. Uncomment and edit what you want to change; the
# environment overrides anything set here.

# Where the source comes from.
#EMBER_REPO=https://github.com/EndersGamesDev/EmberEngine.git
# Which commit this host runs. A host may stay on an older one on purpose:
# it keeps serving the frozen pages that speak its protocol.
#EMBER_REF=origin/main

# This host's name in the address book. Leave unset to let deploy/host-name.sh
# generate one and keep it in ~/.ember/host-name.
#EMBER_HOST_NAME=

# Where the entry is published:
#   none              print it and publish nothing (the default)
#   upstream          merge it into the project's own gh-pages (needs push rights)
#   <git url>#<branch>  write host.json there and list it as a mirror
#EMBER_PUBLISH=none

# Loopback ports the two servers bind. The tunnels are what the world sees.
#EMBER_ARENA_PORT=7780
#EMBER_FIRE_PORT=7781

# Working directory: source checkout, logs, pid files.
#EMBER_HOME=$HOME/ember-host

# The tunnel program. Any binary that prints its public https:// URL on
# startup will do; cloudflared is what the project uses.
#EMBER_TUNNEL_BIN=$HOME/bin/cloudflared
ENV
    echo "wrote $CONF (all defaults, nothing enabled)"
fi

# The file supplies defaults; a variable already exported into this process
# wins. Snapshot the real environment, source the file, put the snapshot back.
_PRE_ENV="$(export -p | grep -E '^(declare -x |export )EMBER_' || true)"
# shellcheck source=/dev/null
. "$CONF"
eval "$_PRE_ENV"

EMBER_REPO="${EMBER_REPO:-https://github.com/EndersGamesDev/EmberEngine.git}"
EMBER_REF="${EMBER_REF:-origin/main}"
EMBER_PUBLISH="${EMBER_PUBLISH:-none}"
EMBER_ARENA_PORT="${EMBER_ARENA_PORT:-7780}"
EMBER_FIRE_PORT="${EMBER_FIRE_PORT:-7781}"
EMBER_HOME="${EMBER_HOME:-$HOME/ember-host}"
DEFAULT_TUNNEL_BIN="$HOME/bin/cloudflared"
EMBER_TUNNEL_BIN="${EMBER_TUNNEL_BIN:-$DEFAULT_TUNNEL_BIN}"

SRC="$EMBER_HOME/src"
RUN="$EMBER_HOME/run"
LOGS="$EMBER_HOME/log"
mkdir -p "$RUN" "$LOGS"

PY="$(command -v python3 || command -v python || true)"

say() { echo "== $* =="; }
die() { echo "host.sh: $*" >&2; exit 1; }

# A helper script comes from next to this one when it is there (the normal
# case: this file is being run out of a checkout), and from the managed
# checkout otherwise (someone copied host.sh onto a box on its own).
helper() {
    if [ -f "$SELF_DIR/$1" ]; then echo "$SELF_DIR/$1"; else echo "$SRC/deploy/$1"; fi
}

# --- the two games ---------------------------------------------------------
# id, crate the protocol number lives in, binary, and how that binary wants
# its bind address. The two servers disagree about the flag, which is exactly
# the kind of thing that must be stated once rather than remembered twice.
game_ids() { echo "arena fire"; }
game_port()  { case "$1" in arena) echo "$EMBER_ARENA_PORT" ;; fire) echo "$EMBER_FIRE_PORT" ;; esac; }
game_crate() { case "$1" in arena) echo pong-core ;; fire) echo fire-core ;; esac; }
game_pkg()   { case "$1" in arena) echo pong-server ;; fire) echo fire-server ;; esac; }
game_bin()   { case "$1" in arena) echo pong-server ;; fire) echo fire-server ;; esac; }
game_bind()  {
    case "$1" in
        arena) echo "--bind 127.0.0.1:$(game_port arena)" ;;
        fire)  echo "127.0.0.1:$(game_port fire)" ;;
    esac
}

target_dir() { echo "${CARGO_TARGET_DIR:-$SRC/target}"; }

proto_of() {
    local crate; crate="$(game_crate "$1")"
    grep -oE 'PROTO_VERSION: u16 = [0-9]+' "$SRC/crates/$crate/src/proto.rs" \
        | grep -oE '[0-9]+$' | head -1
}

# Speak the protocol, do not settle for a handshake. A listener with a dead
# hub loop still completes a WebSocket upgrade, which is how a broken server
# once passed a deploy's health check.
probe_game() {
    local id="$1" url="$2"
    case "$id" in
        arena) ( cd "$SRC" && cargo run --release -q -p pong-server --example wsbot -- \
                    "$url" create healthcheck - healthcheck 6 >/dev/null ) ;;
        fire)  ( cd "$SRC" && cargo run --release -q -p fire-server --example probe -- \
                    "$url" >/dev/null ) ;;
    esac
}

# --- process control -------------------------------------------------------
pid_of() { [ -f "$RUN/$1.pid" ] && cat "$RUN/$1.pid" || true; }

alive() {
    local pid; pid="$(pid_of "$1")"
    [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null
}

stop_one() {
    local label="$1" pid i
    pid="$(pid_of "$label")"
    rm -f "$RUN/$label.pid"
    [ -n "$pid" ] || return 0
    kill -0 "$pid" 2>/dev/null || return 0
    kill "$pid" 2>/dev/null || true
    for i in $(seq 1 25); do
        kill -0 "$pid" 2>/dev/null || { echo "   stopped $label ($pid)"; return 0; }
        sleep 0.2
    done
    kill -9 "$pid" 2>/dev/null || true
    echo "   killed $label ($pid)"
}

stop_all() {
    local id
    for id in $(game_ids); do
        stop_one "tunnel-$id"
        stop_one "server-$id"
    done
}

# Read the tunnel's public address out of its log.
#
# cloudflared prints one https://<random>.trycloudflare.com line on startup and
# a fresh one on every restart, which is the whole reason this republishes.
# When EMBER_TUNNEL_BIN has been pointed somewhere else, any ws:// or wss://
# line it prints is taken verbatim instead — that is what lets the entire path,
# publish included, be exercised on loopback with no Cloudflare account.
wait_for_tunnel() {
    local id="$1" log="$RUN/tunnel-$id.log" i url
    for i in $(seq 1 60); do
        url="$(grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' "$log" 2>/dev/null | head -1 || true)"
        if [ -n "$url" ]; then echo "wss://${url#https://}"; return 0; fi
        if [ "$EMBER_TUNNEL_BIN" != "$DEFAULT_TUNNEL_BIN" ]; then
            url="$(grep -oE 'wss?://[^[:space:]]+' "$log" 2>/dev/null | head -1 || true)"
            if [ -n "$url" ]; then echo "$url"; return 0; fi
        fi
        alive "tunnel-$id" || { echo "host.sh: the $id tunnel exited; see $log" >&2; return 1; }
        sleep 1
    done
    echo "host.sh: no public address from the $id tunnel in 60s; see $log" >&2
    return 1
}

# --- the address book ------------------------------------------------------
# EMBER_PUBLISH decides where the entry goes, and nothing else in this script
# needs to know which of the three it was.
publish_entry() {
    local name="$1"; shift
    local pub="$EMBER_PUBLISH" args=()
    args=(--name "$name" "$@")
    case "$pub" in
        none|"")
            # Print it and change nothing. Still goes through publish-host.sh
            # so what is shown is exactly what would be published.
            local tmp; tmp="$(mktemp -d -t ember-entry-XXXXXX)"
            bash "$(helper publish-host.sh)" --book "$tmp/host.json" --file host.json "${args[@]}" >/dev/null
            echo "EMBER_PUBLISH=none, so this entry was NOT published:"
            cat "$tmp/host.json"
            rm -rf "$tmp"
            ;;
        upstream)
            bash "$(helper publish-host.sh)" --repo "$EMBER_REPO" --branch gh-pages \
                --file server.json "${args[@]}"
            ;;
        *#*)
            bash "$(helper publish-host.sh)" --repo "${pub%#*}" --branch "${pub##*#}" \
                --file host.json "${args[@]}"
            ;;
        *)
            die "EMBER_PUBLISH='$pub' is not none, upstream, or <git url>#<branch>"
            ;;
    esac
}

# Print the published file this host writes to, or nothing if it is out of
# reach. One shallow fetch of one branch; no checkout.
fetch_published() {
    local repo branch file tmp
    case "$EMBER_PUBLISH" in
        upstream) repo="$EMBER_REPO"; branch=gh-pages; file=server.json ;;
        *#*)      repo="${EMBER_PUBLISH%#*}"; branch="${EMBER_PUBLISH##*#}"; file=host.json ;;
        *)        return 1 ;;
    esac
    tmp="$(mktemp -d -t ember-book-XXXXXX)"
    (
        git init -q "$tmp"
        git -C "$tmp" remote add origin "$repo"
        git -C "$tmp" fetch -q --depth 1 origin "$branch"
        git -C "$tmp" show "FETCH_HEAD:$file"
    ) 2>/dev/null
    local rc=$?
    rm -rf "$tmp"
    return $rc
}

# --- resolve the ref -------------------------------------------------------
sync_source() {
    if [ -d "$SRC/.git" ]; then
        git -C "$SRC" remote set-url origin "$EMBER_REPO"
        git -C "$SRC" fetch -q --tags --prune origin
    else
        say "cloning $EMBER_REPO"
        mkdir -p "$(dirname "$SRC")"
        git clone -q "$EMBER_REPO" "$SRC"
    fi
}

# The commit EMBER_REF names right now.
#
# `origin/<ref>` is tried FIRST and the bare ref second, which looks backwards
# and is not: `git clone` leaves a local branch behind, `git fetch` never
# moves it, and resolving `main` to that stale local branch would make
# `update` report "up to date" forever while origin/main ran away. A sha or a
# tag has no origin/ form and falls through to the second attempt.
resolve_ref() {
    git -C "$SRC" rev-parse --verify -q "origin/${EMBER_REF}^{commit}" \
        || git -C "$SRC" rev-parse --verify -q "${EMBER_REF}^{commit}" \
        || return 1
}

# --- commands --------------------------------------------------------------
cmd_up() {
    local t0; t0="$(date +%s)"
    [ -x "$EMBER_TUNNEL_BIN" ] || die "no tunnel binary at $EMBER_TUNNEL_BIN (set EMBER_TUNNEL_BIN)"

    local name; name="$(bash "$(helper host-name.sh)")"
    say "host $name"

    sync_source
    local rev; rev="$(resolve_ref)" || die "EMBER_REF='$EMBER_REF' names no commit in $EMBER_REPO"
    git -C "$SRC" checkout -q --detach "$rev"
    local version commit
    version="r$(git -C "$SRC" rev-list --count HEAD)"
    commit="$(git -C "$SRC" rev-parse --short HEAD)"
    say "building $version · $commit from $EMBER_REF"

    # Idle priority so a host that is also somebody's desktop stays usable.
    # Both tools are unprivileged; neither is guaranteed to exist.
    local nice=""
    if command -v chrt >/dev/null 2>&1 && command -v ionice >/dev/null 2>&1; then
        nice="chrt --idle 0 ionice -c3"
    fi
    local tb; tb="$(date +%s)"
    (
        cd "$SRC"
        export EMBER_BUILD_VERSION="$version" EMBER_BUILD_COMMIT="$commit"
        # shellcheck disable=SC2086
        $nice cargo build --release -p pong-server -p fire-server
    ) || die "build failed"
    echo "   built in $(( $(date +%s) - tb ))s"

    say "stopping whatever was running"
    stop_all

    local id port bin bind
    for id in $(game_ids); do
        port="$(game_port "$id")"
        bin="$(target_dir)/release/$(game_bin "$id")"
        [ -x "$bin" ] || die "$bin was not built"
        bind="$(game_bind "$id")"
        say "starting $id on 127.0.0.1:$port"
        # EMBER_HOST_NAME, never --name: see the note at the top of the file.
        # shellcheck disable=SC2086
        EMBER_HOST_NAME="$name" RUST_LOG=info \
            nohup "$bin" $bind >> "$LOGS/$id-server.log" 2>&1 &
        echo $! > "$RUN/server-$id.pid"
        sleep 1
        alive "server-$id" || { tail -20 "$LOGS/$id-server.log" >&2; die "$id server did not stay up"; }
    done

    for id in $(game_ids); do
        port="$(game_port "$id")"
        say "local health check for $id"
        # Loopback first, so a failure at the public URL below is unambiguously
        # the tunnel rather than the server.
        probe_game "$id" "ws://127.0.0.1:$port" || die "$id server is listening but did not answer"
    done

    local urls=""
    for id in $(game_ids); do
        port="$(game_port "$id")"
        say "starting the $id tunnel"
        : > "$RUN/tunnel-$id.log"
        nohup "$EMBER_TUNNEL_BIN" tunnel --url "http://127.0.0.1:$port" --no-autoupdate \
            >> "$RUN/tunnel-$id.log" 2>&1 &
        echo $! > "$RUN/tunnel-$id.pid"
    done
    for id in $(game_ids); do
        local url; url="$(wait_for_tunnel "$id")" || die "no public address for $id"
        echo "$url" > "$RUN/$id.url"
        echo "   $id: $url"
        urls="$urls $id=$url"
    done

    for id in $(game_ids); do
        local url; url="$(cat "$RUN/$id.url")"
        say "health check for $id through $url"
        probe_game "$id" "$url" \
            || die "the $id tunnel is up but the server did not answer through it"
    done

    say "publishing"
    local args=()
    for id in $(game_ids); do
        args+=(--game "$id" --url "$(cat "$RUN/$id.url")" --proto "$(proto_of "$id")")
    done
    publish_entry "$name" "${args[@]}" \
        --version "$version" --commit "$commit" \
        --by "$(id -un)@$(hostname 2>/dev/null || uname -n)"

    echo "$rev" > "$RUN/deployed"
    say "UP as $name ($version · $commit) in $(( $(date +%s) - t0 ))s:$urls"
}

cmd_update() {
    sync_source
    local rev deployed
    rev="$(resolve_ref)" || die "EMBER_REF='$EMBER_REF' names no commit"
    deployed="$(cat "$RUN/deployed" 2>/dev/null || echo none)"
    if [ "$rev" = "$deployed" ] && alive server-arena && alive server-fire; then
        echo "up to date at ${rev:0:7} and both servers are running; nothing to do"
        return 0
    fi
    if [ "$rev" = "$deployed" ]; then
        say "ref has not moved but something is not running; bringing it back up"
    else
        say "${deployed:0:7} -> ${rev:0:7}; redeploying"
    fi
    cmd_up
}

cmd_status() {
    local name; name="$(bash "$(helper host-name.sh)")"
    echo "host:      $name"
    echo "home:      $EMBER_HOME"
    if [ -d "$SRC/.git" ]; then
        echo "ref:       $EMBER_REF -> $(git -C "$SRC" rev-parse --short HEAD) (r$(git -C "$SRC" rev-list --count HEAD))"
    else
        echo "ref:       $EMBER_REF (never checked out)"
    fi
    local id
    for id in $(game_ids); do
        if alive "server-$id"; then
            echo "$id server: running, pid $(pid_of "server-$id"), 127.0.0.1:$(game_port "$id")"
        else
            echo "$id server: DOWN"
        fi
        if alive "tunnel-$id"; then
            echo "$id tunnel: running, pid $(pid_of "tunnel-$id"), $(cat "$RUN/$id.url" 2>/dev/null || echo 'no address yet')"
        else
            echo "$id tunnel: DOWN"
        fi
    done
    echo "publish:   $EMBER_PUBLISH"
    local book
    if [ -z "$PY" ]; then
        echo "published: (no python here to read the book with)"
    elif book="$(fetch_published)"; then
        # Print this host's own entry, whether the file is the whole book or a
        # mirror holding one entry.
        printf '%s' "$book" | "$PY" -c '
import json, sys
name = sys.argv[1]
d = json.load(sys.stdin)
entries = d.get("hosts", [d]) if isinstance(d, dict) else []
mine = [e for e in entries if isinstance(e, dict) and e.get("name") == name]
if mine:
    print("published:", json.dumps(mine[0], sort_keys=True))
else:
    print("published: no entry named", name)
' "$name"
    else
        echo "published: the book is not reachable from here"
    fi
}

cmd_down() {
    say "stopping"
    stop_all
    echo "down"
}

case "$CMD" in
    up)     cmd_up ;;
    update) cmd_update ;;
    status) cmd_status ;;
    down)   cmd_down ;;
    *)      sed -n '2,20p' "$0" >&2; exit 2 ;;
esac
