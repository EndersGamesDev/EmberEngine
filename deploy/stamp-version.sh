#!/usr/bin/env bash
# Write web/version.json from git, so the landing page can say which release
# asset it was built from.
#
#   bash deploy/stamp-version.sh
#
# The version is the commit COUNT on the current branch plus the short sha —
# "r418 · 4d53a11". Release tags identify a series version, while this source
# stamp distinguishes builds and lets a visitor compare their ordering.
#
# Called by deploy-pages.sh before assembly. SOURCE_DATE_EPOCH makes a clean
# rebuild carry the release commit's timestamp for byte-identity verification.
# Safe to run by hand: it writes only the ignored web/version.json that the
# assembler copies into the release tree.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_DIR"

COUNT="$(git rev-list --count HEAD)"
SHA="$(git rev-parse --short HEAD)"
# ISO-8601 UTC, second precision. The page shows it only in the tooltip.
if [ -n "${SOURCE_DATE_EPOCH:-}" ]; then
    [[ "$SOURCE_DATE_EPOCH" =~ ^[0-9]+$ ]] || { echo "stamp-version: SOURCE_DATE_EPOCH must be an integer" >&2; exit 1; }
    BUILT="$(date -u -d "@$SOURCE_DATE_EPOCH" +%Y-%m-%dT%H:%M:%SZ)"
else
    BUILT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
fi
SUBJECT="$(git log -1 --pretty=%s)"
# A dirty tree means the assembled bytes do not correspond to any commit.
# Saying so is the whole point of a build ticker.
DIRTY=""
if ! git diff --quiet || ! git diff --cached --quiet; then
    DIRTY="+dirty"
fi

OUT="$REPO_DIR/web/version.json"
# Hand-rolled JSON escaping for the one field that carries arbitrary text.
ESCAPED_SUBJECT="$(printf '%s' "$SUBJECT" | sed 's/\\/\\\\/g; s/"/\\"/g')"

cat > "$OUT" <<JSON
{
  "version": "r${COUNT}${DIRTY}",
  "commit": "${SHA}",
  "built": "${BUILT}",
  "subject": "${ESCAPED_SUBJECT}"
}
JSON

echo "== stamped r${COUNT}${DIRTY} · ${SHA} =="
if [ -n "$DIRTY" ]; then
    echo "!! working tree is DIRTY — the assembled page will say so, because"
    echo "!! the bytes going out do not correspond to any commit."
fi
