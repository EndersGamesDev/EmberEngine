#!/usr/bin/env bash
# Bring Four Kings online multiplayer up end to end, ON THIS PC:
#   1. build + (re)start kings-server inside the claude-sdk WSL distro
#   2. (re)start a Cloudflare quick tunnel in front of it, which mints a
#      fresh https://….trycloudflare.com domain on EVERY restart
#   3. health-check by SPEAKING THE PROTOCOL, on loopback and then through
#      the public URL
#   4. only then publish the domain to server.json on GitHub Pages as
#      "kings_ws" (with "kings_proto"), merging into the other games' keys
#
# Run from Windows (Git Bash):
#   bash deploy/deploy-kings-online.sh          # up (the default)
#   bash deploy/deploy-kings-online.sh down     # stop the server and tunnel
#   bash deploy/deploy-kings-online.sh status   # what is running, log tails, current domain
#
# WHY THIS PC. The arena and Fire Racer run on a remote host over ssh
# (deploy-fire-online.sh). Four Kings is hosted on the developer's Windows
# workstation instead, inside the `claude-sdk` WSL distro, because that is
# where a Rust toolchain, cloudflared and python3 already exist: the Windows
# host itself carries no toolchains by policy, and the remote host is being
# decommissioned. So every ssh in the fire script is a `wsl -d claude-sdk`
# here, and the two long-running processes are started by a hidden wsl.exe
# that outlives this script (deploy/wsl-detach.ps1).
#
# WHAT THAT DOES NOT SURVIVE. There is no systemd in claude-sdk, so nothing
# restarts the pair after a Windows reboot, a Windows sleep, or a
# `wsl --shutdown` (the documented fix for the WSL boot wedge kills the
# server too). Nor is deploy/watchdog.sh ported to this game. After any of
# those, run this script again; until then server.json names a dead
# domain and the kings page says no online server is published.
#
# FOLLOW-UP. When feat/multi-host lands, the publish step (step 10 below)
# becomes `deploy/publish-host.sh --game kings --url <wss> --proto <n>`,
# which upserts the same two keys on this machine's hosts[] entry as well;
# the rest of this script stays as it is.
#
# HOW COMMANDS REACH THE DISTRO. Every command that runs inside claude-sdk
# is a FILE under deploy/wsl/, invoked as
#     MSYS_NO_PATHCONV=1 WSL_UTF8=1 wsl -d claude-sdk -- bash /mnt/c/<repo>/deploy/wsl/<script>.sh <args>
# never as a multi-command string to `bash -lc`: nested quoting through
# Git Bash -> wsl.exe has hung on this machine. MSYS_NO_PATHCONV=1 stops
# Git Bash rewriting the /mnt/c path into C:/Program Files/Git/mnt/c/…
# (it does the same to arguments of pwsh, so the detach call carries it
# too); WSL_UTF8=1 makes wsl.exe's own messages UTF-8 rather than UTF-16
# so a grep over its output works.
#
# Deliberately a separate script, port (7782), tunnel, logs and server.json
# keys from the arena's (7780) and fire's (7781). The games speak different
# protocols with independent version numbers, so redeploying one must never
# be able to knock another offline.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
DISTRO="${EMBER_DISTRO:-claude-sdk}"
PORT=7782
BIND="127.0.0.1:$PORT"
PROTO_FILE="crates/kings-core/src/proto.rs"
# Where the pages currently point players; `status` compares it with the log.
PAGES_URL="${EMBER_PAGES_URL:-https://endersgamesdev.github.io/EmberEngine}"

cd "$REPO_DIR"

# ---- helpers ----------------------------------------------------------------

# A Windows-side path (Git Bash /c/… or C:\…) as the distro sees it (/mnt/c/…).
to_mnt() {
    local w drive rest
    w="$(cygpath -w "$1")"
    drive="$(printf '%s' "${w:0:1}" | tr '[:upper:]' '[:lower:]')"
    rest="$(printf '%s' "${w:2}" | tr '\\' '/')"
    printf '/mnt/%s%s\n' "$drive" "$rest"
}

REPO_MNT="$(to_mnt "$REPO_DIR")"
WSL_DIR="$REPO_MNT/deploy/wsl"

