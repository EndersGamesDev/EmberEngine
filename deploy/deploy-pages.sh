#!/usr/bin/env bash
# Build and verify the wasm bundle and games hub without publishing a branch.
# Run from anywhere (git-bash): bash deploy/deploy-pages.sh
#
# GitHub Actions deploys Pages from the release asset selected by main;
# this script remains the local dry-run and byte-identity path for that asset.
# Set SOURCE_DATE_EPOCH to the release commit time for a reproducible stamp,
# EMBER_PAGES_ARCHIVE to write the assembled tree, and EMBER_PAGES_COMPARE to
# fail unless the assembled files are byte-identical to an existing archive.
#
# Server-build/workstation dry-run recipe:
#   cargo build --target wasm32-unknown-unknown --release -p fire -p arena -p kings -p league -p what-is-this -p ember-julibrot-app --lib
#   wasm-bindgen --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/fire.wasm
#   wasm-bindgen --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/arena.wasm
#   wasm-bindgen --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/kings.wasm
#   wasm-bindgen --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/league.wasm
#   wasm-bindgen --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/what_is_this.wasm
#   wasm-bindgen --target web --no-typescript --out-dir web/labs/julibrot/pkg target/wasm32-unknown-unknown/release/ember_lab_julibrot.wasm
# Copy web/pkg from the server into this checkout, then assemble without builds:
#   EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh
# Assemble the same tree as a release archive without committing or pushing:
#   EMBER_PAGES_ARCHIVE=ember-pages.tar.gz bash deploy/deploy-pages.sh
# Compare a clean rebuild with a downloaded release asset:
#   SOURCE_DATE_EPOCH=... EMBER_PAGES_COMPARE=ember-pages.tar.gz bash deploy/deploy-pages.sh
#
# Layout in the Pages release archive:
#   index.html            games hub (lobby showcase + catalog)
#   games.json            catalog — the newest version of each game is "live"
#   server.json           {ws, v} — current tunnel domain + deploy stamp
#   games/arena/v31/      live Killshot build — Breach-12 shotgun (page + frozen pkg)
#   games/arena/v0/       live arena v0 pong classic (page + frozen pkg)
#   games/fire/v2/        live fire racer build (castle circuit, online)
#   games/kings/v1/       live four kings build (2D page board + 3D wasm view, online)
#   games/league/v2/      live UltimateLegue build (selected from games.json)
#   games/league/v1/      frozen first UltimateLegue build
#   games/what-is-this/v1/ live browser and hardware diagnostic
#   labs/julibrot/        live four-dimensional slice viewer lab
#   games/pong/v1/        archived first web build (materialized from history)
#   games/fire/v1/        archived first fire build; already on the branch and
#                         deliberately never touched again — only $FIRE_LIVE is
#                         removed and rewritten below
#   pkg/                  legacy root bundle, kept fresh for old cached pages
set -euo pipefail

die() { echo "deploy-pages: $*" >&2; exit 1; }
# Windows puts an App Execution Alias stub named python3 on PATH: it prints
# "Python was not found" and exits without running anything, so `command -v`
# alone picks an interpreter that cannot execute a line. A candidate counts
# only once it has run one.
PY=""
for cand in python3 python; do
    c="$(command -v "$cand" 2>/dev/null)" || continue
    "$c" -c "pass" >/dev/null 2>&1 || continue
    PY="$c"
    break
done
[ -n "$PY" ] || die "need a working python3 or python on PATH"

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_DIR"
# gh-pages commit holding the original first web build (auto-run pong).
V1_COMMIT="e7b85e8"

