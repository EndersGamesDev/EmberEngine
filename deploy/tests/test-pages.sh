#!/usr/bin/env bash
# deploy-pages.sh against cargo, wasm-bindgen and git shims.
#
#   bash deploy/tests/test-pages.sh
#
# Nothing here contacts a network, compiles wasm or pushes a branch. The test
# extracts the release archive and inspects the exact assembled Pages tree.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
DEPLOY="$(cd "$HERE/.." && pwd)"
# shellcheck source=deploy/tests/lib.sh
. "$HERE/lib.sh"

# Exercise the production live slot without freezing a second release number
# into the fixture. The deploy derives Arena's directory from the full catalog
# version, so the fixture reads the same public data and checks the safe shape.
ARENA_LIVE="$("$PY" -c 'import json,sys; d=json.load(open(sys.argv[1], encoding="utf-8")); print(next(v["path"].rstrip("/") for g in d["games"] if g["id"] == "arena" for v in g["versions"] if v.get("live") is True))' "$DEPLOY/../web/games.json" | tr -d '\r')"
if [[ ! "$ARENA_LIVE" =~ ^games/arena/v[0-9]+$ ]]; then
    echo "pages fixture: cannot determine deploy-pages.sh's live arena path" >&2
    exit 1
fi
LEAGUE_LIVE="$("$PY" -c 'import json,sys; d=json.load(open(sys.argv[1], encoding="utf-8")); print(next(v["path"].rstrip("/") for g in d["games"] if g["id"] == "league" for v in g["versions"] if v.get("live") is True))' "$DEPLOY/../web/games.json" | tr -d '\r')"
if [[ ! "$LEAGUE_LIVE" =~ ^games/league/v[1-9][0-9]*$ ]]; then
    echo "pages fixture: cannot determine the catalog's live League path" >&2
    exit 1
fi

END_GAME_LIVE="$("$PY" -c 'import json,sys; d=json.load(open(sys.argv[1], encoding="utf-8")); print(next(v["path"].rstrip("/") for g in d["games"] if g["id"] == "end-game" for v in g["versions"] if v.get("live") is True))' "$DEPLOY/../web/games.json" | tr -d '\r')"
[[ "$END_GAME_LIVE" =~ ^games/end-game/v[1-9][0-9]*$ ]] || exit 1

# deploy-pages.sh refuses a catalog whose live `proto` disagrees with the crate
# constant, so the fixture's proto.rs files are written FROM the catalog rather
# than frozen beside it. Freezing them would make this suite fail on the day a
# game legitimately bumps its protocol, which is the day it is most needed. The
# seed then sits one below each, so the bump-warning paths still fire and their
# assertions stay honest about which numbers they saw.
live_proto() {
    "$PY" -c 'import json,sys; d=json.load(open(sys.argv[1], encoding="utf-8")); print(next(v["proto"] for g in d["games"] if g["id"] == sys.argv[2] for v in g["versions"] if v.get("live") is True))' "$DEPLOY/../web/games.json" "$1" | tr -d '\r'
}
ARENA_PROTO="$(live_proto arena)"
FIRE_PROTO="$(live_proto fire)"
KINGS_PROTO="$(live_proto kings)"
LEAGUE_PROTO="$(live_proto league)"
for value in "$ARENA_PROTO" "$FIRE_PROTO" "$KINGS_PROTO" "$LEAGUE_PROTO"; do
    if [[ ! "$value" =~ ^[0-9]+$ ]]; then
        echo "pages fixture: cannot read a live catalog protocol" >&2
        exit 1
    fi
done
ARENA_WAS="$((ARENA_PROTO - 1))"
LEAGUE_WAS="$((LEAGUE_PROTO - 1))"

TMP="$(mktemp -d -t ember-pagestest-XXXXXX)"
trap 'rm -rf "$TMP"' EXIT
REPO="$TMP/repo"
SEED="$TMP/pages-seed"
EXPECTED="$TMP/expected"
export SHIM_PUBLISHED="$TMP/published"
export SHIM_GIT_INDEX="$TMP/git-index"
export SHIM_GIT_TRACKED="$TMP/git-tracked"
export SHIM_LOG="$TMP/argv.log"
# Carries a loader import of its own, so the stamp pass accepts it and the copy
# order is the only thing that can keep it out of the publication.
# The mark is what the assembled bytes are searched for, and it carries no
# loader token: the stamp pass rewrites every `loader.js?v=` it finds in the
# published tree, so a needle containing one could never match and the search
# would report success whatever the copy order did.
TRACKED_RACE_DECOY_MARK='tracked race.js that must never reach the publication'
TRACKED_RACE_DECOY="import \"../../../loader.js?v=1\"; /* $TRACKED_RACE_DECOY_MARK */"
unset CARGO_TARGET_DIR

SHIMS="$TMP/shims"
mkdir -p "$SHIMS"
for shim in cargo wasm-bindgen git node npm npx; do
    cp "$HERE/shims/$shim" "$SHIMS/$shim"
    chmod +x "$SHIMS/$shim"
done
ln -s "$PY" "$SHIMS/python3"
ln -s "$PY" "$SHIMS/python"
export PATH="$SHIMS:$PATH"

mkdir -p "$REPO/deploy" "$REPO/web/$ARENA_LIVE" "$REPO/web/games/arena/v0"
mkdir -p "$REPO/web/games/fire/v2" "$REPO/web/games/kings/v1" "$REPO/web/games/what-is-this/v1"
mkdir -p "$REPO/web/games/league/v2/art"
mkdir -p "$REPO/web/$LEAGUE_LIVE/art/nested" "$REPO/web/$LEAGUE_LIVE/pkg"
mkdir -p "$REPO/web/labs/julibrot/pkg"
mkdir -p "$REPO/crates/arena-core/src" "$REPO/crates/fire-core/src" "$REPO/crates/kings-core/src"
mkdir -p "$REPO/crates/league-core/src"
mkdir -p "$REPO/crates/arena" "$REPO/crates/fire" "$REPO/crates/kings" "$REPO/crates/league"
mkdir -p "$REPO/crates/end-game" "$REPO/web/$END_GAME_LIVE"
mkdir -p "$REPO/crates/what-is-this" "$REPO/crates/labs/julibrot/app"
cp "$DEPLOY/deploy-pages.sh" "$DEPLOY/stage-wasm-types.sh" "$DEPLOY/stamp-version.sh" "$DEPLOY/publish-host.sh" "$DEPLOY/check-toolchain.sh" "$DEPLOY/verify-emitted-modules.py" "$REPO/deploy/"
cp "$DEPLOY/../Cargo.lock" "$REPO/"
cp "$DEPLOY/../package.json" "$DEPLOY/../package-lock.json" "$DEPLOY/../tsconfig.web.json" "$REPO/"
for manifest in arena fire kings league what-is-this end-game; do
    cp "$DEPLOY/../crates/$manifest/Cargo.toml" "$REPO/crates/$manifest/"
done
cp "$DEPLOY/../crates/labs/julibrot/app/Cargo.toml" "$REPO/crates/labs/julibrot/app/"
# Give recompute a deterministic stamp distinct from deploy-pages.sh's first
# stamp, so the test proves loaders read server.json only after recompute.
"$PY" - "$REPO/deploy/publish-host.sh" <<'PY'
import pathlib, sys
p = pathlib.Path(sys.argv[1])
text = p.read_text(encoding="utf-8")
old = 'doc["v"] = str(int(os.environ.get("SOURCE_DATE_EPOCH", time.time())))'
assert text.count(old) == 1
with open(p, "w", encoding="utf-8", newline="") as fh:
    fh.write(text.replace(old, 'doc["v"] = "recomputed-stamp"'))
PY
cp "$DEPLOY/../web/games.json" "$REPO/web/games.json"
printf '[{"name":"lundi","url":"https://source.example/lundi.json"}]\n' > "$REPO/web/mirrors.json"
printf 'root hosts test template\n' > "$REPO/web/hosts.test.mts.j2"
printf 'hub\n' > "$REPO/web/index.html"
printf 'export const hostsTemplate = true;\n' > "$REPO/web/hosts.ts.j2"
printf 'export function loaderTemplate() {}\n' > "$REPO/web/loader.ts.j2"
printf '{}\n' > "$REPO/web/version.json"
printf 'arena live<script src="./settings.js?v=1"></script><script type="module">import { emberLoad } from "../../../loader.js?v=1";</script>\n' > "$REPO/web/$ARENA_LIVE/index.html"
printf 'arena controls\n' > "$REPO/web/$ARENA_LIVE/settings.js"
printf 'arena v0\n' > "$REPO/web/games/arena/v0/index.html"
for name in index.html style.css; do
    printf "fire v2 fixture %s\n" "$name" > "$REPO/web/games/fire/v2/$name"
done
printf '<script type="module" src="./race.js"></script>\n' >> "$REPO/web/games/fire/v2/index.html"
printf 'import "./garage.js"; const LOADER_URL = "../../../loader.js?v=1"; export { LOADER_URL };\n' > "$REPO/web/games/fire/v2/race.ts.j2"
printf 'export const garage = true;\n' > "$REPO/web/games/fire/v2/garage.ts.j2"
# A tracked page file whose name collides with an emitted module. The live-page
# copy places it; only the copy order decides whether compiler output survives.
printf '%s\n' "$TRACKED_RACE_DECOY" > "$REPO/web/games/fire/v2/race.js"
mkdir -p "$REPO/web/games/fire/v2/fonts"
for name in barlow-latin-400.woff2 barlow-condensed-latin-800.woff2 OFL.txt README.md; do
    printf "fire v2 font fixture %s\n" "$name" > "$REPO/web/games/fire/v2/fonts/$name"
