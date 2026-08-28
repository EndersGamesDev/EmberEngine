#!/usr/bin/env bash
# Build the pong wasm bundle and publish web/ to GitHub Pages (gh-pages).
# Run from anywhere (git-bash): bash deploy/deploy-pages.sh
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_DIR"

echo "== building wasm =="
cargo build --target wasm32-unknown-unknown --release -p pong --lib
wasm-bindgen --target web --no-typescript --out-dir web/pkg \
    target/wasm32-unknown-unknown/release/pong.wasm

echo "== publishing gh-pages =="
PAGES_DIR="$(mktemp -d -t ember-pages-XXXX)"
git worktree add "$PAGES_DIR" gh-pages
rm -rf "$PAGES_DIR"/index.html "$PAGES_DIR"/pkg
cp -r web/index.html web/pkg "$PAGES_DIR"/
touch "$PAGES_DIR"/.nojekyll
# Bump the deploy stamp in server.json (preserving the ws url): the page
# uses it to cache-bust the wasm bundle once per deploy.
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
        git commit -m "Deploy pong web build

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
        git push origin gh-pages
    fi
)
git worktree remove --force "$PAGES_DIR"
echo "== live at https://enderpeer.github.io/ember/ =="