# Release versions come from the packages being shipped. Validate every
# catalog entry before a build or archive, then use Arena's release major to
# select its stable vN directory. Other legacy slots remain explicit locators.
IFS=$'\t' read -r ARENA_LIVE LEAGUE_LIVE END_GAME_LIVE < <("$PY" - web/games.json <<'PY'
import json, pathlib, re, sys

catalog_path = pathlib.Path(sys.argv[1])
catalog = json.loads(catalog_path.read_text(encoding="utf-8"))
semantic = re.compile(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)")
manifests = {
    "arena": "crates/arena/Cargo.toml",
    "fire": "crates/fire/Cargo.toml",
    "kings": "crates/kings/Cargo.toml",
    "league": "crates/league/Cargo.toml",
    "what-is-this": "crates/what-is-this/Cargo.toml",
    "end-game": "crates/end-game/Cargo.toml",
    "julibrot": "crates/labs/julibrot/app/Cargo.toml",
}


def package_version(manifest):
    text = pathlib.Path(manifest).read_text(encoding="utf-8")
    found = re.findall(r'^version\s*=\s*"([^"]+)"\s*$', text, re.MULTILINE)
    if len(found) != 1 or semantic.fullmatch(found[0]) is None:
        raise SystemExit("FAILED: %s must declare one three-grade package version" % manifest)
    return found[0]


games = {game.get("id"): game for game in catalog.get("games", [])}
if len(games) != len(catalog.get("games", [])):
    raise SystemExit("FAILED: catalog game ids must be unique")
for game in catalog.get("games", []):
    for release in game.get("versions", []):
        if semantic.fullmatch(str(release.get("version", ""))) is None:
            raise SystemExit("FAILED: every catalog entry must carry a three-grade version")
for game_id, manifest in manifests.items():
    versions = games.get(game_id, {}).get("versions", [])
    live = [release for release in versions if release.get("live") is True]
    if len(live) != 1:
        raise SystemExit("FAILED: %s must select exactly one live release" % game_id)
    expected = package_version(manifest)
    if live[0]["version"] != expected:
        raise SystemExit("FAILED: %s live version must equal package version %s" % (game_id, expected))

# The catalog's `proto` is what the deploy-time host gate trusts, and what the
# hub reads to decide which page may receive a handed-over lobby. Nothing kept
# it honest: it is hand-edited beside a crate constant it must equal, and a
# catalog that says 1 while the crate says 2 sends a player to a page that
# cannot join and tells the gate the wrong protocol to look for. Both numbers
# answer the same question, so they are compared before anything is fetched or
# built.
#
# Both directions, and both derived from the tree rather than from a list kept
# here. A hardcoded tuple of game ids would leave a fifth server game gated by
# nothing at all — the gate skips any live entry with no `proto`, so a missing
# key would be silence on both sides rather than a failure.
proto_re = re.compile(r"PROTO_VERSION: u16 = (\d+)")
cores = {}
for source in sorted(pathlib.Path("crates").glob("*-core/src/proto.rs")):
    game_id = source.parent.parent.name[: -len("-core")]
    found = proto_re.search(source.read_text(encoding="utf-8"))
    if found is None:
        raise SystemExit("FAILED: %s declares no PROTO_VERSION" % source)
    cores[game_id] = int(found.group(1))

for game in catalog.get("games", []):
    game_id = game.get("id")
    if game.get("kind") == "lab":
        # A lab has no host, no protocol and no handover (docs/hosts.md §11).
        # Both readers ignore a `proto` here — the gate skips labs outright and
        # the hub never routes a lobby to one — so a number written here is
        # silently inert, which is the state a catalog edit is most likely to
        # leave behind and least likely to reveal.
        for release in game.get("versions", []):
            if release.get("proto") is not None:
                raise SystemExit("FAILED: %s is a lab and must declare no proto" % game_id)
        continue
    for release in game.get("versions", []):
        if release.get("live") is not True:
            continue
        declared = release.get("proto")
        if game_id in cores:
            # A live entry for a game with a protocol crate must carry that
            # crate's number — including when it carries none at all.
            if declared != cores[game_id]:
                raise SystemExit(
                    "FAILED: %s live catalog proto is %r but crates/%s-core/src/proto.rs declares %d"
                    % (game_id, declared, game_id, cores[game_id])
                )
        elif declared is not None:
            # And a number nothing in the tree can confirm is worse than none:
            # the gate would look for hosts on a protocol no crate defines.
            raise SystemExit(
                "FAILED: %s live catalog declares proto %r but there is no crates/%s-core/src/proto.rs to check it against"
                % (game_id, declared, game_id)
            )

arena = next(release for release in games["arena"]["versions"] if release.get("live") is True)
arena_major = arena["version"].split(".", 1)[0]
arena_path = "games/arena/v%s/" % arena_major
if arena.get("v") != "v%s" % arena_major or arena.get("path") != arena_path:
    raise SystemExit("FAILED: Arena release major must select its vN slot")
league = next(release for release in games["league"]["versions"] if release.get("live") is True)
if re.fullmatch(r"games/league/v[1-9][0-9]*/", league.get("path", "")) is None:
    raise SystemExit("FAILED: League catalog must select exactly one safe live version path")
end_game = next(release for release in games["end-game"]["versions"] if release.get("live") is True)
if re.fullmatch(r"games/end-game/v[1-9][0-9]*/", end_game.get("path", "")) is None:
    raise SystemExit("FAILED: End Game catalog must select exactly one safe live version path")
print(arena_path.rstrip("/") + "\t" + league["path"].rstrip("/") + "\t" + end_game["path"].rstrip("/"))
PY
)
ARENA_LIVE="${ARENA_LIVE//$'\r'/}"
LEAGUE_LIVE="${LEAGUE_LIVE//$'\r'/}"
END_GAME_LIVE="${END_GAME_LIVE//$'\r'/}"