done
printf 'kings v1<script type="module">import { emberLoad } from "../../../loader.js?v=1";</script>\n' > "$REPO/web/games/kings/v1/index.html"
printf '<link href="./ui.css"><script src="./ui.js"></script><img src="../v2/art/arena.webp">\n' > "$REPO/web/$LEAGUE_LIVE/index.html"
printf 'import { emberLoad } from "../../../loader.js?v=1"; const ART = ["../v2/art/swarm.webp", "../v2/art/emberknight.webp", "../v2/art/hallow.webp", "../v2/art/bogmaw.webp", "../v2/art/tessera.webp"]; const ICONS = "../v2/art/icons.svg";\n' > "$REPO/web/$LEAGUE_LIVE/ui.js"
printf 'league CSS\n' > "$REPO/web/$LEAGUE_LIVE/ui.css"
printf 'league portrait bytes\n' > "$REPO/web/$LEAGUE_LIVE/art/swarm.webp"
printf '{"source":"fleet"}\n' > "$REPO/web/$LEAGUE_LIVE/art/nested/manifest.json"
for name in arena.webp swarm.webp emberknight.webp hallow.webp bogmaw.webp tessera.webp icons.svg; do
    printf "shared League art fixture %s\n" "$name" > "$REPO/web/games/league/v2/art/$name"
done
printf 'stale source bundle\n' > "$REPO/web/$LEAGUE_LIVE/pkg/arena.js"
printf 'stale source stamp\n' > "$REPO/web/$LEAGUE_LIVE/version.json"
printf 'what is this v1<script type="module">import { emberLoad } from "../../../loader.js?v=1";</script>\n' > "$REPO/web/games/what-is-this/v1/index.html"
for name in index.html main.js quality.js style.css cover.png prologue.mp4 ambience.wav; do
    printf "End Game fixture %s\n" "$name" > "$REPO/web/$END_GAME_LIVE/$name"
done
cat > "$REPO/web/$END_GAME_LIVE/main.js" <<'JS'
import { Quality } from './quality.js';
import { VoiceAudio } from './dialogue.js';
import { CastleDialogue } from './castle-audio.js';
import { CASTLE_LINES } from './voice-lines.js';
import { renderCastle } from './castle-ui.js';
import { emberLoad } from '../../../loader.js?v=1';
JS
printf 'export class VoiceAudio {}\n' > "$REPO/web/$END_GAME_LIVE/dialogue.js"
printf 'export class CastleDialogue {}\n' > "$REPO/web/$END_GAME_LIVE/castle-audio.js"
printf 'export const CASTLE_LINES = [];\n' > "$REPO/web/$END_GAME_LIVE/voice-lines.js"
printf 'export function renderCastle() {}\n' > "$REPO/web/$END_GAME_LIVE/castle-ui.js"
for name in boss-defeat.wav boss-intro.wav boss-phase2.wav castle-ambience.wav escape-clue.wav escape-ending.wav warden-death.wav warden-movement.wav warden-sword.wav warden-unlocking.wav; do
    printf "End Game fixture %s\n" "$name" > "$REPO/web/$END_GAME_LIVE/$name"
done
printf '<link href="./style.css?v=1"><script type="module" src="./main.js?v=1"></script>\n' > "$REPO/web/labs/julibrot/index.html"
# main.js imports lab.js statically, exactly as the shipped page does: the
# fixture has to carry the same import for the assembly check to mean anything.
printf 'import { openLab } from "./lab.js?v=1"; fetch("./future.js?v=10");\n' > "$REPO/web/labs/julibrot/main.js"
printf 'globalThis.JULIBROT_WORKER_URL = "./worker.js?v=1"; import("./pkg/ember_lab_julibrot.js?v=1"); fetch("./pkg/ember_lab_julibrot_bg.wasm?v=1"); export function openLab() {}\n' > "$REPO/web/labs/julibrot/lab.js"
printf '<canvas id="julibrot"></canvas><script type="module">import { openLab } from "./lab.js?v=1";</script>\n' > "$REPO/web/labs/julibrot/drive.html"
printf 'import("./pkg/ember_lab_julibrot.js?v=1"); fetch("./pkg/ember_lab_julibrot_bg.wasm?v=1");\n' > "$REPO/web/labs/julibrot/worker.js"
printf '<script type="module">import { openLab } from "./lab.js?v=1";</script>\n' > "$REPO/web/labs/julibrot/whole-grid-oracle.html"
printf '{"fixture":"whole-grid"}\n' > "$REPO/web/labs/julibrot/whole-grid-v1.json"
printf 'compressed whole-grid fixture\n' > "$REPO/web/labs/julibrot/whole-grid-v1.rgba.deflate.bin"
printf 'julibrot style\n' > "$REPO/web/labs/julibrot/style.css"
printf 'pub const PROTO_VERSION: u16 = %s;\n' "$ARENA_PROTO" > "$REPO/crates/arena-core/src/proto.rs"
printf 'pub const PROTO_VERSION: u16 = %s;\n' "$FIRE_PROTO" > "$REPO/crates/fire-core/src/proto.rs"
printf 'pub const PROTO_VERSION: u16 = %s;\n' "$KINGS_PROTO" > "$REPO/crates/kings-core/src/proto.rs"
printf 'pub const PROTO_VERSION: u16 = %s;\n' "$LEAGUE_PROTO" > "$REPO/crates/league-core/src/proto.rs"

# Snapshot the fixture's tracked files before adding scratch state. The git
# shim answers ls-files from this manifest, so source-copy tests distinguish
# repository inputs from merely present files.
(cd "$REPO" && find . -type f -printf '%P\n' | LC_ALL=C sort) > "$SHIM_GIT_TRACKED"
printf 'untracked scratch\n' > "$REPO/web/$END_GAME_LIVE/untracked.tmp"

mkdir -p "$SEED/games/arena/v0/pkg" "$SEED/games/arena/v17" "$SEED/games/fire/v1" "$SEED/games/kings/old" "$SEED/games/pong/v1/pkg"
mkdir -p "$SEED/games/league/old" "$SEED/games/league/v1/pkg" "$SEED/$LEAGUE_LIVE/pkg"
printf 'keep arena\n' > "$SEED/games/arena/v17/frozen.txt"
printf 'archived arena<script src="./settings.js?v=archived"></script>\n' > "$SEED/games/arena/v17/index.html"
printf 'archived controls\n' > "$SEED/games/arena/v17/settings.js"
printf 'seed arena v0 page\n' > "$SEED/games/arena/v0/index.html"
printf 'seed arena v0 JavaScript\n' > "$SEED/games/arena/v0/pkg/arena.js"
printf 'seed arena v0 wasm bytes\000\377\n' > "$SEED/games/arena/v0/pkg/arena_bg.wasm"
printf 'keep fire\n' > "$SEED/games/fire/v1/frozen.txt"
printf 'keep kings\n' > "$SEED/games/kings/old/frozen.txt"
printf 'keep league archive\n' > "$SEED/games/league/old/frozen.txt"
printf 'frozen league v1\n' > "$SEED/games/league/v1/index.html"
printf 'seed-only League asset\n' > "$SEED/games/league/v1/seed-only.webp"
printf 'frozen league v1 js\n' > "$SEED/games/league/v1/pkg/league.js"
printf 'frozen league v1 wasm\n' > "$SEED/games/league/v1/pkg/league_bg.wasm"
printf 'stale foreign bundle\n' > "$SEED/$LEAGUE_LIVE/pkg/arena.js"
printf 'stale live asset\n' > "$SEED/$LEAGUE_LIVE/obsolete.css"
printf 'frozen pong\n' > "$SEED/games/pong/v1/index.html"
printf 'frozen pong js\n' > "$SEED/games/pong/v1/pkg/pong.js"
printf 'frozen pong wasm\n' > "$SEED/games/pong/v1/pkg/pong_bg.wasm"
printf '{"v":"seed","proto":%s,"ws":"wss://old.example","league_proto":%s,"league_ws":"wss://old-league.example","mirrors":[{"url":"https://seed.example/quiet-egret.json","name":"quiet-egret"},{"url":"https://stale.example/lundi.json","name":"lundi"}],"hosts":[{"name":"new-host","ws":"wss://new.example","proto":%s,"version":"r2","league_ws":"wss://new-league.example","league_proto":%s},{"name":"lundi","ws":"wss://stale-lundi.example","proto":%s,"version":"r1"},{"name":"quiet-egret","ws":"wss://stale-egret.example","proto":%s,"version":"r1"}]}\n' \
    "$ARENA_WAS" "$LEAGUE_WAS" "$ARENA_PROTO" "$LEAGUE_PROTO" "$ARENA_PROTO" "$ARENA_PROTO" > "$SEED/server.json"
mkdir -p "$SEED/games/end-game/v1/pkg"
printf 'original end game page\n' > "$SEED/games/end-game/v1/index.html"
printf 'original end game bundle\n' > "$SEED/games/end-game/v1/pkg/end_game_bg.wasm"
export SHIM_PAGES_SEED="$SEED"

echo "== ordinary build assembles all live games and the Julibrot lab =="
: > "$SHIM_LOG"
rm -f "$SHIM_GIT_INDEX"
ARCHIVE="$TMP/ember-pages.tar.gz"
if (cd "$REPO" && SOURCE_DATE_EPOCH=1700000000 EMBER_PAGES_ARCHIVE="$ARCHIVE" bash deploy/deploy-pages.sh) > "$TMP/build.log" 2>&1; then
    ok "the shimmed build-and-archive run succeeded"
