#!/usr/bin/env bash
# Write web/version.json from git, so the landing page can say which build it
# was published from.
#
#   bash deploy/stamp-version.sh
#
# The version is the commit COUNT on the current branch plus the short sha —
# "r418 · 4d53a11". A count is used rather than a tag because this repo does
# not tag, and a bare sha tells a visitor nothing about whether the page they
# are looking at is newer than the one they saw yesterday. A count does.
#
# Called by deploy-pages.sh before publishing. Safe to run by hand: it only
# writes web/version.json, which is committed so the file always exists even
# when a page is served from a checkout that never ran the deploy.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_DIR"

COUNT="$(git rev-list --count HEAD)"
SHA="$(git rev-parse --short HEAD)"
# ISO-8601 UTC, second precision. The page shows it only in the tooltip.
BUILT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
SUBJECT="$(git log -1 --pretty=%s)"
# A dirty tree means the published bytes do not correspond to any commit.
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
    echo "!! working tree is DIRTY — the published page will say so, because"
    echo "!! the bytes going out do not correspond to any commit."
fi
