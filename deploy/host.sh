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
# Run deploy/bootstrap-host.sh first on a bare host. This script itself needs
# git, a Rust toolchain, cloudflared and python3. The first build takes minutes;
# later ones seconds.
#
# WHY THE ARENA AND FIRE NAMES GO THROUGH THE ENVIRONMENT. Those servers are
# started with EMBER_HOST_NAME rather than a `--name` flag, because a host may
# stay on an older commit (§7) whose binary never heard of the flag. Kings was
# introduced with `--name`, so its argv follows its standalone recipe.
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

# Loopback ports the three servers bind. The tunnels are what the world sees.
#EMBER_ARENA_PORT=7780
#EMBER_FIRE_PORT=7781
#EMBER_KINGS_PORT=7782

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
EMBER_KINGS_PORT="${EMBER_KINGS_PORT:-7782}"
EMBER_HOME="${EMBER_HOME:-$HOME/ember-host}"
DEFAULT_TUNNEL_BIN="$HOME/bin/cloudflared"
EMBER_TUNNEL_BIN="${EMBER_TUNNEL_BIN:-$DEFAULT_TUNNEL_BIN}"

SRC="$EMBER_HOME/src"
RUN="$EMBER_HOME/run"
LOGS="$EMBER_HOME/log"
mkdir -p "$RUN" "$LOGS"

# Found is not the same as working: on Windows `python3` and `python` are
# usually App Execution Aliases that resolve on PATH and are not interpreters,
# and the real interpreter can sit behind one of them, so the first candidate
# that RUNS wins rather than the first that resolves. `status` degrades
# gracefully on an empty $PY; `up` refuses up front rather than after a
# multi-minute build it could never publish.
PY=""
for py_candidate in python3 python; do
    py_found="$(command -v "$py_candidate" || true)"
    [ -n "$py_found" ] && "$py_found" -c '' >/dev/null 2>&1 || continue
    PY="$py_found"
    break
done

# Idle priority for every compile this script runs, so a host that is also
# somebody's desktop stays usable. Both tools are unprivileged; neither is
# guaranteed to exist. At file scope because the health probes need it too and
# a `local` in cmd_up was invisible to them.
NICE=""
if command -v chrt >/dev/null 2>&1 && command -v ionice >/dev/null 2>&1; then
    NICE="chrt --idle 0 ionice -c3"
fi

say() { echo "== $* =="; }
die() { echo "host.sh: $*" >&2; exit 1; }

# A helper script comes from next to this one when it is there (the normal
# case: this file is being run out of a checkout), and from the managed
# checkout otherwise (someone copied host.sh onto a box on its own).
helper() {
    if [ -f "$SELF_DIR/$1" ]; then echo "$SELF_DIR/$1"; else echo "$SRC/deploy/$1"; fi
}

# --- the three games -------------------------------------------------------
# id, crate the protocol number lives in, binary, and how that binary wants
# its bind address. The three servers disagree about the flag, which is exactly
# the kind of thing that must be stated once rather than remembered twice.
#
# The arena's names are read from the CHECKOUT rather than fixed here. It was
# called pong until it was renamed, and EMBER_REF is allowed to name a commit
# older than that (§7) — every published arena build up to v11 is on the far
# side of it. A fixed table meant such a ref could not be built at all:
# `cargo build -p arena-server` in a tree whose package is `pong-server` dies
# with "package ID specification did not match any packages". Fire and Kings
# were never renamed, so their names stay literal.
game_ids() { echo "arena fire kings"; }
game_port() {
    case "$1" in
        arena) echo "$EMBER_ARENA_PORT" ;;
        fire) echo "$EMBER_FIRE_PORT" ;;
        kings) echo "$EMBER_KINGS_PORT" ;;
    esac
}
arena_is_renamed() { [ -d "$SRC/crates/arena-core" ]; }
game_crate() {
    case "$1" in
        arena) if arena_is_renamed; then echo arena-core; else echo pong-core; fi ;;
        fire)  echo fire-core ;;
        kings) echo kings-core ;;
    esac
}
game_pkg() {
    case "$1" in
        arena) if arena_is_renamed; then echo arena-server; else echo pong-server; fi ;;
        fire)  echo fire-server ;;
        kings) echo kings-server ;;
    esac
}
game_bin() { game_pkg "$1"; }
game_bind()  {
    case "$1" in
        arena) echo "--bind 127.0.0.1:$(game_port arena)" ;;
        fire)  echo "127.0.0.1:$(game_port fire)" ;;
        kings) echo "127.0.0.1:$(game_port kings)" ;;
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
#
# $3 is a label that makes the arena's lobby name unique per probe. The
# loopback and the public probe hit the SAME server seconds apart, and a
# second `create` under a name the first one is still holding fails for a
# reason that has nothing to do with the server's health.
#
# The probes EXEC the example binaries rather than going through `cargo run`.
# `cargo run` here was a second, unniced, untimed build: the build step below
# builds no examples and no dev-dependencies, so the first probe on a cold
# target directory compiled wsbot, rustls and tungstenite at normal scheduling
# and I/O priority, on a box the `chrt --idle` above exists to protect, and
# none of that time appeared in the "built in Ns" figure. It also ran with
# EMBER_BUILD_VERSION unset, which build.rs declares a rerun trigger, so every
# `up` recompiled the server libs unstamped and the next one recompiled them
# stamped again. And a probe that failed to COMPILE was reported as "the server
# did not answer".
probe_game() {
    local id="$1" url="$2" label="${3:-check}" expect_commit="${4:-}" bin
    case "$id" in
        arena)
            bin="$(target_dir)/release/examples/wsbot"
            [ -x "$bin" ] || die "$bin was not built"
            "$bin" "$url" create "health-$label" - "health-$label" 6 >/dev/null
            ;;
        fire)
            bin="$(target_dir)/release/examples/fire-probe"
            [ -x "$bin" ] || die "$bin was not built"
            "$bin" "$url" >/dev/null
            ;;
        kings)
            bin="$(target_dir)/release/examples/kings-probe"
            [ -x "$bin" ] || die "$bin was not built"
            [ -n "$expect_commit" ] || die "the kings probe needs the deployed commit"
            "$bin" "$url" --expect-commit "$expect_commit" >/dev/null
            ;;
    esac
}

