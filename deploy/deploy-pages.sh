#!/usr/bin/env bash
# Build the wasm bundle and publish the games hub to GitHub Pages (gh-pages).
# Run from anywhere (git-bash): bash deploy/deploy-pages.sh
#
# Layout on gh-pages:
#   index.html            games hub (lobby showcase + catalog)
#   games.json            catalog — the newest version of each game is "live"
#   server.json           {ws, v} — current tunnel domain + deploy stamp
#   games/arena/v3/       live arena build (page + its own frozen pkg)
#   games/pong/v2/        live pong build (page + its own frozen pkg)
#   games/pong/v1/        archived first web build (materialized from history)
#   pkg/                  legacy root bundle, kept fresh for old cached pages
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_DIR"
# gh-pages commit holding the original first web build (auto-run pong).
V1_COMMIT="e7b85e8"

echo "== building wasm =="
cargo build --target wasm32-unknown-unknown --release -p pong --lib
wasm-bindgen --target web --no-typescript --out-dir web/pkg \
    target/wasm32-unknown-unknown/release/pong.wasm

echo "== publishing gh-pages =="
PAGES_DIR="$(mktemp -d -t ember-pages-XXXX)"
git worktree add "$PAGES_DIR" gh-pages

rm -rf "$PAGES_DIR"/index.html "$PAGES_DIR"/pkg \
    "$PAGES_DIR"/games/arena/v3 "$PAGES_DIR"/games/pong/v2 "$PAGES_DIR"/games.json
mkdir -p "$PAGES_DIR"/games/arena/v3 "$PAGES_DIR"/games/pong/v2
cp web/index.html web/games.json "$PAGES_DIR"/
cp web/games/arena/v3/index.html "$PAGES_DIR"/games/arena/v3/
cp web/games/pong/v2/index.html "$PAGES_DIR"/games/pong/v2/
cp -r web/pkg "$PAGES_DIR"/games/arena/v3/pkg
cp -r web/pkg "$PAGES_DIR"/games/pong/v2/pkg
cp -r web/pkg "$PAGES_DIR"/pkg
touch "$PAGES_DIR"/.nojekyll

# Archived first web build: materialize once from gh-pages history.
if [ ! -f "$PAGES_DIR/games/pong/v1/index.html" ]; then
    echo "== materializing archived pong v1 from $V1_COMMIT =="
    mkdir -p "$PAGES_DIR"/games/pong/v1/pkg
    git show "$V1_COMMIT:index.html" > "$PAGES_DIR"/games/pong/v1/index.html
    git show "$V1_COMMIT:pkg/pong.js" > "$PAGES_DIR"/games/pong/v1/pkg/pong.js
    git show "$V1_COMMIT:pkg/pong_bg.wasm" > "$PAGES_DIR"/games/pong/v1/pkg/pong_bg.wasm
fi

# Bump the deploy stamp in server.json (preserving the ws url): the pages
# use it to cache-bust the wasm bundles once per deploy.
python - "$PAGES_DIR/server.json" <<'EOF'
import json, os, sys, time
p = sys.argv[1]
d = {}
if os.path.exists(p):
    try:
        d = json.load(open(p))
    except Exception:
        d = {}
d["v"] = str(int(time.time()))
json.dump(d, open(p, "w"))
EOF

(
    cd "$PAGES_DIR"
    git add -A
    if git diff --cached --quiet; then
        echo "nothing changed; skipping commit"
    else
        git commit -m "Deploy games hub

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
        git push origin gh-pages
    fi
)
git worktree remove --force "$PAGES_DIR"
echo "== live at https://enderpeer.github.io/ember/ =="
