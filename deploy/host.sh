#!/usr/bin/env bash
# Run an ember host on this machine (docs/hosts.md §8).
#
#   bash deploy/host.sh up        clone/update, build, start, prove, publish
#   bash deploy/host.sh update    rebuild and restart only if the ref moved
#   bash deploy/host.sh tunnels   mint what tunnels are missing, keep the rest, republish
#   bash deploy/host.sh status    what is running, from what, at what address
#   bash deploy/host.sh down      stop the servers and their tunnels
#
# Everything is configured through ~/.ember/host.env, which this writes on
# first run with every setting commented out at its default. A real value in
# the environment always beats the file, so a one-off run needs no edit:
#
#   EMBER_REF=main bash deploy/host.sh up
#
# Run deploy/bootstrap-host.sh first on a bare host. This script itself needs
# git, a Rust toolchain, cloudflared and python3. The first build takes minutes;
# later ones seconds.
#
# EMBER_PREBUILT=<dir> is the mode for a host that cannot build (docs/hosts.md
# §8). The eight products and a `stamp` file come from that directory, no
# repository is cloned and no compiler is required; everything downstream —
# the tunnel retention below, the loopback-then-public probe order, the pid
# files, run/host.json — is the same code. Unset, which is the default,
# nothing about this script changes.
#
# WHY THE ARENA AND FIRE NAMES GO THROUGH THE ENVIRONMENT. Those servers are
# started with EMBER_HOST_NAME rather than a `--name` flag, because a host may
# stay on an older commit (§7) whose binary never heard of the flag. Kings and
# League were both introduced with `--name`, so their argv follows their
# standalone recipes: no ref this host may sit on has a binary without it.
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
#EMBER_REF=main

# This host's name in the address book. Leave unset to let deploy/host-name.sh
# generate one and keep it in ~/.ember/host-name.
#EMBER_HOST_NAME=

# Where the entry is published:
#   none              print it and publish nothing (the default)
#   <git url>#<branch>  write host.json to a separate address-book mirror
#EMBER_PUBLISH=none

# Loopback ports the four servers bind. The tunnels are what the world sees.
#EMBER_ARENA_PORT=7780
#EMBER_FIRE_PORT=7781
#EMBER_KINGS_PORT=7782
#EMBER_LEAGUE_PORT=7783

# Run binaries somebody else built, instead of cloning and building. The
# directory holds arena-server, fire-server, kings-server, league-server,
# wsbot, fire-probe, kings-probe, league-probe and a `stamp` file naming the
# build. deploy/ship-host.sh fills it. Leave unset on a host that has a
# toolchain.
#EMBER_PREBUILT=

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
EMBER_REF="${EMBER_REF:-main}"
EMBER_PUBLISH="${EMBER_PUBLISH:-none}"
EMBER_ARENA_PORT="${EMBER_ARENA_PORT:-7780}"
EMBER_FIRE_PORT="${EMBER_FIRE_PORT:-7781}"
EMBER_KINGS_PORT="${EMBER_KINGS_PORT:-7782}"
EMBER_LEAGUE_PORT="${EMBER_LEAGUE_PORT:-7783}"
EMBER_HOME="${EMBER_HOME:-$HOME/ember-host}"
EMBER_PREBUILT="${EMBER_PREBUILT:-}"
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
    if [ -z "$py_found" ] || ! "$py_found" -c '' >/dev/null 2>&1; then
        continue
    fi
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

# --- prebuilt mode ---------------------------------------------------------
# A host that cannot build runs binaries somebody else built (docs/hosts.md
# §8). EMBER_PREBUILT names the directory holding them and the `stamp` file
# that says what they are. Everything downstream is the same code — tunnel
# retention, the loopback-then-public probe order, the pid identity, the
# published entry — because two implementations of "is this host healthy" is
# one more than can be kept true.
#
# The stamp is the ONLY statement of identity here: there is no checkout to
# ask, and a binary cannot be asked which commit produced it without being
# run. `update` compares `full_commit` rather than the short form, which is a
# display convenience that grows a digit as a repository does.
#
# The SHIPPED stamp and the RUNNING one are different questions, exactly as
# the checkout's HEAD and $RUN/deployed are on a host that builds: a shipper
# may replace the directory while the servers keep running the previous build.
# So the stamp is copied beside $RUN/deployed when a deploy is recorded, and
# `tunnels`, `update` and the protocol numbers read THAT copy — a tunnel
# repaired without a restart must republish what is answering, not what is
# waiting in the directory to be deployed.
prebuilt() { [ -n "$EMBER_PREBUILT" ]; }