# --- process control -------------------------------------------------------
# A pid file records the pid AND that process's start time — field 22 of
# /proc/<pid>/stat, in clock ticks since boot. A bare pid is not an identity:
# nothing clears these files across a reboot, and pids are handed out again
# from the bottom afterwards, so `update` found a stranger holding the recorded
# number, reported "both servers are running; nothing to do", and left the host
# offline for good — while `down` sent that stranger SIGTERM and then SIGKILL.
# The start time is not reused with the pid and resets on boot, so it settles
# reuse and reboot together, with no boot-id file.
#
# Two degradations, both deliberate. A system with no /proc records `-`, which
# means "cannot be verified here" and falls back to the bare `kill -0` this
# replaces. A file with no second field at all was written by an older host.sh
# and is read as NOT alive — which errs towards redeploying a host, never
# towards signalling a stranger.
start_time_of() { awk '{print $22}' "/proc/$1/stat" 2>/dev/null || true; }

record_pid() {
    local label="$1" pid="$2" st
    st="$(start_time_of "$pid")"
    echo "$pid ${st:--}" > "$RUN/$label.pid"
}

pid_of() {
    if [ -f "$RUN/$1.pid" ]; then cut -d' ' -f1 "$RUN/$1.pid"; fi
}
stamp_of() {
    if [ -f "$RUN/$1.pid" ]; then cut -d' ' -s -f2 "$RUN/$1.pid"; fi
}

alive() {
    local pid stamp now
    pid="$(pid_of "$1")"
    [ -n "$pid" ] || return 1
    kill -0 "$pid" 2>/dev/null || return 1
    stamp="$(stamp_of "$1")"
    [ -n "$stamp" ] || return 1
    if [ "$stamp" = "-" ]; then
        return 0
    fi
    now="$(start_time_of "$pid")"
    [ -n "$now" ] && [ "$now" = "$stamp" ]
}

