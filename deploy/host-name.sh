#!/usr/bin/env bash
# Print this machine's ember host name — the merge key of its entry in the
# address book (docs/hosts.md §6).
#
#   bash deploy/host-name.sh
#   ssh somebox bash -s < deploy/host-name.sh      # works with no checkout there
#
# Three sources, in order:
#   1. EMBER_HOST_NAME, when set and non-empty — the manual override, and the
#      way to break a collision between two machines that hashed the same.
#   2. ~/.ember/host-name, when it exists — the name this box already published
#      under. A host must keep its name across deploys or every deploy would
#      add a NEW entry to the book instead of replacing its own.
#   3. A fresh name derived from sha256("<hostname>|<user>"), written to that
#      file. Deterministic on purpose: a box that loses ~/.ember (a reimage, a
#      wiped home) comes back under the same name rather than orphaning its
#      old entry.
#
# Deliberately depends on nothing but itself: it is piped into `bash -s` over
# ssh by the workstation deploys, on hosts that have no checkout of this repo.
set -euo pipefail

# The name file. The default is the contract's `~/.ember/host-name`; the
# override exists so deploy/tests can exercise this without touching the real
# one on whatever machine runs the tests.
NAME_FILE="${EMBER_NAME_FILE:-$HOME/.ember/host-name}"

# `[a-z0-9-]`, 3 to 32 characters — the shape the book's merge key and the
# `--name` flag both accept. Checked on EVERY path, including the manual
# override: a name with a space or a slash in it would be written into JSON
# and into a pid-file path, and would fail somewhere much less obvious.
valid() {
    printf '%s' "$1" | grep -qE '^[a-z0-9-]{3,32}$'
}

die() { echo "host-name: $*" >&2; exit 1; }

# 1. The override.
if [ -n "${EMBER_HOST_NAME:-}" ]; then
    valid "$EMBER_HOST_NAME" \
        || die "EMBER_HOST_NAME='$EMBER_HOST_NAME' is not [a-z0-9-]{3,32}"
    printf '%s\n' "$EMBER_HOST_NAME"
    exit 0
fi

# 2. The name this box already has.
if [ -s "$NAME_FILE" ]; then
    STORED="$(head -1 "$NAME_FILE" | tr -d '[:space:]')"
    if valid "$STORED"; then
        printf '%s\n' "$STORED"
        exit 0
    fi
    # A corrupt file is worth a word rather than a silent rename: the entry it
    # used to publish under is about to be orphaned in the book.
    echo "host-name: ignoring unusable $NAME_FILE ('$STORED'); generating a new name" >&2
fi

# 3. Derive one.
#
# Two words, adjective then noun, from fixed lists of 24. 576 combinations is
# not collision-proof and is not meant to be — §6 says the later publish wins
# and to set EMBER_HOST_NAME on one of the two. What it buys is a name a human
# can say out loud on a chip in the corner of the page.
ADJECTIVES="amber azure bronze cobalt copper crimson dusky ember flint golden hollow indigo ivory jade lunar misty olive quiet rapid russet silver slate teal violet"
NOUNS="otter heron falcon badger marten walrus lynx raven ibex tapir osprey marlin gannet wombat jackal kestrel bison cobra dingo egret gecko hornet jaguar koala"

# hostname(1) is not everywhere (busybox images ship uname only), and the two
# do not always agree, so prefer hostname and fall back rather than hashing an
# empty string on the boxes that lack it.
HOST="$(hostname 2>/dev/null || uname -n 2>/dev/null || echo unknown)"
USER_NAME="$(id -un 2>/dev/null || echo unknown)"

# Whichever sha256 this box has. Coreutils, BSD/macOS, and openssl cover
# everything we are likely to be piped into; none of the three is guaranteed.
if command -v sha256sum >/dev/null 2>&1; then
    HASH="$(printf '%s|%s' "$HOST" "$USER_NAME" | sha256sum | cut -d' ' -f1)"
elif command -v shasum >/dev/null 2>&1; then
    HASH="$(printf '%s|%s' "$HOST" "$USER_NAME" | shasum -a 256 | cut -d' ' -f1)"
elif command -v openssl >/dev/null 2>&1; then
    HASH="$(printf '%s|%s' "$HOST" "$USER_NAME" | openssl dgst -sha256 | tr -d ' ' | sed 's/.*=//')"
else
    die "no sha256 available (need sha256sum, shasum or openssl); set EMBER_HOST_NAME"
fi

# 32 bits of it is plenty for 576 buckets, and stays inside bash arithmetic on
# a 64-bit shell with room to spare.
N=$((16#${HASH:0:8}))
# shellcheck disable=SC2086
set -- $ADJECTIVES
ADJ_COUNT=$#
eval "ADJ=\${$(( N % ADJ_COUNT + 1 ))}"
# shellcheck disable=SC2086
set -- $NOUNS
NOUN_COUNT=$#
eval "NOUN=\${$(( N / ADJ_COUNT % NOUN_COUNT + 1 ))}"
NAME="$ADJ-$NOUN"

valid "$NAME" || die "generated name '$NAME' is not [a-z0-9-]{3,32} (word list bug)"

# Store it before printing, so the caller that acts on the name and the file
# that will be read next time can never disagree. A read-only or full home is
# not fatal — the name is derived, so the next run recomputes the same one —
# but it is worth saying, because it means an EMBER_HOST_NAME override or a
# hostname change will not stick.
if mkdir -p "$(dirname "$NAME_FILE")" 2>/dev/null \
    && printf '%s\n' "$NAME" > "$NAME_FILE" 2>/dev/null; then
    :
else
    echo "host-name: could not write $NAME_FILE; the name is derived, so it still holds" >&2
fi

printf '%s\n' "$NAME"