PREBUILT_PRODUCTS="arena-server fire-server kings-server league-server wsbot fire-probe kings-probe league-probe"
DEPLOYED_STAMP="$RUN/deployed-stamp"

field_of() {  # <file> <key>
    local line
    line="$(grep -E "^$2=" "$1" 2>/dev/null | head -1 || true)"
    echo "${line#*=}"
}
stamp_field() { field_of "$EMBER_PREBUILT/stamp" "$1"; }
# What is RUNNING, falling back to the directory when nothing is recorded yet.
running_field() {
    if [ -s "$DEPLOYED_STAMP" ]; then field_of "$DEPLOYED_STAMP" "$1"; else stamp_field "$1"; fi
}

# Refuse an incomplete directory up front. A product missing at the moment it
# is needed would be found after the servers were stopped, with the host
# already down and nothing to bring back up.
require_prebuilt() {
    local product field
    [ -d "$EMBER_PREBUILT" ] || die "EMBER_PREBUILT='$EMBER_PREBUILT' is not a directory"
    [ -f "$EMBER_PREBUILT/stamp" ] || die "no stamp file in $EMBER_PREBUILT"
    for product in $PREBUILT_PRODUCTS; do
        [ -x "$EMBER_PREBUILT/$product" ] \
            || die "$EMBER_PREBUILT/$product is missing or not executable"
    done
    for field in version commit full_commit arena_proto fire_proto kings_proto league_proto; do
        [ -n "$(stamp_field "$field")" ] || die "the stamp in $EMBER_PREBUILT has no $field"
    done
}

# --- the four games --------------------------------------------------------
# id, crate the protocol number lives in, binary, and how that binary wants
# its bind address. The servers disagree about the flag, which is exactly
# the kind of thing that must be stated once rather than remembered twice.
#
# The arena's names are read from the CHECKOUT rather than fixed here. It was
# called pong until it was renamed, and EMBER_REF is allowed to name a commit
# older than that (§7) — every published arena build up to v11 is on the far
# side of it. A fixed table meant such a ref could not be built at all:
# `cargo build -p arena-server` in a tree whose package is `pong-server` dies
# with "package ID specification did not match any packages". Fire, Kings and
# League were never renamed, so their names stay literal.
game_ids() { echo "arena fire kings league"; }
game_port() {
    case "$1" in
        arena) echo "$EMBER_ARENA_PORT" ;;
        fire) echo "$EMBER_FIRE_PORT" ;;
        kings) echo "$EMBER_KINGS_PORT" ;;
        league) echo "$EMBER_LEAGUE_PORT" ;;
    esac
}
arena_is_renamed() { [ -d "$SRC/crates/arena-core" ]; }
game_crate() {
    case "$1" in
        arena) if arena_is_renamed; then echo arena-core; else echo pong-core; fi ;;
        fire)  echo fire-core ;;
        kings) echo kings-core ;;
        league) echo league-core ;;
    esac
}
game_pkg() {
    case "$1" in
        arena) if arena_is_renamed; then echo arena-server; else echo pong-server; fi ;;
        fire)  echo fire-server ;;
        kings) echo kings-server ;;
        league) echo league-server ;;
    esac
}
game_bin() { game_pkg "$1"; }
game_bind()  {
    case "$1" in
        arena) echo "--bind 127.0.0.1:$(game_port arena)" ;;
        fire)  echo "127.0.0.1:$(game_port fire)" ;;
        kings) echo "127.0.0.1:$(game_port kings)" ;;
        league) echo "127.0.0.1:$(game_port league)" ;;
    esac
}

target_dir() { echo "${CARGO_TARGET_DIR:-$SRC/target}"; }

# Where the server and its probe actually live. In prebuilt mode the names are
# fixed — the shipper is what resolves a historical package name to
# `arena-server`, so this side needs no checkout to ask.
server_bin() {
    if prebuilt; then
        echo "$EMBER_PREBUILT/$1-server"
    else
        echo "$(target_dir)/release/$(game_bin "$1")"
    fi
}
probe_bin() {
    local name
    case "$1" in
        arena) name=wsbot ;;
        fire)  name=fire-probe ;;
        kings) name=kings-probe ;;
        league) name=league-probe ;;
    esac
    if prebuilt; then
        echo "$EMBER_PREBUILT/$name"
    else
        echo "$(target_dir)/release/examples/$name"
    fi
}

