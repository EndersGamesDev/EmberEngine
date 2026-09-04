#!/usr/bin/env bash
# Build the wasm bundle and publish the games hub to GitHub Pages (gh-pages).
# Run from anywhere (git-bash): bash deploy/deploy-pages.sh
#
# Server-build/workstation-publish recipe (the workstation holds the push key):
#   cargo build --target wasm32-unknown-unknown --release -p fire -p arena -p kings -p what-is-this --lib
#   wasm-bindgen --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/fire.wasm
#   wasm-bindgen --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/arena.wasm
#   wasm-bindgen --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/kings.wasm
#   wasm-bindgen --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/what_is_this.wasm
# Copy web/pkg from the server into this checkout, then publish without builds:
#   EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh
#
# Layout on gh-pages:
#   index.html            games hub (lobby showcase + catalog)
#   games.json            catalog — the newest version of each game is "live"
#   server.json           {ws, v} — current tunnel domain + deploy stamp
#   games/arena/v20/      live arena build — the realism pass: real ballistics, tracers, impacts, spatial sound (page + its own frozen pkg)
#   games/arena/v0/       live arena v0 pong classic (page + frozen pkg)
#   games/fire/v2/        live fire racer build (castle circuit, online)
#   games/kings/v1/       live four kings build (2D page board + 3D wasm view, online)
#   games/what-is-this/v1/ live browser and hardware diagnostic
#   games/pong/v1/        archived first web build (materialized from history)
#   games/fire/v1/        archived first fire build; already on the branch and
#                         deliberately never touched again — only $FIRE_LIVE is
#                         removed and rewritten below
#   pkg/                  legacy root bundle, kept fresh for old cached pages
set -euo pipefail

die() { echo "deploy-pages: $*" >&2; exit 1; }
PY="$(command -v python3 || command -v python)" || die "need python3 or python on PATH"

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_DIR"
# gh-pages commit holding the original first web build (auto-run pong).
V1_COMMIT="e7b85e8"

if [ "${EMBER_PAGES_PREBUILT:-}" = 1 ]; then
    missing=()
    for bundle in fire arena kings what_is_this; do
        for artifact in "$bundle.js" "${bundle}_bg.wasm"; do
            [ -f "web/pkg/$artifact" ] || missing+=("web/pkg/$artifact")
        done
    done
    if [ "${#missing[@]}" -ne 0 ]; then
        echo "FAILED: EMBER_PAGES_PREBUILT=1 requires all four bundles in web/pkg; missing:" >&2
        printf '  %s\n' "${missing[@]}" >&2
        exit 1
    fi
fi

echo "== stamping the build ticker =="
bash deploy/stamp-version.sh

if [ "${EMBER_PAGES_PREBUILT:-}" = 1 ]; then
    echo "== using four prebuilt wasm bundles from web/pkg =="
else
    echo "== building wasm =="
    cargo build --target wasm32-unknown-unknown --release -p fire --lib
    cargo build --target wasm32-unknown-unknown --release -p arena --lib
    cargo build --target wasm32-unknown-unknown --release -p kings --lib
    cargo build --target wasm32-unknown-unknown --release -p what-is-this --lib
    wasm-bindgen --target web --no-typescript --out-dir web/pkg \
        target/wasm32-unknown-unknown/release/fire.wasm
    wasm-bindgen --target web --no-typescript --out-dir web/pkg \
        target/wasm32-unknown-unknown/release/arena.wasm
    wasm-bindgen --target web --no-typescript --out-dir web/pkg \
        target/wasm32-unknown-unknown/release/kings.wasm
    wasm-bindgen --target web --no-typescript --out-dir web/pkg \
        target/wasm32-unknown-unknown/release/what_is_this.wasm
fi

echo "== publishing gh-pages =="
# Detached at what ORIGIN has, never at the local branch. `git worktree add
# <dir> gh-pages` checked out this checkout's own gh-pages, which nothing here
# fetches — `git fetch` moves origin/gh-pages and not the branch — so once a
# second writer published (another workstation, a host running host.sh with
# EMBER_PUBLISH=upstream) the push below was rejected as a non-fast-forward.
# `--detach` also means a leftover worktree still holding the local branch
# cannot block this one, which `-B gh-pages origin/gh-pages` would not survive.
git fetch -q origin gh-pages \
    || { echo "FAILED: cannot fetch origin gh-pages; is the branch there?" >&2; exit 1; }