if [ "${EMBER_PAGES_PREBUILT:-}" = 1 ]; then
    missing=()
    for bundle in fire arena kings league what_is_this end_game; do
        for artifact in "$bundle.js" "${bundle}_bg.wasm"; do
            [ -f "web/pkg/$artifact" ] || missing+=("web/pkg/$artifact")
        done
    done
    for artifact in ember_lab_julibrot.js ember_lab_julibrot_bg.wasm; do
        [ -f "web/labs/julibrot/pkg/$artifact" ] || missing+=("web/labs/julibrot/pkg/$artifact")
    done
    if [ "${#missing[@]}" -ne 0 ]; then
        echo "FAILED: EMBER_PAGES_PREBUILT=1 requires all six game bundles and the Julibrot lab bundle; missing:" >&2
        printf '  %s\n' "${missing[@]}" >&2
        exit 1
    fi
fi

echo "== stamping the build ticker =="
bash deploy/stamp-version.sh

if [ "${EMBER_PAGES_PREBUILT:-}" = 1 ]; then
    echo "== using six prebuilt game bundles from web/pkg and the Julibrot lab bundle =="
else
    echo "== building wasm =="
    cargo build --target wasm32-unknown-unknown --release -p fire --lib
    cargo build --target wasm32-unknown-unknown --release -p arena --lib
    cargo build --target wasm32-unknown-unknown --release -p kings --lib
    cargo build --target wasm32-unknown-unknown --release -p league --lib
    cargo build --target wasm32-unknown-unknown --release -p what-is-this --lib
    cargo build --target wasm32-unknown-unknown --release -p end-game --lib
    cargo build --target wasm32-unknown-unknown --release -p ember-julibrot-app --lib
    wasm-bindgen --target web --no-typescript --out-dir web/pkg \
        target/wasm32-unknown-unknown/release/fire.wasm
    wasm-bindgen --target web --no-typescript --out-dir web/pkg \
        target/wasm32-unknown-unknown/release/arena.wasm
    wasm-bindgen --target web --no-typescript --out-dir web/pkg \
        target/wasm32-unknown-unknown/release/kings.wasm
    wasm-bindgen --target web --no-typescript --out-dir web/pkg \
        target/wasm32-unknown-unknown/release/league.wasm
    wasm-bindgen --target web --no-typescript --out-dir web/pkg \
        target/wasm32-unknown-unknown/release/what_is_this.wasm
    wasm-bindgen --target web --no-typescript --out-dir web/pkg \
        target/wasm32-unknown-unknown/release/end_game.wasm
    wasm-bindgen --target web --no-typescript --out-dir web/labs/julibrot/pkg \
        target/wasm32-unknown-unknown/release/ember_lab_julibrot.wasm
fi

echo "== assembling the Pages release tree =="
# The retired gh-pages branch remains a read-only seed for frozen historical
# bundles that are not stored in source. The detached worktree makes it
# impossible for this local build path to move the branch it reads.
git fetch -q origin gh-pages \
    || { echo "FAILED: cannot fetch the legacy origin/gh-pages seed" >&2; exit 1; }