# The protocol of what is RUNNING. build_entry_args calls this from `tunnels`
# too, where no server was restarted, so answering from the shipped stamp
# would publish a number the live binary does not speak.
proto_of() {
    if prebuilt; then
        running_field "$1_proto"
        return 0
    fi
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
    bin="$(probe_bin "$id")"
    [ -x "$bin" ] || die "$bin was not built"
    case "$id" in
        arena)
            "$bin" "$url" create "health-$label" - "health-$label" 6 >/dev/null
            ;;
        fire)
            "$bin" "$url" >/dev/null
            ;;
        kings)
            [ -n "$expect_commit" ] || die "the kings probe needs the deployed commit"
            "$bin" "$url" --expect-commit "$expect_commit" >/dev/null
            ;;
        league)
            [ -n "$expect_commit" ] || die "the league probe needs the deployed commit"
            # The lobby name is positional, and the label is what keeps it
            # distinct per probe. Two probes that both FINISH can reuse a
            # name: wsprobe sends LeaveLobby before closing and the server
            # drops a lobby whose members are empty (league-server's
            # lib.rs:959). The hazard is a probe that did NOT finish —
            # probe_public retries the same `public` label up to 24 times,
            # so a killed or timed-out attempt leaves its lobby held and the
            # next attempt is refused a name for a reason that has nothing
            # to do with the server's health. A concurrent
            # `ship-host.sh check` is the same collision from another
            # process, which is why it uses `ship-check` and not these.
            "$bin" "$url" "health-$label" --expect-commit "$expect_commit" >/dev/null
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

# The servers only. A tunnel forwards a public name to a loopback port and
# does not care which process answers there, so a restart behind it keeps the
# address; see ensure_tunnels.
stop_servers() {
    local id
    for id in $(game_ids); do
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

# --- tunnels -----------------------------------------------------------------
# A quick tunnel is a process forwarding a public name to a loopback port.
# Restarting the server behind it needs no new tunnel: cloudflared keeps
# forwarding to the port, and the address book keeps its entry. So a live
# tunnel that still answers is KEPT across `up` and `update`, and only a dead
# or unanswering one is minted again. That is also what keeps a host off
# Cloudflare's rate limit for new quick tunnels (HTTP 429, error 1015): the
# first on-host timer re-minted three tunnels every two minutes and took the
# whole public IP - every host behind that router - out of quick tunnels for
# an afternoon (2026-09-06).
#
# EMBER_NO_MINT=1 forbids minting altogether (the timer sets it while backing
# off from a 429): dead tunnels are reported, servers are left alone.

mint_tunnel() {  # <id>: a fresh tunnel process for the game; the log starts over
    local id="$1" port; port="$(game_port "$id")"
    stop_one "tunnel-$id"
    rm -f "$RUN/$id.url"
    say "starting the $id tunnel"
    : > "$RUN/tunnel-$id.log"
    nohup "$EMBER_TUNNEL_BIN" tunnel --url "http://127.0.0.1:$port" --no-autoupdate \
        >> "$RUN/tunnel-$id.log" 2>&1 &
    record_pid "tunnel-$id" "$!"
}

# Let fresh hostnames exist before anything asks for them. The first DNS query
# for a brand-new *.trycloudflare.com name can land before Cloudflare has
# published the record, and a resolver that caches that NXDOMAIN keeps
# returning it long after the record appears (the FRITZ!Box: 20+ minutes).
# Only a real cloudflared mints names that need this; the test stub's loopback
# addresses resolve at once. EMBER_TUNNEL_SETTLE overrides.
settle_tunnels() {
    local settle="${EMBER_TUNNEL_SETTLE:-}"
    if [ -z "$settle" ]; then
        case "$(basename "$EMBER_TUNNEL_BIN")" in cloudflared) settle=15 ;; *) settle=0 ;; esac
    fi
    if [ "$settle" != "0" ]; then
        say "letting the tunnel hostnames propagate (${settle}s)"
        sleep "$settle"
    fi
}

