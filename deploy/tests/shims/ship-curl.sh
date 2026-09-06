#!/usr/bin/env bash
# A fake `curl` for deploy/tests/test-ship-host.sh.
#
# It answers exactly one request — the published version.json — and models the
# failure as well as the success, because "the pages could not be read" is a
# distinct answer from "a redeploy is due" and ship-host.sh has to keep them
# apart:
#
#   SHIP_VERSION=ok        serve $SHIP_VERSION_JSON
#   SHIP_VERSION=missing   a Pages 404: `curl --fail` exits 22
#
# It is deliberately not a web server. Anything it is not asked about is a
# silent success, which is what a caller reading nothing looks like.
set -uo pipefail

LOG="${SHIM_LOG:-/dev/null}"
{ printf 'curl'; for a in "$@"; do printf ' [%s]' "$a"; done; printf '\n'; } >> "$LOG"

case "$*" in
    *version.json*)
        case "${SHIP_VERSION:-ok}" in
            ok)      cat "${SHIP_VERSION_JSON:-/dev/null}" ;;
            missing) exit 22 ;;
        esac
        ;;
esac
exit 0
