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

echo "== stamping the build ticker =="
bash deploy/stamp-version.sh

echo "== building wasm =="
cargo build --target wasm32-unknown-unknown --release -p pong --lib
wasm-bindgen --target web --no-typescript --out-dir web/pkg \
    target/wasm32-unknown-unknown/release/pong.wasm

echo "== publishing gh-pages =="
PAGES_DIR="$(mktemp -d -t ember-pages-XXXX)"
git worktree add "$PAGES_DIR" gh-pages

# Live version dirs (older versions stay frozen on the branch untouched).
ARENA_LIVE="games/arena/v11"
PONG_LIVE="games/pong/v2"

rm -rf "$PAGES_DIR"/index.html "$PAGES_DIR"/pkg \
    "$PAGES_DIR/$ARENA_LIVE" "$PAGES_DIR/$PONG_LIVE" "$PAGES_DIR"/games.json
mkdir -p "$PAGES_DIR/$ARENA_LIVE" "$PAGES_DIR/$PONG_LIVE"
cp web/index.html web/games.json web/version.json "$PAGES_DIR"/
cp "web/$ARENA_LIVE/index.html" "$PAGES_DIR/$ARENA_LIVE/"
cp "web/$PONG_LIVE/index.html" "$PAGES_DIR/$PONG_LIVE/"
cp -r web/pkg "$PAGES_DIR/$ARENA_LIVE/pkg"
cp -r web/pkg "$PAGES_DIR/$PONG_LIVE/pkg"
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
# use it to cache-bust the wasm bundles once per deploy. The stamp also
# records the protocol version this bundle speaks, so a bump is caught
# HERE — the moment it ships — rather than at the first failed join.
PROTO="$(grep -oE 'PROTO_VERSION: u16 = [0-9]+' crates/pong-core/src/proto.rs | grep -oE '[0-9]+$')"
echo "== shipping protocol v$PROTO =="
python - "$PAGES_DIR/server.json" "$PROTO" <<'EOF'
import json, os, sys, time
p, proto = sys.argv[1], int(sys.argv[2])
d = {}
if os.path.exists(p):
    try:
        d = json.load(open(p))
    except Exception:
        d = {}
was = d.get("proto")
d["v"] = str(int(time.time()))
d["proto"] = proto
json.dump(d, open(p, "w"))
if was is None:
    print(f"""
!! NO PREVIOUS PROTOCOL RECORDED on this Pages branch, so this deploy
!! cannot be compared against the last one. It ships v{proto}. If the
!! running pong-server was built before v{proto}, players will be told
!! "this build speaks protocol v{proto}, the live game is v<older>" and
!! cannot create or join. Check the server's build before announcing.
!! (A freshly seeded or relocated gh-pages branch lands here once; the
!! next deploy has a baseline and compares normally.)
""")
elif was != proto:
    print(f"""
!! PROTOCOL BUMP: v{was} -> v{proto}
!! The game server speaks the OLD version until it is redeployed, and the
!! server only lets a client create or join a lobby on an exact match. So
!! from the moment this page is live until pong-server is rebuilt from the
!! same commit, players get:
!!     "this build speaks protocol v{proto}, the live game is v{was}"
!! Redeploy pong-server in the SAME window. Archived pages stay frozen on
!! v{was} and will refuse to join once the server moves - expected, and
!! they already say "archived" in the hub.
""")
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
echo "== live at https://endersgamesdev.github.io/EmberEngine/ =="