else
    bad "the shimmed build-and-archive run failed"
    tail -40 "$TMP/build.log" >&2
fi
mkdir -p "$SHIM_PUBLISHED"
tar -xzf "$ARCHIVE" -C "$SHIM_PUBLISHED"
if grep -q '^git \[push\]' "$SHIM_LOG"; then bad "the archive build attempted a branch push"; else ok "the archive build attempted no branch push"; fi
ARGV="$(cat "$SHIM_LOG")"
contains "$ARGV" "cargo [build] [--target] [wasm32-unknown-unknown] [--release] [-p] [what-is-this] [--lib]" "what-is-this is built as a wasm library"
contains "$ARGV" "[target/wasm32-unknown-unknown/release/what_is_this.wasm]" "what-is-this uses Cargo's default target directory"
contains "$ARGV" "cargo [build] [--target] [wasm32-unknown-unknown] [--release] [-p] [league] [--lib]" "League is built as a wasm library"
contains "$ARGV" "release/league.wasm" "League is passed to wasm-bindgen"
contains "$ARGV" "cargo [build] [--target] [wasm32-unknown-unknown] [--release] [-p] [ember-julibrot-app] [--lib]" "Julibrot is built as a wasm library"
contains "$ARGV" "[--out-dir] [web/labs/julibrot/pkg]" "Julibrot wasm-bindgen output stays in the lab"
contains "$ARGV" "release/ember_lab_julibrot.wasm" "Julibrot artifact is passed to wasm-bindgen"
contains "$ARGV" "cargo [run] [--locked] [-p] [ember-webgen] [--release] [--] [--out] [target/web-generated/deadbee]" "the generator receives the checkout source identifier"
contains "$ARGV" "npx [--no-install] [tsc] [-p] [tsconfig.web.json]" "the pinned TypeScript compiler emits the browser modules"
contains "$(cat "$TMP/build.log")" "TIMING ember-webgen wall=" "the generator wall time is recorded"
contains "$(cat "$TMP/build.log")" "TIMING typescript wall=" "the TypeScript wall time is recorded"
if grep -Fq '[--no-typescript]' "$SHIM_LOG"; then bad "wasm-bindgen declarations were disabled"; else ok "every wasm-bindgen invocation emits declarations"; fi
for bundle in fire arena kings league what_is_this end_game ember_loader; do
    for declaration in "$bundle.d.ts" "${bundle}_bg.wasm.d.ts"; do
        if [ -f "$REPO/target/web-generated/deadbee/pkg-types/$bundle/$declaration" ]; then ok "staged $bundle declaration $declaration"; else bad "missing staged $bundle declaration $declaration"; fi
    done
done
for declaration in ember_lab_julibrot.d.ts ember_lab_julibrot_bg.wasm.d.ts; do
    if [ -f "$REPO/target/web-generated/deadbee/pkg-types/ember_lab_julibrot/$declaration" ]; then ok "staged Julibrot declaration $declaration"; else bad "missing staged Julibrot declaration $declaration"; fi
done
if find "$SHIM_PUBLISHED" -type f -name '*.d.ts' -print -quit | grep -q .; then bad "the Pages tree contains compiler declarations"; else ok "the Pages tree excludes compiler declarations"; fi
if [ -e "$REPO/target/wasm32-unknown-unknown" ]; then bad "the default-target fixture made an unexpected Cargo target entry"; else ok "the default-target build needs no Cargo target symlink"; fi

echo "== converted root modules cannot regain handwritten compiler inputs =="
cp "$SHIM_GIT_TRACKED" "$TMP/git-tracked.saved"
printf 'export const staleHosts = true;\n' > "$REPO/web/hosts.js"
printf 'web/hosts.js\n' >> "$SHIM_GIT_TRACKED"
if (cd "$REPO" && EMBER_PAGES_ARCHIVE="$TMP/tracked-hosts.tar.gz" bash deploy/deploy-pages.sh) > "$TMP/tracked-hosts.log" 2>&1; then
    bad "a tracked web/hosts.js was accepted"
else
    ok "a tracked web/hosts.js is refused before generation"
fi
contains "$(cat "$TMP/tracked-hosts.log")" "converted browser modules must come from TypeScript emission" "the tracked-JavaScript refusal explains the generated source"
mv "$TMP/git-tracked.saved" "$SHIM_GIT_TRACKED"
rm "$REPO/web/hosts.js"
printf 'export const bareLoader = true;\n' > "$REPO/web/loader.ts"
if (cd "$REPO" && EMBER_PAGES_ARCHIVE="$TMP/bare-loader.tar.gz" bash deploy/deploy-pages.sh) > "$TMP/bare-loader.log" 2>&1; then
    bad "a bare web/loader.ts was accepted"
else
    ok "a bare web/loader.ts is refused before generation"
fi
contains "$(cat "$TMP/bare-loader.log")" "use web/loader.ts.j2" "the bare-TypeScript refusal names the template form"
rm "$REPO/web/loader.ts"

echo "== a TypeScript failure refuses assembly before the Pages seed =="
: > "$SHIM_LOG"
TSC_FAIL_ARCHIVE="$TMP/tsc-failure.tar.gz"
if (cd "$REPO" && SHIM_NPX_FAIL=1 EMBER_PAGES_ARCHIVE="$TSC_FAIL_ARCHIVE" bash deploy/deploy-pages.sh) > "$TMP/tsc-failure.log" 2>&1; then
    bad "a failed TypeScript compile was accepted"
else
    ok "a failed TypeScript compile was refused"
fi
contains "$(cat "$TMP/tsc-failure.log")" "TIMING typescript wall=" "the failed TypeScript wall time is recorded"
if [ -f "$TSC_FAIL_ARCHIVE" ]; then bad "a failed TypeScript compile produced an archive"; else ok "a failed TypeScript compile produced no archive"; fi
if grep -q '^git \[fetch\]' "$SHIM_LOG"; then bad "a failed TypeScript compile reached Pages assembly"; else ok "a failed TypeScript compile stopped before Pages assembly"; fi

echo "== a configured Cargo target directory supplies wasm-bindgen inputs =="
: > "$SHIM_LOG"
CUSTOM_TARGET="$TMP/shared-cargo-target"
CUSTOM_ARCHIVE="$TMP/custom-target-pages.tar.gz"
if (cd "$REPO" && SOURCE_DATE_EPOCH=1700000000 CARGO_TARGET_DIR="$CUSTOM_TARGET" EMBER_PAGES_ARCHIVE="$CUSTOM_ARCHIVE" bash deploy/deploy-pages.sh) > "$TMP/custom-target.log" 2>&1; then
    ok "the configured-target build-and-archive run succeeded"
else
    bad "the configured-target build-and-archive run failed"
    tail -40 "$TMP/custom-target.log" >&2
fi
CUSTOM_ARGV="$(cat "$SHIM_LOG")"
contains "$CUSTOM_ARGV" "[$CUSTOM_TARGET/wasm32-unknown-unknown/release/fire.wasm]" "wasm-bindgen reads Fire from the configured Cargo target directory"
contains "$CUSTOM_ARGV" "[$CUSTOM_TARGET/wasm32-unknown-unknown/release/ember_lab_julibrot.wasm]" "wasm-bindgen reads Julibrot from the configured Cargo target directory"
if [ -e "$REPO/target/wasm32-unknown-unknown" ]; then bad "the configured-target build used a default Cargo target entry"; else ok "the configured-target build needs no Cargo target symlink"; fi
for f in index.html main.js quality.js dialogue.js castle-audio.js castle-ui.js voice-lines.js style.css cover.png prologue.mp4 ambience.wav boss-defeat.wav boss-intro.wav boss-phase2.wav castle-ambience.wav escape-clue.wav escape-ending.wav warden-death.wav warden-movement.wav warden-sword.wav warden-unlocking.wav pkg/end_game.js pkg/end_game_bg.wasm; do
    if [ -f "$SHIM_PUBLISHED/$END_GAME_LIVE/$f" ]; then ok "assembled End Game $f"; else bad "missing End Game $f"; fi
done
for module in main dialogue castle-audio castle-ui quality voice-lines; do
    contains "$(cat "$SHIM_PUBLISHED/$END_GAME_LIVE/$module.js")" \
        "/* emitted $module */" \
        "assembled End Game $module.js comes from TypeScript emission"
done
contains "$ARGV" "[-p] [end-game] [--lib]" "End Game is built as an Ember wasm library"
if [ "$END_GAME_LIVE" != games/end-game/v1 ]; then
    if diff -r "$SEED/games/end-game/v1" "$SHIM_PUBLISHED/games/end-game/v1" > "$TMP/end-game-v1.diff"; then ok "frozen End Game v1 remains byte-identical"; else bad "frozen End Game v1 changed"; fi
fi
for f in index.html pkg/what_is_this.js pkg/what_is_this_bg.wasm; do
    if [ -f "$SHIM_PUBLISHED/games/what-is-this/v1/$f" ]; then
        ok "assembled what-is-this $f"
    else
        bad "assembled what-is-this is missing $f"
    fi
done
for f in index.html ui.js ui.css art/swarm.webp art/nested/manifest.json version.json pkg/league.js pkg/league_bg.wasm; do
    if [ -f "$SHIM_PUBLISHED/$LEAGUE_LIVE/$f" ]; then
        ok "assembled League $f"
    else
        bad "assembled League is missing $f"
    fi