PAGES_DIR="$(mktemp -d -t ember-pages-XXXX)"
# Armed BEFORE the add, so neither a failing add nor anything after it can
# leave the directory registered as a worktree. Without this, one failed push
# left gh-pages checked out under /tmp and every later deploy — of the pages
# and of either game — died at its own `worktree add` until a human ran
# `git worktree remove`. The status is preserved: the trap reports the failure
# that caused it, not the cleanup's own.
trap 'st=$?; git worktree remove --force "$PAGES_DIR" >/dev/null 2>&1 || true; rm -rf "$PAGES_DIR"; exit $st' EXIT
git worktree add -q --detach "$PAGES_DIR" FETCH_HEAD

# Live version dirs (older versions stay frozen on the branch untouched).
ARENA_LIVE="games/arena/v20"
ARENA_V0_LIVE="games/arena/v0"
FIRE_LIVE="games/fire/v2"
KINGS_LIVE="games/kings/v1"
WHAT_LIVE="games/what-is-this/v1"

rm -rf "$PAGES_DIR"/index.html "$PAGES_DIR"/pkg \
    "$PAGES_DIR/$ARENA_LIVE" "$PAGES_DIR/$ARENA_V0_LIVE" "$PAGES_DIR/$FIRE_LIVE" "$PAGES_DIR/$KINGS_LIVE" "$PAGES_DIR/$WHAT_LIVE" \
    "$PAGES_DIR"/games.json
mkdir -p "$PAGES_DIR/$ARENA_LIVE" "$PAGES_DIR/$ARENA_V0_LIVE" "$PAGES_DIR/$FIRE_LIVE" "$PAGES_DIR/$KINGS_LIVE" "$PAGES_DIR/$WHAT_LIVE"
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
cp "web/$ARENA_V0_LIVE/index.html" "$PAGES_DIR/$ARENA_V0_LIVE/"
cp "web/$FIRE_LIVE/index.html" "$PAGES_DIR/$FIRE_LIVE/"
cp "web/$KINGS_LIVE/index.html" "$PAGES_DIR/$KINGS_LIVE/"
cp "web/$WHAT_LIVE/index.html" "$PAGES_DIR/$WHAT_LIVE/"
# Each game gets ONLY its own bundle. Copying the whole of web/pkg into every
# game directory shipped arena's 18 MB wasm to fire players and fire's to arena
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
copy_pkg "$PAGES_DIR/$ARENA_LIVE/pkg" arena
copy_pkg "$PAGES_DIR/$ARENA_V0_LIVE/pkg" arena
copy_pkg "$PAGES_DIR/$FIRE_LIVE/pkg" fire
copy_pkg "$PAGES_DIR/$KINGS_LIVE/pkg" kings
copy_pkg "$PAGES_DIR/$WHAT_LIVE/pkg" what_is_this
cp -r web/pkg "$PAGES_DIR"/pkg
# Compatibility shim for cached pre-rename pages that import from root pkg/.
cp "$PAGES_DIR/pkg/arena.js" "$PAGES_DIR/pkg/pong.js"
cp "$PAGES_DIR/pkg/arena_bg.wasm" "$PAGES_DIR/pkg/pong_bg.wasm"
touch "$PAGES_DIR"/.nojekyll

# Archived first web build: materialize once from gh-pages history.
if [ ! -f "$PAGES_DIR/games/pong/v1/index.html" ]; then
    echo "== materializing archived pong v1 from $V1_COMMIT =="
    mkdir -p "$PAGES_DIR"/games/pong/v1/pkg
    git show "$V1_COMMIT:index.html" > "$PAGES_DIR"/games/pong/v1/index.html
    git show "$V1_COMMIT:pkg/pong.js" > "$PAGES_DIR"/games/pong/v1/pkg/pong.js
    git show "$V1_COMMIT:pkg/pong_bg.wasm" > "$PAGES_DIR"/games/pong/v1/pkg/pong_bg.wasm
fi

# The catalog is the hub's promise. Refuse to publish a live link unless this
# assembly actually produced its page, so games.json and this script cannot
# silently drift apart again.
LIVE_PATHS="$("$PY" -c 'import json, sys; d=json.load(open(sys.argv[1], encoding="utf-8")); print("\n".join(v["path"] for g in d["games"] for v in g["versions"] if v.get("live") is True))' web/games.json)"
while IFS= read -r live_path; do
    [ -n "$live_path" ] || continue
    if [ ! -f "$PAGES_DIR/${live_path%/}/index.html" ]; then
        echo "FAILED: live catalog path was not assembled: $live_path" >&2
        exit 1
    fi
