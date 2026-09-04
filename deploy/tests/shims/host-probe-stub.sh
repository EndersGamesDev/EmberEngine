#!/usr/bin/env bash
# A logging, successful protocol probe for test-host-kings.sh.
set -euo pipefail
LOG="${SHIM_LOG:-/dev/null}"
{ printf '%s' "$(basename "$0")"; for arg in "$@"; do printf ' [%s]' "$arg"; done; printf '\n'; } >> "$LOG"