done
for f in arena.webp swarm.webp emberknight.webp hallow.webp bogmaw.webp tessera.webp icons.svg; do
    if cmp -s "$REPO/web/games/league/v2/art/$f" "$SHIM_PUBLISHED/games/league/v2/art/$f"; then
        ok "League v4 shared art $f is derived from tracked source"
    else
        bad "League v4 shared art $f is missing or came from the seed"
    fi
done
is "$(find "$SHIM_PUBLISHED/$LEAGUE_LIVE/pkg" -type f -printf '%f\n' | sort | tr '\n' ' ')" "league.js league_bg.wasm " "League gets only its own generated bundle and replaces stale files"
if cmp -s "$REPO/web/version.json" "$SHIM_PUBLISHED/$LEAGUE_LIVE/version.json"; then
    ok "League carries the source build stamp beside its page"
else
    bad "League is missing the source build stamp"
fi
if [ "$LEAGUE_LIVE" != games/league/v1 ]; then
    if diff -r "$SEED/games/league/v1" "$SHIM_PUBLISHED/games/league/v1" > "$TMP/league-v1.diff"; then
        ok "frozen League v1 is preserved byte-for-byte"
    else
        bad "frozen League v1 changed"
        cat "$TMP/league-v1.diff" >&2
    fi
fi
for f in index.html main.js lab.js drive.html worker.js style.css whole-grid-oracle.html whole-grid-v1.json whole-grid-v1.rgba.deflate.bin pkg/ember_lab_julibrot.js pkg/ember_lab_julibrot_bg.wasm; do
    if [ -f "$SHIM_PUBLISHED/labs/julibrot/$f" ]; then
        ok "assembled Julibrot $f"
    else
        bad "assembled Julibrot is missing $f"
    fi
done
if [ -e "$SHIM_PUBLISHED/$END_GAME_LIVE/untracked.tmp" ]; then
    bad "an untracked file under a live tree was published"
else
    ok "untracked files under live trees are not published"
fi
if [ -e "$SHIM_PUBLISHED/mirrors.json" ] || [ -e "$SHIM_PUBLISHED/hosts.test.mjs" ] \
    || [ -e "$SHIM_PUBLISHED/hosts.test.mts.j2" ]; then
    bad "root-only deployment metadata or test templates were published"
else
    ok "web/mirrors.json and web test templates stay out of the root publication"
fi
if [ -e "$SHIM_PUBLISHED/hosts.ts.j2" ] || [ -e "$SHIM_PUBLISHED/loader.ts.j2" ] \
    || [ -e "$SHIM_PUBLISHED/exhaustive-consumer.js" ]; then
    bad "templates or an unrelated compiler output entered the root publication"
else
    ok "only the named emitted browser modules enter the root publication"
fi
STAMP="$(jget "$SHIM_PUBLISHED/server.json" 'd["v"]')"
END_GAME_EXPECTED="$TMP/end-game-emitted-expected"
module_args=()
for module in main dialogue castle-audio castle-ui quality voice-lines; do
    module_args+=(--module "$END_GAME_LIVE/$module.js")
done
"$PY" "$REPO/deploy/verify-emitted-modules.py" --write \
    --emitted-root "$REPO/target/web-generated/js" \
    --assembled-root "$END_GAME_EXPECTED" \
    --stamp "$STAMP" \
    "${module_args[@]}" \
    --stamped "$END_GAME_LIVE/main.js" >/dev/null
for module in main dialogue castle-audio castle-ui quality voice-lines; do
    if cmp -s "$END_GAME_EXPECTED/$END_GAME_LIVE/$module.js" \
            "$SHIM_PUBLISHED/$END_GAME_LIVE/$module.js"; then
        ok "assembled End Game $module.js matches its emitted expectation"
    else
        bad "assembled End Game $module.js differs from its emitted expectation"
    fi
done
is "$STAMP" "recomputed-stamp" "address recompute changed the deploy stamp"
is "$(jget "$SHIM_PUBLISHED/server.json" 'd["ws"]')" "wss://new.example" "address recompute changed the legacy address"
is "$(jget "$SHIM_PUBLISHED/server.json" 'd["league_proto"]')" "$LEAGUE_PROTO" "League ships its independent source protocol"
is "$(jget "$SHIM_PUBLISHED/server.json" '[m["name"] for m in d["mirrors"]]')" "['quiet-egret', 'lundi']" "the seed's bindings keep their order and the source binding merges by name"
is "$(jget "$SHIM_PUBLISHED/server.json" '[m["url"] for m in d["mirrors"] if m["name"] == "lundi"][0]')" "https://source.example/lundi.json" "source wins for the name it declares, so a frozen URL can be corrected"
is "$(jget "$SHIM_PUBLISHED/server.json" '[m["url"] for m in d["mirrors"] if m["name"] == "quiet-egret"][0]')" "https://seed.example/quiet-egret.json" "a binding only the seed carries is not dropped"
# The seed's own binding is the previous release's declaration, made through
# this same path: a host it binds is served from its mirror too, so its stale
# entry goes with the rest. Taking only this release's bindings would leave
# every previously bound host shadowed by its own entry.
is "$(jget "$SHIM_PUBLISHED/server.json" '[h["name"] for h in d["hosts"] if h["name"] == "quiet-egret"]')" "[]" "a host bound by the seed's own mirrors list is dropped too"
# Without this the binding is a no-op: mergeBook gives the book's own hosts[]
# precedence over any mirror of the same name, so the seed's stale entry would
# win on every page and the mirror would never be read at all.
is "$(jget "$SHIM_PUBLISHED/server.json" '[h["name"] for h in d["hosts"]]')" "['new-host']" "every bound name's stale entry is removed, so the mirror is what pages read"
contains "$(cat "$TMP/build.log")" "dropped 2 seed host entry" "both the source-bound and the seed-bound host were superseded"
is "$(jget "$SHIM_PUBLISHED/server.json" 'd["league_ws"]')" "wss://new-league.example" "League address recompute follows its shipped protocol"
contains "$(cat "$TMP/build.log")" "LEAGUE PROTOCOL BUMP: v$LEAGUE_WAS -> v$LEAGUE_PROTO" "a League protocol change reports its required server restart"
if cmp -s "$REPO/web/$ARENA_LIVE/settings.js" "$SHIM_PUBLISHED/$ARENA_LIVE/settings.js"; then
    ok "Arena settings.js is copied byte-for-byte beside its live page"
else
    bad "Arena settings.js was omitted or changed"
fi
contains "$(cat "$SHIM_PUBLISHED/$ARENA_LIVE/index.html")" "src=\"./settings.js?v=$STAMP\"" "Arena settings loader uses the final recomputed deploy stamp"
# The shared loader: one copy at the root, one bundle under the root pkg, and
# the page's single cache token rewritten to the same stamp as everything else.
if cmp -s "$REPO/target/web-generated/js/loader.js" "$SHIM_PUBLISHED/loader.js" \
    && cmp -s "$REPO/target/web-generated/js/hosts.js" "$SHIM_PUBLISHED/hosts.js"; then
    ok "the emitted host picker and shared loader ship at the pages root"
else
    bad "the root host picker or loader did not come from compiler output"
fi
FIRE_EXPECTED="$TMP/fire-emitted-expected"
verify_fire_modules() {
    "$PY" "$REPO/deploy/verify-emitted-modules.py" "$@" \
        --stamp "$STAMP" \
        --module games/fire/v2/race.js \
        --module games/fire/v2/garage.js \
        --stamped games/fire/v2/race.js
}
verify_fire_modules --write \
    --emitted-root "$REPO/target/web-generated/js" \
    --assembled-root "$FIRE_EXPECTED" >/dev/null
if cmp -s "$FIRE_EXPECTED/games/fire/v2/race.js" \
        "$SHIM_PUBLISHED/games/fire/v2/race.js" \
    && cmp -s "$FIRE_EXPECTED/games/fire/v2/garage.js" \
        "$SHIM_PUBLISHED/games/fire/v2/garage.js"; then
    ok "stamped and unstamped Fire modules match their emitted expectations"
else
    bad "a Fire module differs from its emitted expectation"
fi
contains "$(cat "$REPO/target/web-generated/js/games/fire/v2/race.js")" \
    'loader.js?v=1' "the emitted Fire module remains stamp-independent"
if grep -qF "$TRACKED_RACE_DECOY_MARK" "$SHIM_PUBLISHED/games/fire/v2/race.js"; then
    bad "a same-named tracked page source replaced the emitted module"
else
    ok "the emitted module survives a same-named tracked page source"
fi
DUPLICATE_EMITTED="$TMP/fire-duplicate-emitted"
mkdir -p "$DUPLICATE_EMITTED/games/fire/v2"
cp "$REPO/target/web-generated/js/games/fire/v2/race.js" \
    "$REPO/target/web-generated/js/games/fire/v2/garage.js" \
    "$DUPLICATE_EMITTED/games/fire/v2/"
printf '\nimport "../../../loader.js?v=1";\n' >> "$DUPLICATE_EMITTED/games/fire/v2/race.js"
if verify_fire_modules \
    --emitted-root "$DUPLICATE_EMITTED" \
    --assembled-root "$FIRE_EXPECTED" > "$TMP/fire-duplicate.log" 2>&1; then
    bad "a stamped module with duplicate loader tokens passed emitted verification"
else
    ok "a stamped module with duplicate loader tokens is rejected by emitted verification"
fi
contains "$(cat "$TMP/fire-duplicate.log")" \
    "stamped emitted module must contain exactly one loader cache token" \
    "the duplicate-token rejection comes from the emitted verifier"