done <<< "$LIVE_PATHS"

# Bump the deploy stamp in server.json (preserving the ws url): the pages
# use it to cache-bust the wasm bundles once per deploy. The stamp also
# records the protocol version this bundle speaks, so a bump is caught
# HERE — the moment it ships — rather than at the first failed join.
PROTO="$(grep -oE 'PROTO_VERSION: u16 = [0-9]+' crates/arena-core/src/proto.rs | grep -oE '[0-9]+$')"
# Fire carries its own version in its own crate, on purpose: bumping one game's
# protocol must never gate the other's join.
FIRE_PROTO="$(grep -oE 'PROTO_VERSION: u16 = [0-9]+' crates/fire-core/src/proto.rs | grep -oE '[0-9]+$')"
# Four Kings likewise: its own crate, its own number, its own server.json key.
KINGS_PROTO="$(grep -oE 'PROTO_VERSION: u16 = [0-9]+' crates/kings-core/src/proto.rs | grep -oE '[0-9]+$')"
echo "== shipping arena protocol v$PROTO, fire protocol v$FIRE_PROTO, kings protocol v$KINGS_PROTO =="
"$PY" - "$PAGES_DIR/server.json" "$PROTO" "$FIRE_PROTO" "$KINGS_PROTO" <<'EOF'
import json, os, sys, time
p, proto, fire_proto, kings_proto = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), int(sys.argv[4])


def die(msg):
    sys.stderr.write("deploy-pages: %s\n" % msg)
    raise SystemExit(1)


# FAIL CLOSED, the same rule publish-host.sh states: a book that will not parse
# is never overwritten. This used to start from `{}` on a parse error and push
# the result, which turns one bad byte on gh-pages — a hand edit, a badly
# resolved conflict now that several machines write the branch — into the
# silent loss of every host entry and every mirror. An empty file is the one
# legitimate `{}` start.
d = {}
if os.path.exists(p):
    with open(p, encoding="utf-8") as fh:
        text = fh.read().strip()
    if text:
        try:
            d = json.loads(text)
        except ValueError as e:
            die("%s exists but is not JSON (%s); refusing to overwrite it" % (p, e))
        if not isinstance(d, dict):
            die("%s is not a JSON object; refusing to overwrite it" % p)
was = d.get("proto")
was_fire = d.get("fire_proto")
was_kings = d.get("kings_proto")
d["v"] = str(int(time.time()))
d["proto"] = proto
d["fire_proto"] = fire_proto
d["kings_proto"] = kings_proto
# Temp file plus rename, so an interrupted write cannot leave a truncated book
# behind — which is one of the ways the unparseable book above gets made.
tmp = p + ".tmp"
with open(tmp, "w", encoding="utf-8") as fh:
    json.dump(d, fh)
os.replace(tmp, p)
if was_kings is not None and was_kings != kings_proto:
    print(f"""
!! KINGS PROTOCOL BUMP: v{was_kings} -> v{kings_proto}
!! kings-server speaks the OLD version until it is redeployed, and the join
!! gate is exact equality, so from now until `bash deploy/deploy-kings-online.sh`
!! runs (on the developer's PC, inside the claude-sdk WSL distro), players get:
!!     "this build speaks kings protocol v{kings_proto}, the live game is v{was_kings}"
!! The lobby LISTING keeps working at any version by design, so the browser
!! will show lobbies nobody can enter until the server catches up.
""")
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
!! running arena-server was built before v{proto}, players will be told
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
!! from the moment this page is live until arena-server is rebuilt from the
!! same commit, players get:
!!     "this build speaks protocol v{proto}, the live game is v{was}"
!! Redeploy arena-server in the SAME window. Archived pages stay frozen on
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
        # The worktree is detached, so name both ends of the refspec.
        git push origin HEAD:refs/heads/gh-pages
    fi
)
# No explicit `worktree remove` here: the EXIT trap above does it on every
# path, and a cleanup that only runs when nothing went wrong is the bug.
echo "== live at https://endersgamesdev.github.io/EmberEngine/ =="