# Run one of the deploy/wsl/ scripts inside the distro. $1 = script name.
sdk() {
    local script="$1"; shift
    MSYS_NO_PATHCONV=1 WSL_UTF8=1 wsl -d "$DISTRO" -- bash "$WSL_DIR/$script" "$@"
}
ctl() { sdk kings-ctl.sh "$@"; }

# Launch one of the deploy/wsl/ run scripts DETACHED; prints the wsl.exe PID.
# Empty arguments cannot cross Start-Process, so callers omit optional ones.
detach() {
    local script="$1"; shift
    local ps1
    ps1="$(cygpath -w "$REPO_DIR/deploy/wsl-detach.ps1")"
    # PowerShell writes CRLF; the CR must not reach the "pid N" lines.
    if command -v pwsh >/dev/null 2>&1; then
        MSYS_NO_PATHCONV=1 pwsh -NoProfile -File "$ps1" -Distro "$DISTRO" -Script "$WSL_DIR/$script" "$@" | tr -d '\r'
    else
        MSYS_NO_PATHCONV=1 powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$ps1" -Distro "$DISTRO" -Script "$WSL_DIR/$script" "$@" | tr -d '\r'
    fi
}

# Every step reports its own wall time (the repo rule), and the summary at
# the end lists them all.
STEP_NAME=""
STEP_T0=0
TIMINGS=()
step_done() {
    local d=$(( SECONDS - STEP_T0 ))
    echo "   [$STEP_NAME: ${d}s]"
    TIMINGS+=("$STEP_NAME ${d}s")
    STEP_NAME=""
}
step() {
    [ -n "$STEP_NAME" ] && step_done
    STEP_NAME="$1"
    STEP_T0=$SECONDS
    echo "== $1 =="
}
fail() {
    echo "FAILED: $*" >&2
    if [ -n "$STEP_NAME" ]; then
        echo "        (during '$STEP_NAME' after $(( SECONDS - STEP_T0 ))s; ${SECONDS}s since start)" >&2
    fi
    exit 1
}

# The protocol number the deploy publishes as kings_proto. The join gate is
# exact equality on it, so publishing the wrong one leaves every page unable
# to join; reading it from the crate is the only honest source.
read_proto_version() {
    local file="$1" n
    if [ ! -f "$file" ]; then
        echo "FAILED: $file does not exist, so the kings protocol version cannot be read;" >&2
        echo "        the kings-core crate must be on this branch before Four Kings can be deployed." >&2
        return 1
    fi
    n="$(grep -oE 'PROTO_VERSION: u16 = [0-9]+' "$file" | head -1 | grep -oE '[0-9]+$' || true)"
    if [ -z "$n" ]; then
        echo "FAILED: no 'PROTO_VERSION: u16 = <n>' line in $file; the deploy cannot publish kings_proto without it." >&2
        return 1
    fi
    printf '%s\n' "$n"
}

# A failing wsl.exe (the distro cannot boot, e.g. Wsl/0x8007000e "not enough
# memory") exits non-zero exactly like pgrep finding nothing, so `status` and
# `down` would read an unreachable distro as "not running". Ask it something
# that always succeeds first.
require_distro() {
    ctl host-name >/dev/null 2>&1 || fail "cannot run a command in the $DISTRO distro (wsl.exe failed; see its message above)"
}

# ---- subcommands ------------------------------------------------------------

do_down() {
    step "stopping kings-server and its tunnel"
    require_distro
    ctl stop-server
    ctl stop-tunnel "$BIND"
    sleep 1
    if ctl server-pid >/dev/null; then
        echo "   kings-server is STILL running after pkill" >&2
    else
        echo "   kings-server: stopped"
    fi
    if ctl tunnel-pid "$BIND" >/dev/null; then
        echo "   tunnel is STILL running after pkill" >&2
    else
        echo "   tunnel: stopped"
    fi
    step_done
    echo "   server.json still names the old domain; the page will say the server is unreachable until the next 'up'"
}

