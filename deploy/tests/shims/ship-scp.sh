#!/usr/bin/env bash
# A fake `scp` for deploy/tests/test-ship-host.sh.
#
# Logging the argv is most of the job: the copy LIST is what the suite pins.
# The two fetches have to produce something, though, because ship-host.sh goes
# on to chmod the products and to print the entry — a shim that only logged
# would turn a real ordering bug into a test that passed.
set -uo pipefail

LOG="${SHIM_LOG:-/dev/null}"
{ printf 'scp'; for a in "$@"; do printf ' [%s]' "$a"; done; printf '\n'; } >> "$LOG"

# The last argument is the destination; the one before it, for these calls, is
# the source.
dest=""
src=""
for a in "$@"; do
    src="$dest"
    dest="$a"
done

case "$src" in
    *:*ember-ship-products*)
        # Products off the builder: eight files, inert but executable. The
        # list models what the build script stages, so it has to move with
        # it — a shim that fabricates fewer products than ship-host.sh
        # stages is a fixture that stopped describing the builder.
        mkdir -p "$dest"
        for product in arena-server fire-server kings-server league-server \
                       wsbot fire-probe kings-probe league-probe; do
            printf '#!/bin/sh\nexit 0\n' > "$dest/$product"
            chmod 0755 "$dest/$product"
        done
        ;;
    *:*run/host.json*)
        cp "${SHIP_HOST_JSON:-/dev/null}" "$dest"
        ;;
esac
exit 0
