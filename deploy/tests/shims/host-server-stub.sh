#!/usr/bin/env bash
# A long-running fake game server for test-host-kings.sh.
set -euo pipefail
LOG="${SHIM_LOG:-/dev/null}"
{ printf '%s' "$(basename "$0")"; for arg in "$@"; do printf ' [%s]' "$arg"; done; printf '\n'; } >> "$LOG"
exec /bin/sleep 3600