PAGES_DIR="$(mktemp -d -t ember-pages-XXXX)"
COMPARE_DIR=""
# Armed BEFORE the add, so neither a failing add nor anything after it can
# leave the directory registered as a worktree. Without this, one failed push
# left gh-pages checked out under /tmp and every later deploy — of the pages
# and of either game — died at its own `worktree add` until a human ran
# `git worktree remove`. The status is preserved: the trap reports the failure
# that caused it, not the cleanup's own.
# shellcheck disable=SC2154
trap 'st=$?; git worktree remove --force "$PAGES_DIR" >/dev/null 2>&1 || true; rm -rf "$PAGES_DIR"; [ -z "$COMPARE_DIR" ] || rm -rf "$COMPARE_DIR"; exit $st' EXIT
git worktree add -q --detach "$PAGES_DIR" FETCH_HEAD

# The legacy seed can contain an independently versioned Fire release. Never
# downgrade it while assembling a full site from older Fire source.
"$PY" - "$PAGES_DIR/games/fire/v2/release.json" "$REPO_DIR/crates/fire-core/src/proto.rs" <<'PY'
import json, pathlib, re, sys
release, source = map(pathlib.Path, sys.argv[1:])
if release.exists():
    shipped = json.loads(release.read_text(encoding="utf-8"))["protocol"]
    local = int(re.search(r"PROTO_VERSION: u16 = (\d+)", source.read_text(encoding="utf-8")).group(1))
    if shipped > local:
        raise SystemExit("FAILED: live Fire is newer than this source; preserve its release or integrate its source before a full Pages build")
PY

# Live version dirs (older versions stay frozen on the branch untouched).
ARENA_V0_LIVE="games/arena/v0"
FIRE_LIVE="games/fire/v2"
KINGS_LIVE="games/kings/v1"
WHAT_LIVE="games/what-is-this/v1"
LAB_JULIBROT_LIVE="labs/julibrot"

rm -rf "${PAGES_DIR:?}"/index.html "${PAGES_DIR:?}"/pkg \
    "${PAGES_DIR:?}/$ARENA_LIVE" "${PAGES_DIR:?}/$ARENA_V0_LIVE" "${PAGES_DIR:?}/$FIRE_LIVE" "${PAGES_DIR:?}/$KINGS_LIVE" "${PAGES_DIR:?}/$LEAGUE_LIVE" "${PAGES_DIR:?}/$WHAT_LIVE" "${PAGES_DIR:?}/$END_GAME_LIVE" \
    "${PAGES_DIR:?}/$LAB_JULIBROT_LIVE" \
    "${PAGES_DIR:?}"/games.json
mkdir -p "$PAGES_DIR/$ARENA_LIVE" "$PAGES_DIR/$ARENA_V0_LIVE" "$PAGES_DIR/$FIRE_LIVE" "$PAGES_DIR/$KINGS_LIVE" "$PAGES_DIR/$LEAGUE_LIVE" "$PAGES_DIR/$WHAT_LIVE" "$PAGES_DIR/$END_GAME_LIVE" "$PAGES_DIR/$LAB_JULIBROT_LIVE/pkg"
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
# The developer landing page (marketing): one static file with no build of its
# own. Guarded for the same reason as hosts.js: an older checkout must still
# deploy.
if [ -f web/engine.html ]; then
    cp web/engine.html "$PAGES_DIR"/
fi
cp "web/$ARENA_LIVE/index.html" "$PAGES_DIR/$ARENA_LIVE/"
cp "web/$ARENA_LIVE/settings.js" "$PAGES_DIR/$ARENA_LIVE/"
cp "web/$ARENA_V0_LIVE/index.html" "$PAGES_DIR/$ARENA_V0_LIVE/"
# Fire's page is a shell plus its own stylesheet and two modules. A static
# import is resolved before the importing module runs, so leaving one behind
# is a blank page, not a status line. Generated bindings still come from the
# verified root pkg below.
for name in index.html race.js garage.js style.css; do
    cp "web/$FIRE_LIVE/$name" "$PAGES_DIR/$FIRE_LIVE/"