probe_public() {  # <id> <url> <commit>: up to two minutes of retries
    local id="$1" url="$2" commit="$3" attempt
    for attempt in $(seq 1 24); do
        if probe_game "$id" "$url" public "$commit"; then
            echo "   $id answered through its public address (attempt $attempt)"
            return 0
        fi
        [ "$attempt" -lt 24 ] && sleep 5
    done
    return 1
}

# ensure_tunnels <commit>: on return every game has either a proven tunnel
# (its address in $RUN/<id>.url) or none, and TUNNELS_MISSING names the games
# without one. At most one mint per game per call. Returns 0 when all four
# are proven, 3 when at least one is missing.
TUNNELS_MISSING=""
ensure_tunnels() {
    local commit="$1" id url minted="" missing="" retry=""
    TUNNELS_MISSING=""
    for id in $(game_ids); do
        if alive "tunnel-$id" && [ -s "$RUN/$id.url" ]; then
            echo "   keeping the $id tunnel: $(cat "$RUN/$id.url")"
        elif [ -n "${EMBER_NO_MINT:-}" ]; then
            stop_one "tunnel-$id"
            rm -f "$RUN/$id.url"
            echo "   the $id tunnel is down and EMBER_NO_MINT is set; not minting one"
            missing="$missing $id"
        else
            mint_tunnel "$id"
            minted="$minted $id"
        fi
    done
    for id in $minted; do
        if url="$(wait_for_tunnel "$id")"; then
            echo "$url" > "$RUN/$id.url"
            echo "   $id: $url"
        else
            stop_one "tunnel-$id"
            missing="$missing $id"
        fi
    done
    [ -z "$minted" ] || settle_tunnels
    # Prove every tunnel that exists. A kept one that no longer answers is a
    # corpse - cloudflared can outlive its own tunnel - and is stopped; when
    # minting is allowed it gets one fresh attempt below.
    for id in $(game_ids); do
        case " $missing " in *" $id "*) continue ;; esac
        url="$(cat "$RUN/$id.url")"
        say "health check for $id through $url"
        if probe_public "$id" "$url" "$commit"; then continue; fi
        stop_one "tunnel-$id"
        rm -f "$RUN/$id.url"
        case " $minted " in
            *" $id "*)
                echo "   the $id tunnel is up but the server did not answer through it within two minutes"
                missing="$missing $id"
                continue ;;
        esac
        if [ -n "${EMBER_NO_MINT:-}" ]; then
            echo "   the $id tunnel no longer answers and EMBER_NO_MINT is set; not minting one"
            missing="$missing $id"
            continue
        fi
        echo "   the $id tunnel no longer answers; minting a new one"
        mint_tunnel "$id"
        retry="$retry $id"
    done
    if [ -n "$retry" ]; then
        for id in $retry; do
            if url="$(wait_for_tunnel "$id")"; then
                echo "$url" > "$RUN/$id.url"
                echo "   $id: $url"
            else
                stop_one "tunnel-$id"
                missing="$missing $id"
            fi
        done
        settle_tunnels
        for id in $retry; do
            case " $missing " in *" $id "*) continue ;; esac
            url="$(cat "$RUN/$id.url")"
            say "health check for $id through $url"
            if ! probe_public "$id" "$url" "$commit"; then
                stop_one "tunnel-$id"
                rm -f "$RUN/$id.url"
                echo "   the $id tunnel is up but the server did not answer through it within two minutes"
                missing="$missing $id"
            fi
        done
    fi
    if [ -n "$missing" ]; then
        TUNNELS_MISSING="${missing# }"
        echo "host.sh: no public address for:$missing" >&2
        if grep -qs -E '429|error code: 1015' "$RUN"/tunnel-*.log; then
            echo "host.sh: Cloudflare is rate-limiting new quick tunnels (429 / error 1015) for this public IP; retry later, not sooner" >&2
        fi
        return 3
    fi
    return 0
}

