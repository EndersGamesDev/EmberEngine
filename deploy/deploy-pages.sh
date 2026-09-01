#!/usr/bin/env bash
# Build the wasm bundle and publish the games hub to GitHub Pages (gh-pages).
# Run from anywhere (git-bash): bash deploy/deploy-pages.sh
#
# Layout on gh-pages:
#   index.html            games hub (lobby showcase + catalog)
#   games.json            catalog — the newest version of each game is "live"
#   server.json           {ws, v} — current tunnel domain + deploy stamp
#   games/arena/v12/      live arena build (page + its own frozen pkg)
#   games/pong/v2/        live pong build (page + its own frozen pkg)
#   games/fire/v2/        live fire racer build (castle circuit, online)
#   games/pong/v1/        archived first web build (materialized from history)
#   games/fire/v1/        archived first fire build; already on the branch and
#                         deliberately never touched again — only $FIRE_LIVE is
#                         removed and rewritten below
#   pkg/                  legacy root bundle, kept fresh for old cached pages
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_DIR"
# gh-pages commit holding the original first web build (auto-run pong).
V1_COMMIT="e7b85e8"

echo "== stamping the build ticker =="
bash deploy/stamp-version.sh

echo "== building wasm =="
cargo build --target wasm32-unknown-unknown --release -p fire --lib
cargo build --target wasm32-unknown-unknown --release -p pong --lib
wasm-bindgen --target web --no-typescript --out-dir web/pkg \
    target/wasm32-unknown-unknown/release/fire.wasm
wasm-bindgen --target web --no-typescript --out-dir web/pkg \
    target/wasm32-unknown-unknown/release/pong.wasm

echo "== publishing gh-pages =="
PAGES_DIR="$(mktemp -d -t ember-pages-XXXX)"
git worktree add "$PAGES_DIR" gh-pages

# Live version dirs (older versions stay frozen on the branch untouched).
ARENA_LIVE="games/arena/v12"
PONG_LIVE="games/pong/v2"
FIRE_LIVE="games/fire/v2"

rm -rf "$PAGES_DIR"/index.html "$PAGES_DIR"/pkg \
    "$PAGES_DIR/$ARENA_LIVE" "$PAGES_DIR/$PONG_LIVE" "$PAGES_DIR/$FIRE_LIVE" "$PAGES_DIR"/games.json
mkdir -p "$PAGES_DIR/$ARENA_LIVE" "$PAGES_DIR/$PONG_LIVE" "$PAGES_DIR/$FIRE_LIVE"
cp web/index.html web/games.json web/version.json "$PAGES_DIR"/
# The shared host-picking logic (docs/hosts.md §5). It lives at the pages root
# and every live page imports it from there, so there is one copy of the rule
# rather than one per game. Guarded because a checkout that predates it still
# has to be deployable: the frozen pages carry their own inline discovery and
# read the legacy keys, so a hub without hosts.js degrades to what it did
# before rather than breaking.
if [ -f web/hosts.js ]; then
    cp web/hosts.js "$PAGES_DIR"/
else
    echo "   note: web/hosts.js does not exist in this checkout; not copying it"
fi
cp "web/$ARENA_LIVE/index.html" "$PAGES_DIR/$ARENA_LIVE/"
cp "web/$PONG_LIVE/index.html" "$PAGES_DIR/$PONG_LIVE/"
cp "web/$FIRE_LIVE/index.html" "$PAGES_DIR/$FIRE_LIVE/"
# Each game gets ONLY its own bundle. Copying the whole of web/pkg into every
# game directory shipped pong's 18 MB wasm to fire players and fire's to pong
# players — a fire player was downloading ~23 MB to run a ~6 MB game. The
# root pkg/ still carries everything, because old cached pages resolve their
# imports against it.
copy_pkg() {
    # $1 = destination dir, $2... = crate names whose bundle belongs there
    local dest="$1"; shift
    mkdir -p "$dest"
    for crate in "$@"; do
        cp "web/pkg/$crate.js" "web/pkg/${crate}_bg.wasm" "$dest/"
    done
}
copy_pkg "$PAGES_DIR/$ARENA_LIVE/pkg" pong
copy_pkg "$PAGES_DIR/$PONG_LIVE/pkg" pong
copy_pkg "$PAGES_DIR/$FIRE_LIVE/pkg" fire
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
# Fire carries its own version in its own crate, on purpose: bumping one game's
# protocol must never gate the other's join.
FIRE_PROTO="$(grep -oE 'PROTO_VERSION: u16 = [0-9]+' crates/fire-core/src/proto.rs | grep -oE '[0-9]+$')"
echo "== shipping arena protocol v$PROTO, fire protocol v$FIRE_PROTO =="
python - "$PAGES_DIR/server.json" "$PROTO" "$FIRE_PROTO" <<'EOF'
import json, os, sys, time
p, proto, fire_proto = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
d = {}
if os.path.exists(p):
    try:
        d = json.load(open(p))
    except Exception:
        d = {}
was = d.get("proto")
was_fire = d.get("fire_proto")
d["v"] = str(int(time.time()))
d["proto"] = proto
d["fire_proto"] = fire_proto
json.dump(d, open(p, "w"))
if was_fire is not None and was_fire != fire_proto:
    print(f"""
!! FIRE PROTOCOL BUMP: v{was_fire} -> v{fire_proto}
!! fire-server speaks the OLD version until it is redeployed, and the join
!! gate is exact equality, so from now until `bash deploy/deploy-fire-online.sh`
!! runs, players get:
!!     "this build speaks fire protocol v{fire_proto}, the live game is v{was_fire}"
!! The lobby LISTING keeps working at any version by design, so the browser
!! will show lobbies nobody can enter until the server catches up.
""")
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

# The top-level protocol keys just moved, and the legacy top-level ADDRESS
# keys are defined against them: `ws` must name a host that speaks the
# protocol the pages now ship. Recompute them from the host list immediately,
# so a bump re-points `ws` at a host that already speaks the new version
# instead of leaving every frozen and live page on a host they can no longer
# join until somebody redeploys a server.
bash "$REPO_DIR/deploy/publish-host.sh" --book "$PAGES_DIR/server.json" --recompute

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