done
# The page sets its own type. A face the assembly leaves behind is not an
# error anyone sees: the stylesheet falls through to the system stack and
# the page just looks wrong. OFL.txt travels with the files because the
# licence requires its notice to; README.md is a repository convention and
# stays out of the published tree.
mkdir -p "$PAGES_DIR/$FIRE_LIVE/fonts"
cp "web/$FIRE_LIVE"/fonts/*.woff2 "web/$FIRE_LIVE/fonts/OFL.txt" "$PAGES_DIR/$FIRE_LIVE/fonts/"
cp "web/$KINGS_LIVE/index.html" "$PAGES_DIR/$KINGS_LIVE/"
# Version-local UI, images and provenance sidecars are runtime assets too.
# Never reuse a version-local pkg: the tested bindings come from web/pkg below.
"$PY" - "web/$LEAGUE_LIVE" "$PAGES_DIR/$LEAGUE_LIVE" <<'PY'
import pathlib, shutil, sys
source, dest = map(pathlib.Path, sys.argv[1:])
if not (source / "index.html").is_file():
    raise SystemExit(f"FAILED: missing live League page: {(source / 'index.html').as_posix()}")
for parent in [source, *source.parents]:
    if parent.is_symlink():
        raise SystemExit(f"FAILED: League source contains a symlink: {parent}")
for path in sorted(source.rglob("*")):
    if path.is_symlink():
        raise SystemExit(f"FAILED: League source contains a symlink: {path}")
    relative = path.relative_to(source)
    if relative.parts[0] == "pkg" or relative.as_posix() == "version.json":
        continue
    target = dest / relative
    if path.is_dir():
        target.mkdir(parents=True, exist_ok=True)
    elif path.is_file():
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, target)
    else:
        raise SystemExit(f"FAILED: League source is not a regular file: {path}")
PY
cp web/version.json "$PAGES_DIR/$LEAGUE_LIVE/"
cp "web/$WHAT_LIVE/index.html" "$PAGES_DIR/$WHAT_LIVE/"
# End Game has version-local UI and media. Copy only its source assets here;
# generated bindings come from the verified root pkg below.
for name in index.html main.js quality.js style.css cover.png prologue.mp4 ambience.wav; do
    cp "web/$END_GAME_LIVE/$name" "$PAGES_DIR/$END_GAME_LIVE/"
done
cp web/version.json "$PAGES_DIR/$END_GAME_LIVE/"

# lab.js is not optional furniture: main.js imports it statically, so a deploy
# that omits it resolves the import to a missing file and the whole module graph
# fails to load — no page, no controls, and not even the page's own error
# handler, because the handler is inside the module that never ran. drive.html
# is the controls-free driver the lab's pixel proofs are taken through, and it
# is shipped for the same reason a proof is worth having: a measurement taken
# on a locally built copy is a measurement of a build nobody is serving.
cp "web/$LAB_JULIBROT_LIVE/index.html" "web/$LAB_JULIBROT_LIVE/main.js" \
    "web/$LAB_JULIBROT_LIVE/lab.js" "web/$LAB_JULIBROT_LIVE/drive.html" \
    "web/$LAB_JULIBROT_LIVE/worker.js" "web/$LAB_JULIBROT_LIVE/style.css" \
    "$PAGES_DIR/$LAB_JULIBROT_LIVE/"
cp "web/$LAB_JULIBROT_LIVE/pkg/ember_lab_julibrot.js" \
    "web/$LAB_JULIBROT_LIVE/pkg/ember_lab_julibrot_bg.wasm" \
    "$PAGES_DIR/$LAB_JULIBROT_LIVE/pkg/"
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
copy_pkg "$PAGES_DIR/$LEAGUE_LIVE/pkg" league
copy_pkg "$PAGES_DIR/$WHAT_LIVE/pkg" what_is_this
copy_pkg "$PAGES_DIR/$END_GAME_LIVE/pkg" end_game
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

# The catalog is the hub's promise. Refuse to archive a live link unless this
# assembly actually produced its page, so games.json and this script cannot
# silently drift apart again.
# Piped through tr: Python's stdout is a text stream, so on Windows every
# newline it prints leaves the pipe as CR LF and `read -r` keeps the CR in
# the value, which made this check reject a path it had just assembled.
LIVE_PATHS="$("$PY" -c 'import json, sys; d=json.load(open(sys.argv[1], encoding="utf-8")); print("\n".join(v["path"] for g in d["games"] for v in g["versions"] if v.get("live") is True))' web/games.json | tr -d '\r')"
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
LEAGUE_PROTO="$(grep -oE 'PROTO_VERSION: u16 = [0-9]+' crates/league-core/src/proto.rs | grep -oE '[0-9]+$')"
echo "== shipping arena protocol v$PROTO, fire protocol v$FIRE_PROTO, kings protocol v$KINGS_PROTO, league protocol v$LEAGUE_PROTO =="
"$PY" - "$PAGES_DIR/server.json" "$PROTO" "$FIRE_PROTO" "$KINGS_PROTO" "$LEAGUE_PROTO" <<'EOF'
import json, os, sys, time
p = sys.argv[1]
proto, fire_proto, kings_proto, league_proto = map(int, sys.argv[2:6])


def die(msg):
    sys.stderr.write("deploy-pages: %s\n" % msg)
    raise SystemExit(1)


# FAIL CLOSED, the same rule publish-host.sh states: a book that will not parse
# is never overwritten. This used to start from `{}` on a parse error and push
# the result, which turns one bad byte in the legacy seed into the
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
was_league = d.get("league_proto")
d["v"] = str(int(time.time()))
d["proto"] = proto
d["fire_proto"] = fire_proto
d["kings_proto"] = kings_proto
d["league_proto"] = league_proto
# Temp file plus rename, so an interrupted write cannot leave a truncated book
# behind — which is one of the ways the unparseable book above gets made.
tmp = p + ".tmp"
with open(tmp, "w", encoding="utf-8") as fh:
    json.dump(d, fh)
os.replace(tmp, p)
if was_league is not None and was_league != league_proto:
    print(f"""
!! LEAGUE PROTOCOL BUMP: v{was_league} -> v{league_proto}
!! league-server must be rebuilt and restarted with the same protocol before
!! players can create or join a match. The lobby listing stays available to
!! older browsers, but the game join gate requires exact equality.
""")
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
!! NO PREVIOUS PROTOCOL RECORDED in the Pages seed, so this build
!! cannot be compared against the last one. It ships v{proto}. If the
!! running arena-server was built before v{proto}, players will be told
!! "this build speaks protocol v{proto}, the live game is v<older>" and
!! cannot create or join. Check the server's build before announcing.
!! (A freshly seeded release archive lands here once; the next build has
!! a baseline and compares normally.)
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

# Mirror bindings come from SOURCE, not from the frozen seed. The served book
# is frozen between releases and no workflow deploys on a push to gh-pages, so
# a binding that exists only on that branch reaches no player; meanwhile a
# quick tunnel rotates on a running host and takes its game offline until the
# next release. The bound mirror is the documented answer to exactly that
# (docs/hosts.md §3) and it is useless if the binding itself cannot be changed
# through the publication path.
#
# A name this file declares wins, because otherwise a wrong URL frozen into the
# seed could never be corrected from source, which is the whole point. A name
# only the seed carries is kept, because dropping a binding nobody asked about
# would silently unpublish a host.
MIRRORS_SRC="$REPO_DIR/web/mirrors.json"
if [ -f "$MIRRORS_SRC" ]; then
    "$PY" - "$PAGES_DIR/server.json" "$MIRRORS_SRC" <<'PY'
import json, os, re, sys

book_path, src_path = sys.argv[1], sys.argv[2]
NAME = re.compile(r"^[a-z0-9-]{3,32}$")


def die(msg):
    sys.stderr.write("deploy-pages: %s\n" % msg)
    raise SystemExit(1)


with open(src_path, encoding="utf-8") as fh:
    text = fh.read().strip()
# An empty list is a legitimate "this release binds no mirror". Anything that
# is not a list of valid bindings is a typo in the one file that decides which
# third-party URLs the pages will fetch, and is refused rather than skipped.
try:
    source = json.loads(text) if text else []
except ValueError as e:
    die("%s is not JSON (%s)" % (src_path, e))
if not isinstance(source, list):
    die("%s must be a list of {name, url} bindings" % src_path)
bindings = []
seen = set()
for item in source:
    if not isinstance(item, dict):
        die("%s has an entry that is not an object" % src_path)
    name, url = item.get("name"), item.get("url")
    if not isinstance(name, str) or NAME.match(name) is None:
        die("%s binds a mirror to %r, which is not a host name" % (src_path, name))
    if not isinstance(url, str) or not url.startswith(("https://", "http://")):
        die("%s binds %s to %r, which is not a URL" % (src_path, name, url))
    # One name, one binding. Two entries for a name mean the writer meant one
    # of them, and publishing both would have every page fetch a URL nobody
    # chose — the merge below would keep whichever it saw last.
    if name in seen:
        die("%s binds %s twice; a host has one mirror" % (src_path, name))
    seen.add(name)
    bindings.append({"url": url, "name": name})

with open(book_path, encoding="utf-8") as fh:
    book = json.load(fh)
listed = book.get("mirrors")
merged = [m for m in listed if isinstance(m, dict)] if isinstance(listed, list) else []
by_name = {}
for index, entry in enumerate(merged):
    if isinstance(entry.get("name"), str):
        by_name[entry["name"]] = index
for binding in bindings:
    if binding["name"] in by_name:
        merged[by_name[binding["name"]]] = binding
    else:
        by_name[binding["name"]] = len(merged)
        merged.append(binding)
book["mirrors"] = merged

# A bound name is served from its mirror, so the book's own copy of that
# entry has to go. `mergeBook` gives `hosts[]` precedence over any mirror of
# the same name, and the assembled book inherits `hosts[]` from the frozen
# seed untouched — so leaving the entry in place makes the binding a no-op,
# and the mirror that exists precisely to carry a rotated address is never
# read. That was the whole failure this file was added to fix.
#
# The drop set comes from the MERGED list, not from this file's own bindings.
# A binding the seed already carries is the previous release's declaration
# that the same host is served from a mirror, made through this same path and
# reviewed the same way; it is not weaker evidence for being a release older.
# Taking only the new bindings would leave every previously bound host
# shadowed by its own stale entry, which is the same bug one release removed.
hosts = book.get("hosts")
if isinstance(hosts, list) and merged:
    bound = {m["name"] for m in merged if isinstance(m.get("name"), str)}
    kept = [h for h in hosts if not (isinstance(h, dict) and h.get("name") in bound)]
    dropped = len(hosts) - len(kept)
    if dropped:
        book["hosts"] = kept
        print("   dropped %d seed host entry(s) now served from a mirror" % dropped)

tmp = book_path + ".tmp"
with open(tmp, "w", encoding="utf-8") as fh:
    json.dump(book, fh)
os.replace(tmp, book_path)
print("   bound mirrors: " + (", ".join(m["name"] for m in merged) or "none"))
PY
else
    echo "   note: web/mirrors.json does not exist in this checkout; the seed's bindings stand"
fi

# The top-level protocol keys just moved, and the legacy top-level ADDRESS
# keys are defined against them: `ws` must name a host that speaks the
# protocol the pages now ship. Recompute them from the host list immediately,
# so a bump re-points `ws` at a host that already speaks the new version
# instead of leaving every frozen and live page on a host they can no longer
# join until somebody redeploys a server.
bash "$REPO_DIR/deploy/publish-host.sh" --book "$PAGES_DIR/server.json" --recompute

# The checked-in lab loader stays pinned at v=1 for its page contract. Only
# assembled copies receive the final deployment stamp after address recompute.
DEPLOY_STAMP="$("$PY" -c 'import json,sys; print(json.load(open(sys.argv[1], encoding="utf-8"))["v"])' "$PAGES_DIR/server.json")"
# Arena's menu and WASM share a revisioned settings contract. Ship and stamp
# the menu beside its own versioned page, never mutate archived releases.
"$PY" - "$PAGES_DIR/$ARENA_LIVE/index.html" "$DEPLOY_STAMP" <<'PY'
import pathlib, re, sys
p = pathlib.Path(sys.argv[1])
text = p.read_text(encoding="utf-8")
token = re.compile(r"\./settings\.js\?v=1(?=[\"'])")
if len(token.findall(text)) != 1:
    raise SystemExit("FAILED: Arena settings cache key must occur exactly once")
with open(p, "w", encoding="utf-8", newline="") as fh:
    fh.write(token.sub(lambda _: './settings.js?v=' + sys.argv[2], text))
PY
for loader in index.html main.js lab.js drive.html worker.js; do
    assembled="$PAGES_DIR/$LAB_JULIBROT_LIVE/$loader"
    if ! grep -qE '\?v=1([^0-9]|$)' "$assembled"; then
        echo "FAILED: Julibrot cache key rewrite matched no ?v=1 token in $loader" >&2
        exit 1
    fi
    "$PY" - "$assembled" "$DEPLOY_STAMP" <<'PY'
import pathlib, re, sys
p = pathlib.Path(sys.argv[1])
text = p.read_text(encoding="utf-8")
stamped = re.sub(r"\?v=1(?![0-9])", "?v=" + sys.argv[2], text)
with open(p, "w", encoding="utf-8", newline="") as fh:
    fh.write(stamped)
PY
    if grep -qE '\?v=1([^0-9]|$)' "$assembled"; then
        echo "FAILED: Julibrot cache key rewrite left ?v=1 in $loader" >&2
        exit 1
    fi
done

# A module the assembly does not place is a page that never runs. A static
# import is resolved before any code in the importing module executes, so a
# missing target is not a caught error reported in the status line: it is a
# blank page with a console entry nobody is watching. The list of files to copy
# and the list of files the pages reference are two lists that drifted apart
# once and would drift again, so the second is read out of the assembled pages
# themselves and checked against what is on disk beside them.
"$PY" - "$PAGES_DIR/$LAB_JULIBROT_LIVE" <<'PY'
import pathlib, re, sys

root = pathlib.Path(sys.argv[1])
reference = re.compile(
    r"""(?:from|import)\s*\(?\s*["'](\.{1,2}/[^"'?]+)"""
    r"""|(?:src|href)\s*=\s*["'](\.{1,2}/[^"'?]+)"""
)
missing = []
for page in sorted(root.rglob("*")):
    if not page.is_file() or page.suffix not in {".js", ".html"}:
        continue
    text = page.read_text(encoding="utf-8")
    for found in reference.finditer(text):
        target = found.group(1) or found.group(2)
        if not (page.parent / target).is_file():
            missing.append(f"{page.relative_to(root)} references {target}")
if missing:
    print("FAILED: the assembled lab references files the deploy does not ship:", file=sys.stderr)
    for entry in missing:
        print(f"  {entry}", file=sys.stderr)
    sys.exit(1)
PY

if [ -n "${EMBER_PAGES_COMPARE:-}" ]; then
    case "$EMBER_PAGES_COMPARE" in
        /*) COMPARE_PATH="$EMBER_PAGES_COMPARE" ;;
        *) COMPARE_PATH="$REPO_DIR/$EMBER_PAGES_COMPARE" ;;
    esac
    [ -f "$COMPARE_PATH" ] || die "comparison archive does not exist: $COMPARE_PATH"
    COMPARE_DIR="$(mktemp -d -t ember-pages-compare-XXXX)"
    tar -xzf "$COMPARE_PATH" -C "$COMPARE_DIR"
    if ! diff -qr --exclude=.git "$COMPARE_DIR" "$PAGES_DIR"; then
        die "assembled tree is not byte-identical to $COMPARE_PATH"
    fi
    rm -rf "$COMPARE_DIR"
    COMPARE_DIR=""
    echo "== assembled tree is byte-identical to $COMPARE_PATH =="
fi

if [ -n "${EMBER_PAGES_ARCHIVE:-}" ]; then
    case "$EMBER_PAGES_ARCHIVE" in
        /*) ARCHIVE_PATH="$EMBER_PAGES_ARCHIVE" ;;
        *) ARCHIVE_PATH="$REPO_DIR/$EMBER_PAGES_ARCHIVE" ;;
    esac
    mkdir -p "$(dirname "$ARCHIVE_PATH")"
    tar --exclude='./.git' -czf "$ARCHIVE_PATH" -C "$PAGES_DIR" .
    echo "== assembled release archive at $ARCHIVE_PATH; no branch was published =="
fi

if [ -z "${EMBER_PAGES_ARCHIVE:-}" ] && [ -z "${EMBER_PAGES_COMPARE:-}" ]; then
    echo "== local dry-run complete; no branch or release asset was published =="
fi
