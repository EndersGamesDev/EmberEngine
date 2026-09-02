#!/usr/bin/env bash
# watchdog.sh's decisions, driven against PATH shims.
#
#   bash deploy/tests/test-watchdog.sh
#
# NO HOST IS CONTACTED AND NOTHING IS DEPLOYED. `ssh`, `curl` and `sleep` are
# the shims in deploy/tests/shims, and the two deploy scripts are replaced in
# the test's own repository by recorders that append a line to a file — what is
# under test is which redeploys the watchdog DECIDES on, not what a deploy
# does. The git side is real: a repository with a bare origin carrying main and
# gh-pages, so the fetch, the state files and the origin/gh-pages fallback are
# the genuine article.
#
# What it is here to catch: a book that cannot be read being mistaken for
# "nothing is published", which answers a CDN blip by redeploying every game on
# every host — restarting healthy servers and minting a new tunnel domain for
# each — and then does it again on the next pass, for as long as the fetch
# keeps failing.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
DEPLOY="$(cd "$HERE/.." && pwd)"
# shellcheck source=deploy/tests/lib.sh
. "$HERE/lib.sh"

TMP="$(mktemp -d -t ember-wdtest-XXXXXX)"
trap 'rm -rf "$TMP"' EXIT

SHIMS="$TMP/shims"
mkdir -p "$SHIMS"
for s in ssh curl sleep; do
    cp "$HERE/shims/$s" "$SHIMS/$s"
    chmod +x "$SHIMS/$s"
done
export PATH="$SHIMS:$PATH"
export SHIM_LOG="$TMP/argv.log"
export SHIM_HOST_NAME=amber-otter
: > "$SHIM_LOG"

# --- a repository with a bare origin, main and gh-pages ---------------------
REPO="$TMP/repo"
ORIGIN="$TMP/origin.git"
DEPLOYED="$TMP/deployed.log"
: > "$DEPLOYED"
git init -q --bare "$ORIGIN"
mkdir -p "$REPO"
cd "$REPO"
git init -q -b main .
git remote add origin "$ORIGIN"
git config user.name "ember tests"
git config user.email "tests@ember.local"
cp -r "$DEPLOY" "$REPO/deploy"
rm -rf "$REPO/deploy/tests"
# The deploys become recorders. A watchdog test must be able to tell "it
# decided to redeploy" from "it did not", and running the real scripts here
# would only re-test them.
for f in deploy-pong-online.sh deploy-fire-online.sh deploy-pages.sh; do
    cat > "$REPO/deploy/$f" <<'REC'
#!/usr/bin/env bash
# test recorder; see deploy/tests/test-watchdog.sh
echo "DEPLOYED $(basename "$0") host=${EMBER_HOST:-none}" >> "$DEPLOY_RECORD"
REC
done
git add -A
git commit -qm "first"
git push -q origin main
HEAD_SHA="$(git rev-parse HEAD)"

git checkout -q --orphan gh-pages
git rm -rq --cached .
find . -mindepth 1 -maxdepth 1 ! -name .git -exec rm -rf {} +
printf 'not json at all\n' > server.json
git add server.json
git commit -qm "an unreadable book"
git push -q origin gh-pages
git checkout -q main

STATE="$TMP/state"
mkdir -p "$STATE"
export WATCHDOG_STATE_DIR="$STATE"
export EMBER_PAGES_URL="http://pages.invalid/EmberEngine"
export EMBER_HOSTS=fakehost
export DEPLOY_RECORD="$DEPLOYED"

GOOD="$TMP/good-book.json"
cat > "$GOOD" <<'JSON'
{
  "v": "1",
  "proto": 12,
  "fire_proto": 1,
  "hosts": [
    {
      "name": "amber-otter",
      "ws": "wss://a.example",
      "proto": 12,
      "fire_ws": "wss://a-fire.example",
      "fire_proto": 1
    }
  ]
}
JSON
EMPTY_BOOK="$TMP/empty-book.json"
printf '{"v": "1", "proto": 12, "fire_proto": 1, "hosts": []}\n' > "$EMPTY_BOOK"