# The entry for what is running now: written locally always, published where
# EMBER_PUBLISH says. Games named after the commit have no proven tunnel and
# are DROPPED from the entry rather than left at a dead address - the book
# must never name an address that does not answer. Returns publish-host.sh's
# status.
ENTRY_ARGS=()
build_entry_args() {  # <version> <commit> [missing game ids...] -> ENTRY_ARGS
    local version="$1" commit="$2"; shift 2
    local missing="$*" id proven=""
    ENTRY_ARGS=()
    for id in $(game_ids); do
        case " $missing " in
            *" $id "*) ENTRY_ARGS+=(--drop-game "$id"); continue ;;
        esac
        # no address on disk is the same thing as no proven tunnel
        if [ ! -s "$RUN/$id.url" ]; then ENTRY_ARGS+=(--drop-game "$id"); continue; fi
        ENTRY_ARGS+=(--game "$id" --url "$(cat "$RUN/$id.url")" --proto "$(proto_of "$id")")
        proven="$proven $id"
    done
    if [ -n "$proven" ]; then
        # the build stamp applies to the games in the call, so only with one
        ENTRY_ARGS+=(--version "$version" --commit "$commit")
    fi
    ENTRY_ARGS+=(--by "$(id -un)@$(hostname 2>/dev/null || uname -n)")
}

publish_current() {  # <name> <version> <commit> [missing game ids...]
    local name="$1"; shift
    build_entry_args "$@"
    write_local_entry "$name" "${ENTRY_ARGS[@]}"
    publish_entry "$name" "${ENTRY_ARGS[@]}"
}

# --- the address book ------------------------------------------------------
# EMBER_PUBLISH decides where the entry goes, and nothing else in this script
# needs to know whether publication is disabled or uses a separate mirror.
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
        *#*)
            bash "$(helper publish-host.sh)" --repo "${pub%#*}" --branch "${pub##*#}" \
                --file host.json "${args[@]}"
            ;;
        *)
            die "EMBER_PUBLISH='$pub' is not none or <git url>#<branch>"
            ;;
    esac
}