CORRUPT_STAMPED="$TMP/fire-corrupt-stamped"
cp -R "$FIRE_EXPECTED" "$CORRUPT_STAMPED"
printf '\n// corrupt stamped module\n' >> "$CORRUPT_STAMPED/games/fire/v2/race.js"
if verify_fire_modules \
    --emitted-root "$REPO/target/web-generated/js" \
    --assembled-root "$CORRUPT_STAMPED" > "$TMP/fire-corrupt-stamped.log" 2>&1; then
    bad "a corrupted stamped module passed emitted verification"
else
    ok "a corrupted stamped module is rejected by emitted verification"
fi
contains "$(cat "$TMP/fire-corrupt-stamped.log")" \
    "assembled module does not match its emitted expectation: games/fire/v2/race.js" \
    "the corrupted stamped-module rejection comes from the emitted verifier"
CORRUPT_UNSTAMPED="$TMP/fire-corrupt-unstamped"
cp -R "$FIRE_EXPECTED" "$CORRUPT_UNSTAMPED"
printf '\n// corrupt unstamped module\n' >> "$CORRUPT_UNSTAMPED/games/fire/v2/garage.js"
if verify_fire_modules \
    --emitted-root "$REPO/target/web-generated/js" \
    --assembled-root "$CORRUPT_UNSTAMPED" > "$TMP/fire-corrupt-unstamped.log" 2>&1; then
    bad "a corrupted unstamped module passed emitted verification"
else
    ok "a corrupted unstamped module is rejected by emitted verification"
fi
contains "$(cat "$TMP/fire-corrupt-unstamped.log")" \
    "assembled module does not match its emitted expectation: games/fire/v2/garage.js" \
    "the corrupted unstamped-module rejection comes from the emitted verifier"
for f in pkg/ember_loader.js pkg/ember_loader_bg.wasm; do
    if [ -f "$SHIM_PUBLISHED/$f" ]; then ok "assembled the shared loader $f"; else bad "missing the shared loader $f"; fi
done
if [ -f "$SHIM_PUBLISHED/$ARENA_LIVE/pkg/ember_loader.js" ]; then
    bad "the shared loader was copied into a game directory as well as the root"
else
    ok "the shared loader is shipped once, not once per game"
fi
contains "$ARGV" "[-p] [ember-loader] [--lib]" "the shared loader is built as a wasm library"
contains "$ARGV" "release/ember_loader.wasm" "the shared loader is passed to wasm-bindgen"
contains "$(cat "$SHIM_PUBLISHED/$ARENA_LIVE/index.html")" "../../../loader.js?v=$STAMP" "the page's loader import carries the deploy stamp"
contains "$(cat "$TMP/build.log")" "stamped the shared loader into 6 live game page" "the assembly reports one stamp per live game page"
contains "$(cat "$TMP/build.log")" "resolved " "the assembly reports its live-page reference walk"
if grep -qE 'loader\.js\?v=1([^0-9]|$)' "$SHIM_PUBLISHED/$ARENA_LIVE/index.html"; then
    bad "the assembled page kept a stale loader cache key"
else
    ok "the assembled page has no stale loader cache key"
fi
contains "$(cat "$REPO/web/$ARENA_LIVE/index.html")" 'loader.js?v=1' "the Arena source loader import remains pinned at v=1"
cp "$REPO/web/games/fire/v2/race.ts.j2" "$TMP/fire-race.saved"
sed -i '/loader\.js?v=1/d' "$REPO/web/games/fire/v2/race.ts.j2"
if (cd "$REPO" && SOURCE_DATE_EPOCH=1700000000 EMBER_PAGES_ARCHIVE="$TMP/loader-missing.tar.gz" bash deploy/deploy-pages.sh) > "$TMP/loader-missing.log" 2>&1; then
    bad "a live game page with no loader import was accepted"
else
    ok "a live game page with no loader import was refused"
fi
contains "$(cat "$TMP/loader-missing.log")" "missing: games/fire/v2" "the loader stamp mismatch names the missing live page"
mv "$TMP/fire-race.saved" "$REPO/web/games/fire/v2/race.ts.j2"
if grep -q '"bytes"' "$REPO/web/games.json"; then
    bad "the tracked catalog carries a generated bundle size"
else
    ok "the tracked catalog leaves generated bundle sizes to the deploy"
fi
for spec in "$ARENA_LIVE arena arena" "$END_GAME_LIVE end-game end_game" "$LEAGUE_LIVE league league" "games/fire/v2 fire fire" "games/kings/v1 kings kings" "games/what-is-this/v1 what-is-this what_is_this"; do
    # shellcheck disable=SC2086
    set -- $spec
    live_path="$1"
    game_id="$2"
    bundle="$3"
    expected_bytes="$(wc -c < "$SHIM_PUBLISHED/$live_path/pkg/${bundle}_bg.wasm" | tr -d ' ')"
    served_bytes="$(jget "$SHIM_PUBLISHED/games.json" '[v["bytes"] for g in d["games"] if g["id"] == "'"$game_id"'" for v in g["versions"] if v.get("live") is True][0]')"
    is "$served_bytes" "$expected_bytes" "$game_id live catalog bytes match its decoded wasm file"
done
if grep -qE '\./settings\.js\?v=1([^0-9]|$)' "$SHIM_PUBLISHED/$ARENA_LIVE/index.html"; then
    bad "assembled Arena settings loader retained its stale cache key"
else
    ok "assembled Arena settings loader has no stale cache key"
fi
contains "$(cat "$REPO/web/$ARENA_LIVE/index.html")" 'src="./settings.js?v=1"' "Arena source loader remains pinned at v=1"
for f in index.html settings.js; do
    if cmp -s "$SEED/games/arena/v17/$f" "$SHIM_PUBLISHED/games/arena/v17/$f"; then
        ok "archived Arena $f is untouched"
    else
        bad "archived Arena $f was rewritten"
    fi
done
if diff -r "$SEED/games/arena/v0" "$SHIM_PUBLISHED/games/arena/v0" > "$TMP/arena-v0.diff"; then
    ok "frozen Arena v0 remains byte-identical to the seed"
else
    bad "frozen Arena v0 changed"
    cat "$TMP/arena-v0.diff" >&2
fi
while IFS= read -r seed_file; do
    relative="${seed_file#"$SEED/games/arena/v0/"}"
    seed_sha="$(sha256sum "$seed_file" | cut -d ' ' -f 1)"
    assembled_sha="$(sha256sum "$SHIM_PUBLISHED/games/arena/v0/$relative" | cut -d ' ' -f 1)"
    is "$assembled_sha" "$seed_sha" "frozen Arena v0 $relative keeps its seed SHA-256"
done < <(find "$SEED/games/arena/v0" -type f | sort)
if grep -Fq "git [ls-files] [-z] [--] [web/games/arena/v0]" "$SHIM_LOG"; then
    bad "the assembler read Arena v0 source files"
else
    ok "the assembler leaves Arena v0 entirely to the seed"
fi
while IFS= read -r f; do
    name="${f#"$SHIM_PUBLISHED/labs/julibrot/"}"
    contains "$(cat "$f")" "?v=$STAMP" "Julibrot $name uses the deploy stamp"
    if grep -qE '\?v=1([^0-9]|$)' "$f"; then
        bad "assembled Julibrot $name retained ?v=1"
    else
        ok "assembled Julibrot $name has no stale ?v=1 cache key"
    fi
    if grep -qE '\?v=1([^0-9]|$)' "$REPO/web/labs/julibrot/$name"; then
        ok "Julibrot source $name remains pinned at ?v=1"
    else
        bad "Julibrot source $name was rewritten"
    fi
done < <(find "$SHIM_PUBLISHED/labs/julibrot" -maxdepth 1 -type f \( -name '*.html' -o -name '*.js' \) | LC_ALL=C sort)
contains "$(cat "$SHIM_PUBLISHED/labs/julibrot/lab.js")" "JULIBROT_WORKER_URL = \"./worker.js?v=$STAMP\"" "the worker bootstrap URL uses the deploy stamp"
contains "$(cat "$SHIM_PUBLISHED/labs/julibrot/main.js")" "./future.js?v=10" "a future two-digit cache key is not partly rewritten"
contains "$(cat "$SHIM_PUBLISHED/labs/julibrot/main.js")" "from \"./lab.js?v=$STAMP\"" "the page's static import of the lab module is stamped"
is "$(jget "$SHIM_PUBLISHED/games.json" '[v["path"] for g in d["games"] if g.get("kind") == "lab" for v in g["versions"] if v.get("live")][0]')" "labs/julibrot/" "the live Julibrot catalog path was assembled"

echo "== comparison mode proves byte identity without pushing =="
cp "$REPO/web/version.json" "$TMP/version.before-archive.json"
: > "$SHIM_LOG"
if (cd "$REPO" && SOURCE_DATE_EPOCH=1700000000 EMBER_PAGES_PREBUILT=1 EMBER_PAGES_COMPARE="$ARCHIVE" bash deploy/deploy-pages.sh) > "$TMP/archive.log" 2>&1; then
    ok "the prebuilt comparison run succeeded"
else
    bad "the prebuilt comparison run failed"
    tail -40 "$TMP/archive.log" >&2
fi
if grep -q '^git \[push\]' "$SHIM_LOG"; then
    bad "comparison mode pushed a branch"
else
    ok "comparison mode made no branch push"
fi
tar -tzf "$ARCHIVE" > "$TMP/archive.list"
if grep -Fqx './index.html' "$TMP/archive.list" && grep -Fqx "./$ARENA_LIVE/index.html" "$TMP/archive.list"; then
    ok "the archive contains the hub and live game tree"
else
    bad "the archive is missing the hub or live game tree"
