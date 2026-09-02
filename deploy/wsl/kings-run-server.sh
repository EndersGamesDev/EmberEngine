#!/bin/bash
# Run kings-server in the FOREGROUND inside the claude-sdk WSL distro, with
# its output appended to $HOME/kings-server.log.
#
#   bash deploy/wsl/kings-run-server.sh <bind> [host name]
#
# This script is what deploy/wsl-detach.ps1 launches: the wsl.exe it starts
# stays alive for as long as this foreground process does, which is the only
# way a process in a systemd-less distro outlives the deploy that started it
# (a `nohup ... &` inside a `wsl -- bash` command is reaped the moment that
# command returns). `exec` makes the server the process itself, so the pkill
# in deploy-kings-online.sh, anchored on the binary path, finds exactly one.
#
# The host name goes to the server as --name (docs/hosts.md convention:
# Welcome carries it). It is the second argument when the deploy resolved
# EMBER_HOST_NAME on the host; otherwise $HOME/.ember/host-name inside the
# distro if that file exists; otherwise the server runs unnamed. An empty
# second argument cannot arrive through Start-Process, so absent means unset.
set -u
source "$HOME/.cargo/env"
export CARGO_TARGET_DIR="$HOME/targets/ember"

BIND="${1:?usage: kings-run-server.sh <bind> [host name]}"
NAME="${2:-}"
if [ -z "$NAME" ] && [ -r "$HOME/.ember/host-name" ]; then
    NAME="$(tr -d '[:space:]' < "$HOME/.ember/host-name")"
fi

BIN="$CARGO_TARGET_DIR/release/kings-server"
LOG="$HOME/kings-server.log"
if [ ! -x "$BIN" ]; then
    echo "kings-run-server: $BIN is missing; run the build step first" >> "$LOG"
    exit 1
fi

export RUST_LOG="${RUST_LOG:-info}"
if [ -n "$NAME" ]; then
    exec "$BIN" "$BIND" --name "$NAME" >> "$LOG" 2>&1
else
    exec "$BIN" "$BIND" >> "$LOG" 2>&1
fi