# run_pass: reset the state so the commit-driven branch is never what fires,
# then one --once pass. Everything the watchdog decided is in its log.
run_pass() {
    echo "$HEAD_SHA" > "$STATE/.watchdog-state-fakehost"
    echo "$HEAD_SHA" > "$STATE/.watchdog-state-pages"
    : > "$DEPLOYED"
    bash "$REPO/deploy/watchdog.sh" --once > "$TMP/pass.log" 2>&1 || true
    cat "$TMP/pass.log"
}

echo "== an unreadable book is not 'nothing is published' =="
# The book 404s and the copy on gh-pages will not parse, so this pass has no
# book at all. The old code turned that into a fleet-wide redeploy.
SHIM_BOOK=missing OUT="$(SHIM_BOOK=missing run_pass)"
contains "$OUT" "not treating hosts as unpublished this pass" "the pass says the book is unavailable"
is "$(wc -l < "$DEPLOYED" | tr -d ' ')" "0" "and nothing was redeployed"
case "$OUT" in
    *"has no published address"*) bad "an unreadable book was read as an unpublished host" ;;
    *)                            ok "no host was reported as unpublished" ;;
esac

echo "== the fetched gh-pages is the fallback when the CDN is not =="
# `git fetch origin main gh-pages` has already brought the book down, so a
# Pages outage need not cost a pass at all.
git checkout -q gh-pages
cp "$GOOD" server.json
git commit -qam "a readable book"
git push -q origin gh-pages
git checkout -q main
OUT="$(SHIM_BOOK=missing run_pass)"
contains "$OUT" "using origin/gh-pages for this pass" "it falls back to the fetched branch"
contains "$OUT" "arena OK" "and probes the address it found there"
is "$(wc -l < "$DEPLOYED" | tr -d ' ')" "0" "so still nothing was redeployed"

echo "== a published address that answers is left alone =="
OUT="$(SHIM_BOOK=ok SHIM_BOOK_FILE="$GOOD" run_pass)"
contains "$OUT" "arena OK   (wss://a.example)" "the arena is reported healthy"
contains "$OUT" "fire OK   (wss://a-fire.example)" "and so is fire"
is "$(wc -l < "$DEPLOYED" | tr -d ' ')" "0" "nothing was redeployed"

echo "== a published address that does not answer is redeployed =="
OUT="$(SHIM_BOOK=ok SHIM_BOOK_FILE="$GOOD" SHIM_PROBE_CODE=502 run_pass)"
contains "$OUT" "arena DOWN (wss://a.example)" "the dead address is named"
contains "$(cat "$DEPLOYED")" "DEPLOYED deploy-pong-online.sh host=fakehost" "and that host's arena was redeployed"

echo "== a book that parses but does not list the host still bootstraps it =="
# The distinction that matters: absent from a book that PARSED is a new host,
# and it must still be deployed.
OUT="$(SHIM_BOOK=ok SHIM_BOOK_FILE="$EMPTY_BOOK" run_pass)"
contains "$OUT" "has no published address; will deploy it" "an unlisted host is named"
contains "$(cat "$DEPLOYED")" "DEPLOYED deploy-pong-online.sh host=fakehost" "and deployed"
contains "$(cat "$DEPLOYED")" "DEPLOYED deploy-fire-online.sh host=fakehost" "both games"

echo "== a new commit redeploys whatever the book says =="
: > "$SHIM_LOG"
echo "$HEAD_SHA" > "$STATE/.watchdog-state-fakehost"
echo "0000000000000000000000000000000000000000" > "$STATE/.watchdog-state-pages"
: > "$DEPLOYED"
SHIM_BOOK=missing bash "$REPO/deploy/watchdog.sh" --once > "$TMP/pages.log" 2>&1 || true
contains "$(cat "$TMP/pages.log")" "redeploying pages" "an unreadable book does not block the pages deploy"
contains "$(cat "$DEPLOYED")" "DEPLOYED deploy-pages.sh" "which ran"

summary watchdog
