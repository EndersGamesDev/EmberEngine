#!/bin/bash
# The short synchronous steps of the Four Kings deploy that run inside the
# claude-sdk WSL distro, one subcommand each, so that deploy-kings-online.sh
# never passes a multi-command string through Git Bash -> wsl.exe -> bash.
#
#   bash deploy/wsl/kings-ctl.sh preflight
#   bash deploy/wsl/kings-ctl.sh host-name [name]        print the host name to use
#   bash deploy/wsl/kings-ctl.sh stop-server              pkill kings-server (exact binary path)
#   bash deploy/wsl/kings-ctl.sh stop-tunnel <bind>       pkill the tunnel anchored on its --url
#   bash deploy/wsl/kings-ctl.sh port-holder <port>       print who listens on the port (empty = free)
#   bash deploy/wsl/kings-ctl.sh server-pid               pgrep kings-server; exit 1 if none
#   bash deploy/wsl/kings-ctl.sh tunnel-pid <bind>        pgrep the tunnel; exit 1 if none
#   bash deploy/wsl/kings-ctl.sh server-log-size          bytes in the server log now
#   bash deploy/wsl/kings-ctl.sh server-log-first-after <bytes>   first log line written after that size
#   bash deploy/wsl/kings-ctl.sh server-log-tail [n]
#   bash deploy/wsl/kings-ctl.sh tunnel-log-truncate
#   bash deploy/wsl/kings-ctl.sh tunnel-log-tail [n]
#   bash deploy/wsl/kings-ctl.sh tunnel-domain            first https://*.trycloudflare.com in the tunnel log
#   bash deploy/wsl/kings-ctl.sh merge <server.json> KEY VALUE [KEY VALUE ...]
#
# Paths and patterns here are the deploy's contract: the server binary is
# $HOME/targets/ember/release/kings-server (kings-build.sh puts it there and
# kings-run-server.sh execs it), the logs are $HOME/kings-server.log and
# $HOME/cloudflared-kings.log, and the tunnel command line carries
# `--url http://<bind>`. Matching the server on its full path and the tunnel
# on its port is what keeps the arena's and fire's processes out of reach.
set -u
source "$HOME/.cargo/env"
export CARGO_TARGET_DIR="$HOME/targets/ember"

SERVER_BIN="$CARGO_TARGET_DIR/release/kings-server"
# The [r] trick: the pattern never matches its own pgrep/pkill command line.
SERVER_PATTERN="$CARGO_TARGET_DIR/release/kings-serve[r]"
SERVER_LOG="$HOME/kings-server.log"
TUNNEL_LOG="$HOME/cloudflared-kings.log"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

tunnel_pattern() { printf 'cloudflare[d] tunnel --url http://%s' "$1"; }

cmd="${1:-}"
shift || true
case "$cmd" in
    preflight)
        missing=""
        for tool in cargo cloudflared python3 ss pgrep pkill chrt ionice; do
            command -v "$tool" >/dev/null 2>&1 || missing="$missing $tool"
        done
        if [ -n "$missing" ]; then
            echo "kings-ctl preflight: missing in this distro:$missing" >&2
            echo "                     cargo comes from rustup; cloudflared from the Cloudflare apt repo or a release binary;" >&2
            echo "                     python3 and ss (iproute2) from apt; see the claude-sdk entry in the global CLAUDE.md." >&2
            exit 1
        fi
        echo "kings-ctl preflight: cargo $(cargo --version 2>/dev/null | cut -d' ' -f2), cloudflared $(cloudflared --version 2>/dev/null | head -1 | cut -d' ' -f3), python3 $(python3 --version 2>/dev/null | cut -d' ' -f2), ss present"
        ;;
    host-name)
        name="${1:-}"
        if [ -z "$name" ] && [ -r "$HOME/.ember/host-name" ]; then
            name="$(tr -d '[:space:]' < "$HOME/.ember/host-name")"
        fi
        printf '%s\n' "$name"
        ;;
    stop-server)
        pkill -f "$SERVER_PATTERN" 2>/dev/null
        true
        ;;
    stop-tunnel)
        bind="${1:?kings-ctl stop-tunnel <bind>}"
        pkill -f "$(tunnel_pattern "$bind")" 2>/dev/null
        true
        ;;
    port-holder)
        port="${1:?kings-ctl port-holder <port>}"
        ss -ltnp "sport = :$port" 2>/dev/null | tail -n +2
        true
        ;;
    server-pid)
        pgrep -f "$SERVER_PATTERN"
        ;;
    tunnel-pid)
        bind="${1:?kings-ctl tunnel-pid <bind>}"
        pgrep -f "$(tunnel_pattern "$bind")"
        ;;
    server-log-size)
        if [ -f "$SERVER_LOG" ]; then stat -c %s "$SERVER_LOG"; else echo 0; fi
        ;;
    server-log-first-after)
        offset="${1:?kings-ctl server-log-first-after <bytes>}"
        [ -f "$SERVER_LOG" ] && tail -c "+$(( offset + 1 ))" "$SERVER_LOG" | head -1
        true
        ;;
    server-log-tail)
        [ -f "$SERVER_LOG" ] && tail -n "${1:-20}" "$SERVER_LOG"
        true
        ;;
    tunnel-log-truncate)
        : > "$TUNNEL_LOG"
        ;;
    tunnel-log-tail)
        [ -f "$TUNNEL_LOG" ] && tail -n "${1:-20}" "$TUNNEL_LOG"
        true
        ;;
    tunnel-domain)
        [ -f "$TUNNEL_LOG" ] && grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' "$TUNNEL_LOG" | head -1
        true
        ;;
    merge)
        exec python3 "$HERE/../merge-server-json.py" "$@"
        ;;
    *)
        echo "kings-ctl: unknown subcommand '$cmd'; see the header of $0" >&2
        exit 2
        ;;
esac