stop_one() {
    local label="$1" pid
    pid="$(pid_of "$label")"
    if [ -z "$pid" ] || ! alive "$label"; then
        # Gone, or the number now belongs to something that is not ours.
        rm -f "$RUN/$label.pid"
        return 0
    fi
    kill "$pid" 2>/dev/null || true
    for _ in $(seq 1 25); do
        if ! kill -0 "$pid" 2>/dev/null; then
            echo "   stopped $label ($pid)"
            rm -f "$RUN/$label.pid"
            return 0
        fi
        sleep 0.2
    done
    kill -9 "$pid" 2>/dev/null || true
    echo "   killed $label ($pid)"
    # Removed LAST, on every path: it used to go first, so an interrupt between
    # the removal and the kill orphaned a live server with no record of it.
    rm -f "$RUN/$label.pid"
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
    local id="$1" log url
    log="$RUN/tunnel-$id.log"
    for _ in $(seq 1 60); do
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
            # Print the local entry and change nothing elsewhere. It already
            # went through publish-host.sh, so this is exactly what a writer
            # can fetch with republish-host.sh.
            echo "EMBER_PUBLISH=none, so this entry was NOT published:"
            cat "$RUN/host.json"
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

# Always leave the proven, current entry on the host itself. A workstation
# with the upstream key can fetch this file and republish it; the host never
# needs that key. Kept separate from publish_entry so an unchanged `update`
# can refresh the file without pushing or restarting anything.
write_local_entry() {
    local name="$1"; shift
    bash "$(helper publish-host.sh)" --book "$RUN/host.json" --file host.json \
        --name "$name" "$@" >/dev/null
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
    # Every path out of `up` ends in publish-host.sh, including EMBER_PUBLISH=none.
    # Refuse now rather than after the build, the start and both health checks.
    [ -n "$PY" ] || die "need a working python3 (or python) on PATH to publish the entry"

    local name; name="$(bash "$(helper host-name.sh)")"
    say "host $name"

    sync_source
    local rev; rev="$(resolve_ref)" || die "EMBER_REF='$EMBER_REF' names no commit in $EMBER_REPO"
    git -C "$SRC" checkout -q --detach "$rev"
    local version commit
    version="r$(git -C "$SRC" rev-list --count HEAD)"
    commit="$(git -C "$SRC" rev-parse --short HEAD)"
    say "building $version · $commit from $EMBER_REF"

    local tb; tb="$(date +%s)"
    (
        cd "$SRC"
        export EMBER_BUILD_VERSION="$version" EMBER_BUILD_COMMIT="$commit"
        # The health probes are built HERE, in the same niced and timed step,
        # rather than compiled by `cargo run` on first use. One invocation per
        # package: a single multi-package `--example` line has to resolve the
        # target name across packages and is easy to get subtly wrong.
        # shellcheck disable=SC2086
        $NICE cargo build --release -p "$(game_pkg arena)" -p "$(game_pkg fire)" -p "$(game_pkg kings)"
        # shellcheck disable=SC2086
        $NICE cargo build --release -p "$(game_pkg arena)" --example wsbot
        # shellcheck disable=SC2086
        $NICE cargo build --release -p "$(game_pkg fire)" --example probe
        cp "$(target_dir)/release/examples/probe" "$(target_dir)/release/examples/fire-probe"
        # shellcheck disable=SC2086
        $NICE cargo build --release -p "$(game_pkg kings)" --example probe
        cp "$(target_dir)/release/examples/probe" "$(target_dir)/release/examples/kings-probe"
    ) || die "build failed"
    echo "   built in $(( $(date +%s) - tb ))s"

    say "stopping whatever was running"
    stop_all

    local id port bin bind ts
    ts="$(date +%s)"
    for id in $(game_ids); do
        port="$(game_port "$id")"
        bin="$(target_dir)/release/$(game_bin "$id")"
        [ -x "$bin" ] || die "$bin was not built"
        bind="$(game_bind "$id")"
        say "starting $id on 127.0.0.1:$port"
        if [ "$id" = kings ]; then
            # Kings was born with this flag; its standalone recipe uses it and
            # the explicit argv is the clearest identity evidence in a ps row.
            # shellcheck disable=SC2086
            RUST_LOG=info nohup "$bin" $bind --name "$name" \
                >> "$LOGS/$id-server.log" 2>&1 &
        else
            # EMBER_HOST_NAME, never --name: see the note at the top of the
            # file. This keeps every historical arena/fire ref runnable.
            # shellcheck disable=SC2086
            EMBER_HOST_NAME="$name" RUST_LOG=info \
                nohup "$bin" $bind >> "$LOGS/$id-server.log" 2>&1 &
        fi
        record_pid "server-$id" "$!"
        sleep 1
        alive "server-$id" || { tail -20 "$LOGS/$id-server.log" >&2; die "$id server did not stay up"; }
    done
    echo "   servers started in $(( $(date +%s) - ts ))s"

    local tl; tl="$(date +%s)"
    for id in $(game_ids); do
        port="$(game_port "$id")"
        say "local health check for $id"
        # Loopback first, so a failure at the public URL below is unambiguously
        # the tunnel rather than the server.
        probe_game "$id" "ws://127.0.0.1:$port" local "$commit" \
            || die "$id server is listening but did not answer"
    done
    echo "   local probes passed in $(( $(date +%s) - tl ))s"

    local urls="" tt
    tt="$(date +%s)"
    for id in $(game_ids); do
        port="$(game_port "$id")"
        say "starting the $id tunnel"
        : > "$RUN/tunnel-$id.log"
        nohup "$EMBER_TUNNEL_BIN" tunnel --url "http://127.0.0.1:$port" --no-autoupdate \
            >> "$RUN/tunnel-$id.log" 2>&1 &
        record_pid "tunnel-$id" "$!"
    done
    for id in $(game_ids); do
        local url; url="$(wait_for_tunnel "$id")" || die "no public address for $id"
        echo "$url" > "$RUN/$id.url"
        echo "   $id: $url"
        urls="$urls $id=$url"
    done
    echo "   tunnels started in $(( $(date +%s) - tt ))s"

    # Let the hostnames exist before anything asks for them. The first DNS
    # query for a brand-new *.trycloudflare.com name can land before
    # Cloudflare has published the record, and a resolver that caches that
    # NXDOMAIN keeps returning it long after the record appears — so the
    # first attempt waits, and the rest retry for up to two minutes. The
    # ssh deploys learned this the hard way on 2026-09-01; this script
    # learned it on its first real run on 2026-09-02, when it probed the
    # arena's fresh name within a second of minting it and gave up.
    # Only a real cloudflared mints names that need this; the test stub's
    # loopback addresses resolve at once. EMBER_TUNNEL_SETTLE overrides.
    local settle="${EMBER_TUNNEL_SETTLE:-}" tp
    tp="$(date +%s)"
    if [ -z "$settle" ]; then
        case "$(basename "$EMBER_TUNNEL_BIN")" in cloudflared) settle=15 ;; *) settle=0 ;; esac
    fi
    if [ "$settle" != "0" ]; then
        say "letting the tunnel hostnames propagate (${settle}s)"
        sleep "$settle"
    fi
    for id in $(game_ids); do
        local url; url="$(cat "$RUN/$id.url")"
        say "health check for $id through $url"
        local ok="" attempt
        for attempt in $(seq 1 24); do
            if probe_game "$id" "$url" public "$commit"; then ok=1; break; fi
            [ "$attempt" -lt 24 ] && sleep 5
        done
        [ -n "$ok" ] || die "the $id tunnel is up but the server did not answer through it within two minutes"
        echo "   $id answered through its public address (attempt $attempt)"
    done
    echo "   public probes passed in $(( $(date +%s) - tp ))s"

    # WHAT IS RUNNING is recorded as soon as it is proven running — both
    # servers answered on loopback and through their public addresses — and
    # before the publish, which is a different question. It used to come after,
    # so a host with no push rights on EMBER_REPO (the documented normal case
    # for "anyone with a Linux box") aborted here with both games healthy and
    # `$RUN/deployed` never written; every later `update` then read "none",
    # took the redeploy branch, and its first act was stop_all — tearing down
    # two working servers mid-game, rebuilding, minting two new tunnel URLs and
    # failing to publish again. The operator never even saw the UP line.
    echo "$rev" > "$RUN/deployed"
    say "UP as $name ($version · $commit) in $(( $(date +%s) - t0 ))s:$urls"

    say "publishing"
    local args=() pubrc=0
    for id in $(game_ids); do
        args+=(--game "$id" --url "$(cat "$RUN/$id.url")" --proto "$(proto_of "$id")")
    done
    write_local_entry "$name" "${args[@]}" \
        --version "$version" --commit "$commit" \
        --by "$(id -un)@$(hostname 2>/dev/null || uname -n)"
    # Loud and non-zero, but no longer fatal to the record above: a host absent
    # from the book IS a failure and a wrapper must see it, while `update` must
    # stop mistaking an unpublished host for an undeployed one.
    publish_entry "$name" "${args[@]}" \
        --version "$version" --commit "$commit" \
        --by "$(id -un)@$(hostname 2>/dev/null || uname -n)" || pubrc=$?
    if [ "$pubrc" -ne 0 ]; then
        echo "host.sh: the servers are UP at$urls but publishing FAILED;" >&2
        echo "         players will not find this host until it publishes." >&2
        echo "         Fix EMBER_PUBLISH (currently '$EMBER_PUBLISH') and re-run." >&2
        return "$pubrc"
    fi
}

cmd_update() {
    sync_source
    local rev deployed
    rev="$(resolve_ref)" || die "EMBER_REF='$EMBER_REF' names no commit"
    deployed="$(cat "$RUN/deployed" 2>/dev/null || echo none)"
    local all_up=1 id
    for id in $(game_ids); do
        alive "server-$id" || all_up=""
    done
    if [ "$rev" = "$deployed" ] && [ -n "$all_up" ]; then
        local name version commit args=()
        name="$(bash "$(helper host-name.sh)")"
        version="r$(git -C "$SRC" rev-list --count HEAD)"
        commit="$(git -C "$SRC" rev-parse --short HEAD)"
        for id in $(game_ids); do
            args+=(--game "$id" --url "$(cat "$RUN/$id.url")" --proto "$(proto_of "$id")")
        done
        write_local_entry "$name" "${args[@]}" \
            --version "$version" --commit "$commit" \
            --by "$(id -un)@$(hostname 2>/dev/null || uname -n)"
        echo "up to date at ${rev:0:7} and all three servers are running; nothing to do"
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