# Always leave the proven, current entry on the host itself. A workstation
# with an address-book key can fetch this file and republish it; the host
# never needs that key. Kept separate from publish_entry so an unchanged
# `update` can refresh the file without pushing or restarting anything.
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
# Branch-shaped names resolve only under refs/remotes/origin after the pruning
# fetch. A missing remote branch therefore fails closed instead of selecting a
# stale local branch left by clone or an earlier checkout. Only an explicit
# refs/tags name or a full object id may resolve directly, because each names
# an immutable object rather than a branch that can disappear upstream.
resolve_ref() {
    local branch
    case "$EMBER_REF" in
        refs/tags/*)
            git -C "$SRC" rev-parse --verify -q "${EMBER_REF}^{commit}"
            ;;
        refs/heads/*)
            branch="${EMBER_REF#refs/heads/}"
            git -C "$SRC" rev-parse --verify -q "refs/remotes/origin/${branch}^{commit}"
            ;;
        refs/remotes/origin/*)
            branch="${EMBER_REF#refs/remotes/origin/}"
            git -C "$SRC" rev-parse --verify -q "refs/remotes/origin/${branch}^{commit}"
            ;;
        origin/*)
            branch="${EMBER_REF#origin/}"
            git -C "$SRC" rev-parse --verify -q "refs/remotes/origin/${branch}^{commit}"
            ;;
        *)
            if [[ "$EMBER_REF" =~ ^[0-9a-fA-F]{40}$ ]]; then
                git -C "$SRC" rev-parse --verify -q "${EMBER_REF}^{commit}"
            else
                git -C "$SRC" rev-parse --verify -q "refs/remotes/origin/${EMBER_REF}^{commit}"
            fi
            ;;
    esac
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

    local rev version commit
    if prebuilt; then
        # Nothing to fetch and nothing to compile: the identity of what will
        # run is a fact of the directory, checked before anything is stopped.
        require_prebuilt
        rev="$(stamp_field full_commit)"
        version="$(stamp_field version)"
        commit="$(stamp_field commit)"
        say "running the prebuilt $version · $commit from $EMBER_PREBUILT"
    else
        sync_source
        rev="$(resolve_ref)" || die "EMBER_REF='$EMBER_REF' names no commit in $EMBER_REPO"
        git -C "$SRC" checkout -q --detach "$rev"
        version="r$(git -C "$SRC" rev-list --count HEAD)"
        commit="$(git -C "$SRC" rev-parse --short HEAD)"
        say "building $version · $commit from $EMBER_REF"

        local tb; tb="$(date +%s)"
        (
            cd "$SRC"
            export EMBER_BUILD_VERSION="$version" EMBER_BUILD_COMMIT="$commit"
            # The health probes are built HERE, in the same niced and timed
            # step, rather than compiled by `cargo run` on first use. One
            # invocation per package: a single multi-package `--example` line
            # has to resolve the target name across packages and is easy to
            # get subtly wrong.
            # shellcheck disable=SC2086
            $NICE cargo build --release -p "$(game_pkg arena)" -p "$(game_pkg fire)" \
                -p "$(game_pkg kings)" -p "$(game_pkg league)"
            # shellcheck disable=SC2086
            $NICE cargo build --release -p "$(game_pkg arena)" --example wsbot
            # shellcheck disable=SC2086
            $NICE cargo build --release -p "$(game_pkg fire)" --example probe
            cp "$(target_dir)/release/examples/probe" "$(target_dir)/release/examples/fire-probe"
            # shellcheck disable=SC2086
            $NICE cargo build --release -p "$(game_pkg kings)" --example probe
            cp "$(target_dir)/release/examples/probe" "$(target_dir)/release/examples/kings-probe"
            # League's example is `wsprobe`, not `probe`: the copy still gives
            # it the per-game name the host side looks for, but the source
            # path is the one its own crate declares.
            # shellcheck disable=SC2086
            $NICE cargo build --release -p "$(game_pkg league)" --example wsprobe
            cp "$(target_dir)/release/examples/wsprobe" "$(target_dir)/release/examples/league-probe"
        ) || die "build failed"
        echo "   built in $(( $(date +%s) - tb ))s"
    fi

    say "stopping the servers (live tunnels are kept)"
    stop_servers

    local id port bin bind ts
    ts="$(date +%s)"
    for id in $(game_ids); do
        port="$(game_port "$id")"
        bin="$(server_bin "$id")"
        [ -x "$bin" ] || die "$bin was not built"
        bind="$(game_bind "$id")"
        say "starting $id on 127.0.0.1:$port"
        if [ "$id" = kings ] || [ "$id" = league ]; then
            # Both were born with this flag; their standalone recipes use it
            # and the explicit argv is the clearest identity evidence in a ps
            # row.
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

    # WHAT IS RUNNING is recorded as soon as the servers are proven on
    # loopback - before the tunnels and before the publish, which are
    # different questions. It used to come after the public probes, so a
    # host that could not mint a tunnel (Cloudflare's 429 on 2026-09-06)
    # never wrote it, and every later `update` restarted three healthy
    # servers to try again; the publish had taught the same lesson earlier,
    # when a host with no push rights on EMBER_REPO (the documented normal
    # case for "anyone with a Linux box") aborted here with both games
    # healthy and `update` then tore them down on every run.
    echo "$rev" > "$RUN/deployed"
    # The identity of what is now running, for the commands that must not read
    # it from a directory a shipper may have replaced since.
    ! prebuilt || cp "$EMBER_PREBUILT/stamp" "$DEPLOYED_STAMP"

    local tt trc=0
    tt="$(date +%s)"
    ensure_tunnels "$commit" || trc=$?
    local urls=""
    for id in $(game_ids); do
        [ -s "$RUN/$id.url" ] && urls="$urls $id=$(cat "$RUN/$id.url")"
    done
    echo "   tunnels settled in $(( $(date +%s) - tt ))s"
    if [ "$trc" -ne 0 ]; then
        say "servers UP as $name ($version · $commit) in $(( $(date +%s) - t0 ))s, but not every tunnel is proven:$urls"
        say "publishing the proven games only (dropping: $TUNNELS_MISSING)"
        # shellcheck disable=SC2086
        publish_current "$name" "$version" "$commit" $TUNNELS_MISSING || true
        echo "host.sh: 'host.sh tunnels' finishes the job once tunnels can be minted" >&2
        return 3
    fi
    say "UP as $name ($version · $commit) in $(( $(date +%s) - t0 ))s:$urls"

    say "publishing"
    local pubrc=0
    # Loud and non-zero, but not fatal to the record above: a host absent
    # from the book IS a failure and a wrapper must see it, while `update` must
    # not mistake an unpublished host for an undeployed one.
    publish_current "$name" "$version" "$commit" || pubrc=$?
    if [ "$pubrc" -ne 0 ]; then
        echo "host.sh: the servers are UP at$urls but publishing FAILED;" >&2
        echo "         players will not find this host until it publishes." >&2
        echo "         Fix EMBER_PUBLISH (currently '$EMBER_PUBLISH') and re-run." >&2
        return "$pubrc"
    fi
}

# Mint what tunnels are missing, keep the ones that answer, republish. The
# repair for "the servers are fine but a tunnel died" - which `update` does
# not do, on purpose: it must never restart healthy servers for a tunnel.
cmd_tunnels() {
    [ -x "$EMBER_TUNNEL_BIN" ] || die "no tunnel binary at $EMBER_TUNNEL_BIN (set EMBER_TUNNEL_BIN)"
    [ -n "$PY" ] || die "need a working python3 (or python) on PATH to publish the entry"
    prebuilt || [ -d "$SRC/.git" ] || die "nothing deployed yet; run host.sh up"
    local id
    for id in $(game_ids); do
        alive "server-$id" || die "the $id server is not running; run host.sh up"
    done
    # The RUNNING build, not the checkout's HEAD: the two differ whenever the
    # source moved ahead of a deploy, and the kings and league probes ask the
    # server for its commit. A prebuilt host has no checkout to ask and the same gap to
    # avoid — a shipper may have left a newer directory beside these live
    # servers — so it answers from the stamp recorded when they were started.
    local rev name version commit trc=0
    rev="$(cat "$RUN/deployed" 2>/dev/null || true)"
    [ -n "$rev" ] || die "nothing deployed yet; run host.sh up"
    name="$(bash "$(helper host-name.sh)")"
    if prebuilt; then
        version="$(running_field version)"
        commit="$(running_field commit)"
    else
        version="r$(git -C "$SRC" rev-list --count "$rev")"
        commit="$(git -C "$SRC" rev-parse --short "$rev")"
    fi
    ensure_tunnels "$commit" || trc=$?
    if [ "$trc" -ne 0 ]; then
        say "publishing the proven games only (dropping: $TUNNELS_MISSING)"
        # shellcheck disable=SC2086
        publish_current "$name" "$version" "$commit" $TUNNELS_MISSING || true
        return 3
    fi
    say "publishing"
    publish_current "$name" "$version" "$commit"
}

cmd_update() {
    local rev deployed
    if prebuilt; then
        # The stamp is what moved or did not: a shipper replaced the products
        # and said so, and there is no ref here to ask.
        require_prebuilt
        rev="$(stamp_field full_commit)"
    else
        sync_source
        rev="$(resolve_ref)" || die "EMBER_REF='$EMBER_REF' names no commit"
    fi
    deployed="$(cat "$RUN/deployed" 2>/dev/null || echo none)"
    local all_up=1 id
    for id in $(game_ids); do
        alive "server-$id" || all_up=""
    done
    if [ "$rev" = "$deployed" ] && [ -n "$all_up" ]; then
        local name version commit
        name="$(bash "$(helper host-name.sh)")"
        if prebuilt; then
            version="$(running_field version)"
            commit="$(running_field commit)"
        else
            version="r$(git -C "$SRC" rev-list --count "$rev")"
            commit="$(git -C "$SRC" rev-parse --short "$rev")"
        fi
        # A game whose tunnel is missing is dropped from the local entry rather
        # than written at an empty address (which publish-host.sh refuses).
        build_entry_args "$version" "$commit"
        write_local_entry "$name" "${ENTRY_ARGS[@]}"
        echo "up to date at ${rev:0:7} and all four servers are running; nothing to do"
        return 0
    fi
    if [ "$rev" = "$deployed" ]; then
        if prebuilt; then
            say "the stamp has not moved but something is not running; bringing it back up"
        else
            say "ref has not moved but something is not running; bringing it back up"
        fi
    else
        say "${deployed:0:7} -> ${rev:0:7}; redeploying"
    fi
    cmd_up
}

cmd_status() {
    local name; name="$(bash "$(helper host-name.sh)")"
    echo "host:      $name"
    echo "home:      $EMBER_HOME"
    if prebuilt; then
        if [ -f "$EMBER_PREBUILT/stamp" ]; then
            echo "prebuilt:  $EMBER_PREBUILT -> $(stamp_field commit) ($(stamp_field version))"
        else
            echo "prebuilt:  $EMBER_PREBUILT (no stamp there)"
        fi
        if [ -s "$DEPLOYED_STAMP" ]; then
            echo "running:   $(running_field commit) ($(running_field version))"
        fi
    elif [ -d "$SRC/.git" ]; then
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
    up)      cmd_up ;;
    update)  cmd_update ;;
    tunnels) cmd_tunnels ;;
    status) cmd_status ;;
    down)   cmd_down ;;
    *)      sed -n '2,20p' "$0" >&2; exit 2 ;;
esac