fi
if grep -Eq '^\./\.git(/|$)' "$TMP/archive.list"; then
    bad "the release archive contains worktree metadata"
else
    ok "the release archive excludes worktree metadata"
fi
if grep -Eq '\.d\.ts$' "$TMP/archive.list"; then bad "the release archive contains declarations"; else ok "the release archive excludes declarations"; fi
contains "$(cat "$TMP/archive.log")" "byte-identical to" "comparison mode reports byte identity"
cp "$TMP/version.before-archive.json" "$REPO/web/version.json"

mkdir -p "$EXPECTED"
cp -R "$SEED/games" "$EXPECTED/"
for spec in "${ARENA_LIVE#games/} arena" "fire/v2 fire" "kings/v1 kings" "${LEAGUE_LIVE#games/} league"; do
    # shellcheck disable=SC2086
    set -- $spec
    live="$1"
    bundle="$2"
    rm -rf "$EXPECTED/games/$live"
    mkdir -p "$EXPECTED/games/$live/pkg"
    cp "$REPO/web/games/$live/index.html" "$EXPECTED/games/$live/"
    if [ "$bundle" = fire ]; then
        cp "$REPO/web/games/$live/style.css" "$EXPECTED/games/$live/"
        # The faces and their licence ship; README.md does not.
        mkdir -p "$EXPECTED/games/$live/fonts"
        cp "$REPO/web/games/$live"/fonts/*.woff2 "$REPO/web/games/$live/fonts/OFL.txt" "$EXPECTED/games/$live/fonts/"
    fi
    if [ "$bundle" = league ]; then
        cp "$REPO/web/version.json" "$EXPECTED/games/$live/"
        cp "$REPO/web/games/$live/ui.js" "$REPO/web/games/$live/ui.css" "$EXPECTED/games/$live/"
        cp -R "$REPO/web/games/$live/art" "$EXPECTED/games/$live/"
    fi
    if [ "games/$live" = "$ARENA_LIVE" ]; then
        # Expected output is authored independently of the publisher's rewrite.
        printf 'arena live<script src="./settings.js?v=%s"></script><script type="module">import { emberLoad } from "../../../loader.js?v=%s";</script>\n' "$STAMP" "$STAMP" > "$EXPECTED/games/$live/index.html"
        cp "$REPO/web/games/$live/settings.js" "$EXPECTED/games/$live/"
    fi
    printf 'shim js for %s\n' "$bundle" > "$EXPECTED/games/$live/pkg/$bundle.js"
    printf 'shim wasm for %s\n' "$bundle" > "$EXPECTED/games/$live/pkg/${bundle}_bg.wasm"
done
"$PY" "$REPO/deploy/verify-emitted-modules.py" --write \
    --emitted-root "$REPO/target/web-generated/js" \
    --assembled-root "$EXPECTED" \
    --stamp "$STAMP" \
    --module games/fire/v2/race.js \
    --module games/fire/v2/garage.js \
    --stamped games/fire/v2/race.js
mkdir -p "$EXPECTED/games/league/v2"
cp -R "$REPO/web/games/league/v2/art" "$EXPECTED/games/league/v2/"
for page in games/kings/v1/index.html "$LEAGUE_LIVE/ui.js"; do
    sed -i "s/loader\\.js?v=1/loader.js?v=$STAMP/" "$EXPECTED/$page"
done
for game in arena fire kings league; do
    if diff -r "$EXPECTED/games/$game" "$SHIM_PUBLISHED/games/$game" > "$TMP/$game.diff"; then
        ok "$game Pages tree is unchanged"
    else
        bad "$game Pages tree changed"
        cat "$TMP/$game.diff" >&2
    fi
done

echo "== prebuilt mode fails closed on missing game and Julibrot artifacts =="
rm "$REPO/web/pkg/what_is_this_bg.wasm" "$REPO/web/pkg/league.js" "$REPO/web/pkg/league_bg.wasm" "$REPO/web/pkg/ember_loader.js" "$REPO/web/labs/julibrot/pkg/ember_lab_julibrot.js" "$REPO/web/labs/julibrot/pkg/ember_lab_julibrot_bg.wasm"
: > "$SHIM_LOG"
if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh) > "$TMP/missing.log" 2>&1; then
    bad "prebuilt mode accepted a missing artifact"
else
    ok "prebuilt mode refused a missing artifact"
fi
contains "$(cat "$TMP/missing.log")" "web/labs/julibrot/pkg/ember_lab_julibrot.js" "the failure lists the missing Julibrot JavaScript"
contains "$(cat "$TMP/missing.log")" "web/labs/julibrot/pkg/ember_lab_julibrot_bg.wasm" "the failure lists the missing Julibrot wasm"
contains "$(cat "$TMP/missing.log")" "web/pkg/what_is_this_bg.wasm" "the failure lists the missing game wasm"
contains "$(cat "$TMP/missing.log")" "web/pkg/league.js" "the failure lists the missing League JavaScript"
contains "$(cat "$TMP/missing.log")" "web/pkg/league_bg.wasm" "the failure lists the missing League wasm"
contains "$(cat "$TMP/missing.log")" "web/pkg/ember_loader.js" "the failure lists the missing shared loader"
if grep -q '^cargo' "$SHIM_LOG"; then bad "the refused prebuilt run invoked cargo"; else ok "the refused prebuilt run invoked no cargo"; fi
if grep -q '^git \[fetch\]' "$SHIM_LOG"; then bad "missing prebuilt artifacts reached Pages assembly"; else ok "missing prebuilt artifacts stopped before Pages assembly"; fi

echo "== complete prebuilt mode reuses wasm and refreshes declarations =="
printf 'shim wasm for what_is_this\n' > "$REPO/web/pkg/what_is_this_bg.wasm"
printf 'shim js for league\n' > "$REPO/web/pkg/league.js"
printf 'shim wasm for league\n' > "$REPO/web/pkg/league_bg.wasm"
printf 'shim js for ember_loader\n' > "$REPO/web/pkg/ember_loader.js"
printf 'shim js for ember_lab_julibrot\n' > "$REPO/web/labs/julibrot/pkg/ember_lab_julibrot.js"
printf 'shim wasm for ember_lab_julibrot\n' > "$REPO/web/labs/julibrot/pkg/ember_lab_julibrot_bg.wasm"
rm -rf "$REPO/web/$END_GAME_LIVE/pkg"
if [ -f "$REPO/web/pkg/end_game.d.ts" ] && [ -f "$REPO/web/pkg/end_game_bg.wasm.d.ts" ] && [ ! -e "$REPO/web/$END_GAME_LIVE/pkg" ]; then
    ok "complete prebuilt mode needs End Game declarations only at the documented root package path"
else
    bad "the End Game prebuilt fixture did not isolate the documented root package path"
fi
: > "$SHIM_LOG"
rm -f "$SHIM_GIT_INDEX"
if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh) > "$TMP/prebuilt.log" 2>&1; then
    ok "complete prebuilt mode assembled the tree"
else
    bad "complete prebuilt mode failed"
    tail -40 "$TMP/prebuilt.log" >&2
fi
if grep -q '^cargo \[build\]' "$SHIM_LOG" || grep -q '^wasm-bindgen' "$SHIM_LOG"; then bad "complete prebuilt mode rebuilt wasm"; else ok "complete prebuilt mode reused every wasm artifact"; fi
contains "$(cat "$SHIM_LOG")" "cargo [run] [--locked] [-p] [ember-webgen]" "complete prebuilt mode still refreshes Rust-owned declarations"
contains "$(cat "$SHIM_LOG")" "npx [--no-install] [tsc]" "complete prebuilt mode still compiles declarations"

echo "== unsafe or ambiguous League catalog destinations are refused =="
cp "$REPO/web/games.json" "$TMP/catalog.saved"
for fixture in traversal other-game duplicate; do
    "$PY" - "$TMP/catalog.saved" "$REPO/web/games.json" "$fixture" <<'PY'
import json, sys
catalog = json.load(open(sys.argv[1], encoding="utf-8"))
league = next(g for g in catalog["games"] if g["id"] == "league")
live = next(v for v in league["versions"] if v.get("live") is True)
if sys.argv[3] == "duplicate":
    league["versions"].append(dict(live))
else:
    live["path"] = {"traversal": "games/league/v2/../../fire/v2/", "other-game": "games/fire/v2/"}[sys.argv[3]]
with open(sys.argv[2], "w", encoding="utf-8") as fh:
    json.dump(catalog, fh)
PY
    : > "$SHIM_LOG"
    if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh) > "$TMP/league-$fixture.log" 2>&1; then
        bad "the $fixture League destination was accepted"
    else
        ok "the $fixture League destination was refused"
    fi
    if [ "$fixture" = duplicate ]; then
        contains "$(cat "$TMP/league-$fixture.log")" "exactly one live release" "the $fixture League failure identifies the catalog contract"
    else
        contains "$(cat "$TMP/league-$fixture.log")" "exactly one safe live version path" "the $fixture League failure identifies the catalog contract"
    fi
    if grep -q '^git \[fetch\]' "$SHIM_LOG"; then bad "$fixture League destination reached Pages assembly"; else ok "$fixture League destination stopped before Pages assembly"; fi
done
cp "$TMP/catalog.saved" "$REPO/web/games.json"

echo "== catalog release versions are three-grade and match live packages =="
for fixture in missing malformed mismatch; do
    "$PY" - "$TMP/catalog.saved" "$REPO/web/games.json" "$fixture" <<'PY'
import json, sys
catalog = json.load(open(sys.argv[1], encoding="utf-8"))
arena = next(game for game in catalog["games"] if game["id"] == "arena")
live = next(release for release in arena["versions"] if release.get("live") is True)
if sys.argv[3] == "missing":
    del arena["versions"][-1]["version"]
elif sys.argv[3] == "malformed":
    arena["versions"][-1]["version"] = "v1"
else:
    # Derived, never frozen, for the reason the protocol fixtures above are:
    # a hardcoded "wrong" version becomes the RIGHT one the day the series
    # reaches it, and the suite then proves the opposite of what it says.
    major, minor, patch = live["version"].split(".")
    live["version"] = "%d.%s.%s" % (int(major) + 1, minor, patch)
with open(sys.argv[2], "w", encoding="utf-8") as fh:
    json.dump(catalog, fh)
PY
    : > "$SHIM_LOG"
    if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh) > "$TMP/version-$fixture.log" 2>&1; then
        bad "the $fixture catalog release version was accepted"
    else
        ok "the $fixture catalog release version was refused"
    fi
    contains "$(cat "$TMP/version-$fixture.log")" "version" "the $fixture release-version failure identifies the contract"
    if grep -q '^git \[fetch\]' "$SHIM_LOG"; then bad "$fixture release version reached Pages assembly"; else ok "$fixture release version stopped before Pages assembly"; fi
done
cp "$TMP/catalog.saved" "$REPO/web/games.json"

echo "== the catalog protocol must equal the crate constant, both directions =="
# This check is what lets the deploy-time host gate believe games.json at all,
# so the agreeing case has to keep passing or the gate is bought at the price
# of never being able to ship.
: > "$SHIM_LOG"
if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh) > "$TMP/proto-agree.log" 2>&1; then
    ok "a catalog whose live protocol equals the crate constant is accepted"
else
    bad "an agreeing catalog protocol was refused"
    tail -20 "$TMP/proto-agree.log" >&2
fi
for fixture in stale-catalog absent-catalog; do
    "$PY" - "$TMP/catalog.saved" "$REPO/web/games.json" "$fixture" <<'PY'
import json, sys
catalog = json.load(open(sys.argv[1], encoding="utf-8"))
fire = next(game for game in catalog["games"] if game["id"] == "fire")
live = next(release for release in fire["versions"] if release.get("live") is True)
if sys.argv[3] == "stale-catalog":
    # The exact 2026-09 shape: the crate moved and the catalog did not.
    live["proto"] = live["proto"] + 1
else:
    del live["proto"]
with open(sys.argv[2], "w", encoding="utf-8") as fh:
    json.dump(catalog, fh)
PY
    : > "$SHIM_LOG"
    if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh) > "$TMP/proto-$fixture.log" 2>&1; then
        bad "the $fixture catalog protocol was accepted"
    else
        ok "the $fixture catalog protocol was refused"
    fi
    contains "$(cat "$TMP/proto-$fixture.log")" "fire live catalog proto is" "the $fixture refusal names the game and both numbers"
    contains "$(cat "$TMP/proto-$fixture.log")" "crates/fire-core/src/proto.rs declares $FIRE_PROTO" "the $fixture refusal names the crate constant it must equal"
    if grep -q '^git \[fetch\]' "$SHIM_LOG"; then bad "$fixture protocol drift reached Pages assembly"; else ok "$fixture protocol drift stopped before Pages assembly"; fi
done
cp "$TMP/catalog.saved" "$REPO/web/games.json"

# The other direction, and the one a hardcoded list of game ids would have
# passed silently: a live protocol number nothing in the tree can confirm. That
# is how a fifth server game ends up gated by nothing at all.
for fixture in uncheckable lab-proto; do
    "$PY" - "$TMP/catalog.saved" "$REPO/web/games.json" "$fixture" <<'PY'
import json, sys
catalog = json.load(open(sys.argv[1], encoding="utf-8"))
if sys.argv[3] == "uncheckable":
    game = next(g for g in catalog["games"] if g["id"] == "what-is-this")
    next(r for r in game["versions"] if r.get("live") is True)["proto"] = 3
else:
    # A lab has no protocol at all, and a number here is inert in both readers.
    game = next(g for g in catalog["games"] if g.get("kind") == "lab")
    next(r for r in game["versions"] if r.get("live") is True)["proto"] = 3
with open(sys.argv[2], "w", encoding="utf-8") as fh:
    json.dump(catalog, fh)
PY
    : > "$SHIM_LOG"
    if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 EMBER_PAGES_ARCHIVE="$TMP/proto-$fixture.tar.gz" bash deploy/deploy-pages.sh) > "$TMP/proto-$fixture.log" 2>&1; then
        bad "the $fixture catalog protocol was accepted"
    else
        ok "the $fixture catalog protocol was refused"
    fi
    if [ -f "$TMP/proto-$fixture.tar.gz" ]; then bad "$fixture still produced an archive"; else ok "$fixture produced no archive"; fi
    if grep -q '^git \[fetch\]' "$SHIM_LOG"; then bad "$fixture reached Pages assembly"; else ok "$fixture stopped before Pages assembly"; fi
done
contains "$(cat "$TMP/proto-uncheckable.log")" "what-is-this live catalog declares proto 3" "the uncheckable refusal names the game and the number"
contains "$(cat "$TMP/proto-uncheckable.log")" "no crates/what-is-this-core/src/proto.rs to check it against" "and the crate path it would have needed"
contains "$(cat "$TMP/proto-lab-proto.log")" "julibrot is a lab and must declare no proto" "the lab refusal names the lab, whose proto both readers ignore"
cp "$TMP/catalog.saved" "$REPO/web/games.json"

echo "== a protocol crate with no constant is named, not a traceback =="
cp "$REPO/crates/kings-core/src/proto.rs" "$TMP/kings-proto.saved"
printf 'pub const SOMETHING_ELSE: u16 = 1;\n' > "$REPO/crates/kings-core/src/proto.rs"
if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh) > "$TMP/proto-noconst.log" 2>&1; then
    bad "a protocol crate with no constant was accepted"
else
    ok "a protocol crate with no constant was refused"
fi
contains "$(cat "$TMP/proto-noconst.log")" "FAILED: crates/kings-core/src/proto.rs declares no PROTO_VERSION" "the refusal names the file rather than raising a traceback"
case "$(cat "$TMP/proto-noconst.log")" in
    *Traceback*) bad "the missing constant produced a Python traceback" ;;
    *)           ok "no traceback reached the operator" ;;
esac
cp "$TMP/kings-proto.saved" "$REPO/crates/kings-core/src/proto.rs"

echo "== a deleted protocol crate is named too =="
mv "$REPO/crates/kings-core/src/proto.rs" "$TMP/kings-proto.moved"
if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh) > "$TMP/proto-nocrate.log" 2>&1; then
    bad "a live protocol whose crate was deleted was accepted"
else
    ok "a live protocol whose crate was deleted was refused"
fi
contains "$(cat "$TMP/proto-nocrate.log")" "no crates/kings-core/src/proto.rs to check it against" "the refusal names the crate that went missing"
mv "$TMP/kings-proto.moved" "$REPO/crates/kings-core/src/proto.rs"

echo "== a malformed mirror binding file fails closed =="
# This one file decides which third-party URLs every player's page will fetch.
# A typo in it must stop the release rather than be skipped into a book that
# silently binds nothing, or binds a name nobody authorised.
cp "$REPO/web/mirrors.json" "$TMP/mirrors.saved"
for fixture in not-json not-a-list bad-name bad-url duplicate; do
    case "$fixture" in
        not-json)   printf '[{"name": "lundi",\n' > "$REPO/web/mirrors.json" ;;
        not-a-list) printf '{"name":"lundi","url":"https://source.example/lundi.json"}\n' > "$REPO/web/mirrors.json" ;;
        bad-name)   printf '[{"name":"Lundi Host","url":"https://source.example/lundi.json"}]\n' > "$REPO/web/mirrors.json" ;;
        bad-url)    printf '[{"name":"lundi","url":"lundi.json"}]\n' > "$REPO/web/mirrors.json" ;;
        duplicate)  printf '[{"name":"lundi","url":"https://a.example/l.json"},{"name":"lundi","url":"https://b.example/l.json"}]\n' > "$REPO/web/mirrors.json" ;;
    esac
    if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 EMBER_PAGES_ARCHIVE="$TMP/mirrors-$fixture.tar.gz" bash deploy/deploy-pages.sh) > "$TMP/mirrors-$fixture.log" 2>&1; then
        bad "the $fixture mirror binding file was accepted"
    else
        ok "the $fixture mirror binding file was refused"
    fi
    contains "$(cat "$TMP/mirrors-$fixture.log")" "mirrors.json" "the $fixture refusal names the file"
    if [ -f "$TMP/mirrors-$fixture.tar.gz" ]; then bad "$fixture mirror bindings still produced an archive"; else ok "$fixture mirror bindings produced no archive"; fi
done
contains "$(cat "$TMP/mirrors-duplicate.log")" "binds lundi twice; a host has one mirror" "a repeated name is refused by name, not silently resolved"
printf '[]\n' > "$REPO/web/mirrors.json"
if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh) > "$TMP/mirrors-empty.log" 2>&1; then
    ok "an empty binding list is a legitimate release that binds no new mirror"
else
    bad "an empty binding list was refused"
    tail -20 "$TMP/mirrors-empty.log" >&2
fi
contains "$(cat "$TMP/mirrors-empty.log")" "bound mirrors: quiet-egret, lundi" "and leaves the seed's own bindings alone"
cp "$TMP/mirrors.saved" "$REPO/web/mirrors.json"

echo "== a missing League page is refused before any archive =="
mv "$REPO/web/$LEAGUE_LIVE/index.html" "$TMP/league-index.saved"
: > "$SHIM_LOG"
if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh) > "$TMP/missing-league.log" 2>&1; then
    bad "a missing live League page was accepted"
else
    ok "a missing live League page was refused"
fi
contains "$(cat "$TMP/missing-league.log")" "$LEAGUE_LIVE/index.html" "the failure identifies the missing League page"
if grep -q '^git \[push\]' "$SHIM_LOG"; then bad "missing League page attempted a branch push"; else ok "missing League page attempted no branch push"; fi
mv "$TMP/league-index.saved" "$REPO/web/$LEAGUE_LIVE/index.html"

echo "== a missing Arena controls script is refused before any archive =="
mv "$REPO/web/$ARENA_LIVE/settings.js" "$TMP/settings.js.saved"
: > "$SHIM_LOG"
if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh) > "$TMP/missing-settings.log" 2>&1; then
    bad "a live Arena page without settings.js was accepted"
else
    ok "a live Arena page without settings.js was refused"
fi
contains "$(cat "$TMP/missing-settings.log")" "settings.js" "the missing-controls failure names settings.js"
if grep -q '^git \[push\]' "$SHIM_LOG"; then bad "missing settings.js attempted a branch push"; else ok "missing settings.js attempted no branch push"; fi
mv "$TMP/settings.js.saved" "$REPO/web/$ARENA_LIVE/settings.js"

echo "== the Arena cache-token contract fails closed =="
cp "$REPO/web/$ARENA_LIVE/index.html" "$TMP/arena-index.saved"
for fixture in missing duplicate two-digit; do
    case "$fixture" in
        missing) printf 'arena live<script src="./settings.js"></script>\n' > "$REPO/web/$ARENA_LIVE/index.html" ;;
        duplicate) printf '<script src="./settings.js?v=1"></script><script src="./settings.js?v=1"></script>\n' > "$REPO/web/$ARENA_LIVE/index.html" ;;
        two-digit) printf 'arena live<script src="./settings.js?v=10"></script>\n' > "$REPO/web/$ARENA_LIVE/index.html" ;;
    esac
    : > "$SHIM_LOG"
    if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh) > "$TMP/settings-$fixture.log" 2>&1; then
        bad "the $fixture Arena settings cache token was accepted"
    else
        ok "the $fixture Arena settings cache token was refused"
    fi
    contains "$(cat "$TMP/settings-$fixture.log")" "Arena settings cache key must occur exactly once" "the $fixture cache-token failure explains the contract"
    if grep -q '^git \[push\]' "$SHIM_LOG"; then bad "$fixture settings cache token attempted a branch push"; else ok "$fixture settings cache token attempted no branch push"; fi
done
cp "$TMP/arena-index.saved" "$REPO/web/$ARENA_LIVE/index.html"

echo "== seed-only references and source symlinks are refused =="
cp "$REPO/web/$LEAGUE_LIVE/index.html" "$TMP/league-index.saved"
printf '<img src="../v1/seed-only.webp">\n' >> "$REPO/web/$LEAGUE_LIVE/index.html"
if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh) > "$TMP/seed-only.log" 2>&1; then
    bad "a live reference supplied only by the seed was accepted"
else
    ok "a live reference supplied only by the seed was refused"
fi
contains "$(cat "$TMP/seed-only.log")" "$LEAGUE_LIVE/index.html references ../v1/seed-only.webp, which this assembly did not place" "the seed-only refusal names the live edge"
mv "$TMP/league-index.saved" "$REPO/web/$LEAGUE_LIVE/index.html"

ln -s index.html "$REPO/web/$END_GAME_LIVE/linked.html"
printf 'web/%s/linked.html\n' "$END_GAME_LIVE" >> "$SHIM_GIT_TRACKED"
if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh) > "$TMP/source-symlink.log" 2>&1; then
    bad "a tracked symlink under a live tree was accepted"
else
    ok "a tracked symlink under a live tree was refused"
fi
contains "$(cat "$TMP/source-symlink.log")" "live source contains a symlink" "the live-tree symlink refusal identifies its cause"
rm "$REPO/web/$END_GAME_LIVE/linked.html"
sed -i "\\|^web/$END_GAME_LIVE/linked.html$|d" "$SHIM_GIT_TRACKED"

mv "$REPO/web/games" "$REPO/tracked-games"
ln -s ../tracked-games "$REPO/web/games"
if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh) > "$TMP/source-ancestor-symlink.log" 2>&1; then
    bad "a symlinked web/games source ancestor was accepted"
else
    ok "a symlinked web/games source ancestor was refused"
fi
contains "$(cat "$TMP/source-ancestor-symlink.log")" "live source contains a symlink: web/games" "the ancestor symlink refusal walks to the repository root"
rm "$REPO/web/games"
mv "$REPO/tracked-games" "$REPO/web/games"

echo "== a statically imported module the deploy does not ship is refused =="
printf 'import { openLab } from "./lab.js?v=1"; import nowhere from "./nowhere.js?v=1";\n' > "$REPO/web/labs/julibrot/main.js"
: > "$SHIM_LOG"
if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh) > "$TMP/unshipped.log" 2>&1; then
    bad "a page importing a module the deploy does not ship was accepted"
else
    ok "a page importing a module the deploy does not ship was refused"
fi
contains "$(cat "$TMP/unshipped.log")" "main.js references ./nowhere.js" "the failure names the page and the module it cannot resolve"

echo "== dropping the lab module from the copy list is refused =="
printf 'import { openLab } from "./lab.js?v=1";\n' > "$REPO/web/labs/julibrot/main.js"
"$PY" - "$REPO/deploy/deploy-pages.sh" <<'PY'
import pathlib, sys
p = pathlib.Path(sys.argv[1])
text = p.read_text(encoding="utf-8")
old = 'if relative.name == "README.md":\n        continue'
assert text.count(old) == 1
with open(p, "w", encoding="utf-8", newline="") as fh:
    fh.write(text.replace(old, 'if relative.name in {"README.md", "lab.js"}:\n        continue'))
PY
: > "$SHIM_LOG"
if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh) > "$TMP/unshipped-lab.log" 2>&1; then
    bad "a deploy that omits the statically imported lab module was accepted"
else
    ok "a deploy that omits the statically imported lab module was refused"
fi
cp "$DEPLOY/deploy-pages.sh" "$REPO/deploy/"

echo "== unshipped dynamic imports and direct worker entries are refused =="
cp "$REPO/web/labs/julibrot/worker.js" "$TMP/worker.saved"
printf 'await import("./pkg/missing-worker.js?v=1");\n' > "$REPO/web/labs/julibrot/worker.js"
if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh) > "$TMP/unshipped-worker-import.log" 2>&1; then
    bad "an unshipped module imported only by worker.js was accepted"
else
    ok "an unshipped module imported only by worker.js was refused"
fi
contains "$(cat "$TMP/unshipped-worker-import.log")" "worker.js references ./pkg/missing-worker.js" "the dynamic-import refusal names worker.js and its target"
mv "$TMP/worker.saved" "$REPO/web/labs/julibrot/worker.js"

cp "$REPO/web/labs/julibrot/lab.js" "$TMP/lab.saved"
printf 'new Worker("./missing-worker.js?v=1"); import("./pkg/ember_lab_julibrot.js?v=1");\n' > "$REPO/web/labs/julibrot/lab.js"
if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh) > "$TMP/unshipped-worker-entry.log" 2>&1; then
    bad "an unshipped direct worker entry was accepted"
else
    ok "an unshipped direct worker entry was refused"
fi
contains "$(cat "$TMP/unshipped-worker-entry.log")" "lab.js references ./missing-worker.js" "the worker-entry refusal names its source and target"
mv "$TMP/lab.saved" "$REPO/web/labs/julibrot/lab.js"

echo "== every live catalog path must be assembled =="
mkdir -p "$SEED/games/fire/v2"
# One ABOVE this source's Fire protocol, derived for the same reason the
# proto.rs files are: a frozen 2 stopped exercising the downgrade the moment
# Fire's own protocol reached 2, and the suite kept passing while proving
# nothing.
printf '{"protocol":%s,"commit":"peer-release"}\n' "$((FIRE_PROTO + 1))" > "$SEED/games/fire/v2/release.json"
: > "$SHIM_LOG"
if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh) > "$TMP/peer-fire.log" 2>&1; then
    bad "newer independently published Fire was overwritten"
else
    ok "newer independently published Fire is protected"
fi
contains "$(cat "$TMP/peer-fire.log")" "live Fire is newer than this source" "Fire downgrade refusal identifies the source mismatch"
if grep -q '^git \[push\]' "$SHIM_LOG"; then bad "Fire downgrade attempted a branch push"; else ok "Fire downgrade attempted no branch push"; fi
rm "$SEED/games/fire/v2/release.json"

"$PY" - "$REPO/web/games.json" <<'PY'
import json, sys
p = sys.argv[1]
with open(p, encoding="utf-8") as fh:
    d = json.load(fh)
d["games"].append({"id": "not-assembled", "versions": [{"version": "1.0.0", "path": "games/not-assembled/v1/", "live": True}]})
with open(p, "w", encoding="utf-8") as fh:
    json.dump(d, fh)
PY
: > "$SHIM_LOG"
if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh) > "$TMP/drift.log" 2>&1; then
    bad "a live catalog path absent from the Pages tree was accepted"
else
    ok "a missing live catalog path was refused"
fi
contains "$(cat "$TMP/drift.log")" "games/not-assembled/v1/" "the drift failure names the live path"

summary pages