do_status() {
    local pid domain published
    step "kings-server ($DISTRO, $BIND)"
    require_distro
    if pid="$(ctl server-pid)"; then
        echo "   running, pid $(printf '%s' "$pid" | tr '\n' ' ')"
        [ "$(printf '%s\n' "$pid" | wc -l)" -gt 1 ] && echo "   !! more than one kings-server process; 'down' then 'up'"
        ctl server-log-tail 5 | sed 's/^/   | /'
    else
        echo "   not running"
    fi
    step "tunnel (cloudflared --url http://$BIND)"
    if pid="$(ctl tunnel-pid "$BIND")"; then
        domain="$(ctl tunnel-domain)"
        echo "   running, pid $(printf '%s' "$pid" | tr '\n' ' ')"
        echo "   domain in the log: ${domain:-(none yet)}"
        ctl tunnel-log-tail 5 | sed 's/^/   | /'
    else
        echo "   not running"
        domain=""
    fi
    step "published address"
    published="$(curl -s --max-time 15 "$PAGES_URL/server.json?ts=$(date +%s)" 2>/dev/null | grep -oE '"kings_ws": *"[^"]*"' | sed -E 's/.*: *"//; s/"$//' || true)"
    if [ -z "$published" ]; then
        echo "   kings_ws is not in $PAGES_URL/server.json (or it could not be fetched)"
    else
        echo "   kings_ws: $published"
        if [ -n "$domain" ] && [ "$published" != "wss://${domain#https://}" ]; then
            echo "   !! the published address is NOT this tunnel's domain; run 'up' to republish"
        fi
    fi
    step_done
}

do_up() {
    local VERSION COMMIT KINGS_PROTO HOST_NAME LOG_MARK PID HOLDER FIRST TUNNEL WS_URL ok i PAGES_DIR

    step "checking the tree"
    # The distro builds the working tree at /mnt/c DIRECTLY (there is no
    # tarball), so this guard is the only thing making "deployed == HEAD"
    # true. Refuse a dirty tree rather than quietly shipping something other
    # than what is in front of you.
    if ! git diff --quiet || ! git diff --cached --quiet; then
        git status --short >&2
        fail "working tree is dirty. This deploys the checkout as it stands, so commit or stash first."
    fi
    # Computed on the HOST and passed in, so distro git never reads /mnt/c.
    VERSION="r$(git rev-list --count HEAD)"
    COMMIT="$(git rev-parse --short HEAD)"
    KINGS_PROTO="$(read_proto_version "$PROTO_FILE")" || exit 1
    echo "   deploying $VERSION $COMMIT, kings protocol v$KINGS_PROTO, from $REPO_MNT"

    step "preflight"
    command -v cygpath >/dev/null 2>&1 || fail "cygpath is missing; run this from Git Bash"
    command -v wsl >/dev/null 2>&1 || fail "wsl.exe is not on PATH"
    if ! command -v pwsh >/dev/null 2>&1 && ! command -v powershell.exe >/dev/null 2>&1; then
        fail "neither pwsh nor powershell.exe is on PATH; deploy/wsl-detach.ps1 needs one of them"
    fi
    [ -f deploy/wsl-detach.ps1 ] || fail "deploy/wsl-detach.ps1 is missing"
    for f in kings-ctl.sh kings-build.sh kings-run-server.sh kings-run-tunnel.sh kings-probe.sh; do
        [ -f "deploy/wsl/$f" ] || fail "deploy/wsl/$f is missing"
    done
    ctl preflight || fail "the $DISTRO distro is missing a tool (see above)"

    step "building kings-server + probe in $DISTRO"
    sdk kings-build.sh "$REPO_MNT" "$VERSION" "$COMMIT" || fail "the build failed (see cargo's output above)"

    step "stopping the old pair"
    # Anchored on the exact binary path and on this port's --url, so the
    # arena's and fire's processes, should they ever run in this distro, are
    # never matched.
    ctl stop-server
    ctl stop-tunnel "$BIND"
    sleep 1
    echo "== checking nobody else holds $PORT =="
    HOLDER="$(ctl port-holder "$PORT")"
    if [ -n "$HOLDER" ]; then
        echo "  $HOLDER" >&2
        fail "something is already listening on $PORT (row above); stop it before deploying"
    fi

    step "launching kings-server"
    HOST_NAME="${EMBER_HOST_NAME:-}"
    [ -n "$HOST_NAME" ] || HOST_NAME="$(ctl host-name)"
    LOG_MARK="$(ctl server-log-size)"
    if [ -n "$HOST_NAME" ]; then
        PID="$(detach kings-run-server.sh "$BIND" "$HOST_NAME")"
    else
        PID="$(detach kings-run-server.sh "$BIND")"
    fi
    if [ -n "$HOST_NAME" ]; then
        echo "   wsl.exe pid $PID, --name $HOST_NAME"
    else
        echo "   wsl.exe pid $PID, unnamed (set EMBER_HOST_NAME, or write \$HOME/.ember/host-name inside $DISTRO)"
    fi
    sleep 2
    if ! ctl server-pid >/dev/null; then
        ctl server-log-tail 20 >&2 || true
        fail "kings-server is not running; last log lines above"
    fi
    FIRST="$(ctl server-log-first-after "$LOG_MARK")"
    echo "   first log line: ${FIRST:-(nothing logged yet)}"

    step "loopback probe (before exposing it)"
    # Prove the hub loop is alive on the loopback side first, so a failure
    # here is unambiguously the server rather than the tunnel. The probe
    # requires Welcome (only the hub thread produces it), the build's
    # PROTO_VERSION, and, with --expect-commit, the stamp of the binary just
    # built, which catches a missed pkill leaving last week's server on 7782.
    if ! sdk kings-probe.sh "ws://$BIND" --expect-commit "$COMMIT"; then
        ctl server-log-tail 20 >&2 || true
        fail "the server is listening but did not pass the probe on loopback"
    fi

    step "starting the tunnel (fresh domain)"
    # Truncated BEFORE the launch so the grep below can only find this run's
    # hostname. A quick tunnel mints a new random name every start, which is
    # why the republish below exists at all.
    ctl tunnel-log-truncate
    PID="$(detach kings-run-tunnel.sh "$BIND")"
    echo "   wsl.exe pid $PID"

    step "waiting for the tunnel domain"
    TUNNEL=""
    for i in $(seq 1 30); do
        sleep 2
        TUNNEL="$(ctl tunnel-domain)"
        [ -n "$TUNNEL" ] && break
    done
    if [ -z "$TUNNEL" ]; then
        ctl tunnel-log-tail 20 >&2 || true
        fail "no trycloudflare domain appeared in 60 s; see \$HOME/cloudflared-kings.log in $DISTRO (tail above)"
    fi
    WS_URL="wss://${TUNNEL#https://}"
    echo "   $TUNNEL  ->  $WS_URL"

    step "letting the tunnel hostname propagate"
    # Do not ask DNS for the hostname before Cloudflare has published it: a
    # resolver that caches the NXDOMAIN keeps serving it long after the record
    # appears, and every later probe fails while the tunnel is fine from
    # anywhere else (fire's deploy hit exactly this on 2026-09-01).
    sleep 15

    step "probe THROUGH the public URL"
    ok=""
    for i in $(seq 1 10); do
        if sdk kings-probe.sh "$WS_URL" --expect-commit "$COMMIT"; then
            ok=1
            break
        fi
        sleep 3
    done
    if [ -z "$ok" ]; then
        fail "the tunnel is up but the server never passed the probe through it. Not publishing server.json; the page keeps its previous value."
    fi

    step "publishing server.json to GitHub Pages"
    # Only after the public probe passed: publishing first would point every
    # player at a server we had not yet proved was alive through that URL.
    # Merge, never overwrite: server.json carries the arena's "ws"/"proto",
    # fire's "fire_ws"/"fire_proto", and the multi-host "hosts"/"mirrors".
    PAGES_DIR="$(mktemp -d -t ember-pages-XXXX)"
    git worktree prune
    git worktree add -q "$PAGES_DIR" gh-pages
    ctl merge "$(to_mnt "$PAGES_DIR")/server.json" kings_ws "$WS_URL" kings_proto "$KINGS_PROTO" \
        || { git worktree remove --force "$PAGES_DIR"; fail "merge-server-json.py failed; server.json untouched"; }
    (
        cd "$PAGES_DIR"
        git add server.json
        if git diff --cached --quiet; then
            echo "   server.json unchanged"
        else
            git commit -q -m "Point kings_ws at $WS_URL

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
            git push -q origin gh-pages
            echo "   pushed gh-pages"
        fi
    )
    git worktree remove --force "$PAGES_DIR"
    step_done

    echo "== ONLINE: $WS_URL =="
    echo "   the kings page picks it up from server.json on its next load"
    echo "   steps: $(printf '%s; ' "${TIMINGS[@]}")total ${SECONDS}s"
}

case "${1:-up}" in
    up)     do_up ;;
    down)   do_down ;;
    status) do_status ;;
    *)      echo "usage: bash deploy/deploy-kings-online.sh [up|down|status]" >&2; exit 2 ;;
esac
