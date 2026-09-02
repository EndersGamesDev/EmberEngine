#!/bin/bash
# Run a Cloudflare quick tunnel in front of kings-server, in the FOREGROUND,
# inside the claude-sdk WSL distro, output appended to
# $HOME/cloudflared-kings.log.
#
#   bash deploy/wsl/kings-run-tunnel.sh <bind>          e.g. 127.0.0.1:7782
#
# Launched by deploy/wsl-detach.ps1 for the same reason as the server: the
# hidden wsl.exe keeps the process alive after the deploy returns. The
# command line is exactly `cloudflared tunnel --url http://<bind>
# --no-autoupdate`, and deploy-kings-online.sh's pkill is anchored on that
# --url, so the arena's and fire's tunnels (other ports) are never matched.
#
# A quick tunnel mints a NEW random *.trycloudflare.com hostname every time it
# starts; the deploy greps it out of this log and publishes it. The log is
# truncated by the deploy BEFORE this script is launched (kings-ctl.sh
# tunnel-log-truncate), so the grep can only find this run's hostname.
set -u
source "$HOME/.cargo/env"
export CARGO_TARGET_DIR="$HOME/targets/ember"

BIND="${1:?usage: kings-run-tunnel.sh <bind>}"
LOG="$HOME/cloudflared-kings.log"

if ! command -v cloudflared >/dev/null 2>&1; then
    echo "kings-run-tunnel: cloudflared is not installed in this distro" >> "$LOG"
    exit 1
fi
exec cloudflared tunnel --url "http://$BIND" --no-autoupdate >> "$LOG" 2>&1
