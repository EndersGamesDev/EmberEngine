#!/usr/bin/env bash
# deploy-pages.sh against cargo, wasm-bindgen and git shims.
#
#   bash deploy/tests/test-pages.sh
#
# Nothing here contacts a network, compiles wasm or pushes a branch. The git
# shim captures the assembled Pages tree where a push would have occurred.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
DEPLOY="$(cd "$HERE/.." && pwd)"
# shellcheck source=deploy/tests/lib.sh
. "$HERE/lib.sh"

# Exercise the production live slot without freezing a second release number
# into the fixture. The independent catalog check below still catches a
# disagreement between deploy-pages.sh and games.json.
ARENA_LIVE="$(sed -n 's/^ARENA_LIVE="\([^"]*\)"$/\1/p' "$DEPLOY/deploy-pages.sh" | tr -d '\r')"
if [[ ! "$ARENA_LIVE" =~ ^games/arena/v[0-9]+$ ]]; then
    echo "pages fixture: cannot determine deploy-pages.sh's live arena path" >&2
    exit 1
fi

TMP="$(mktemp -d -t ember-pagestest-XXXXXX)"
trap 'rm -rf "$TMP"' EXIT
REPO="$TMP/repo"
SEED="$TMP/pages-seed"
EXPECTED="$TMP/expected"
export SHIM_PUBLISHED="$TMP/published"
export SHIM_GIT_INDEX="$TMP/git-index"
export SHIM_LOG="$TMP/argv.log"

SHIMS="$TMP/shims"
mkdir -p "$SHIMS"
for shim in cargo wasm-bindgen git; do
    cp "$HERE/shims/$shim" "$SHIMS/$shim"
    chmod +x "$SHIMS/$shim"
done
ln -s "$PY" "$SHIMS/python3"
ln -s "$PY" "$SHIMS/python"
export PATH="$SHIMS:$PATH"

mkdir -p "$REPO/deploy" "$REPO/web/$ARENA_LIVE" "$REPO/web/games/arena/v0"
mkdir -p "$REPO/web/games/fire/v2" "$REPO/web/games/kings/v1" "$REPO/web/games/what-is-this/v1"
mkdir -p "$REPO/web/labs/julibrot/pkg"
mkdir -p "$REPO/crates/arena-core/src" "$REPO/crates/fire-core/src" "$REPO/crates/kings-core/src"
cp "$DEPLOY/deploy-pages.sh" "$DEPLOY/stamp-version.sh" "$DEPLOY/publish-host.sh" "$REPO/deploy/"
# Give recompute a deterministic stamp distinct from deploy-pages.sh's first
# stamp, so the test proves loaders read server.json only after recompute.
"$PY" - "$REPO/deploy/publish-host.sh" <<'PY'
import pathlib, sys
p = pathlib.Path(sys.argv[1])
text = p.read_text(encoding="utf-8")
old = 'doc["v"] = str(int(time.time()))'
assert text.count(old) == 1
with open(p, "w", encoding="utf-8", newline="") as fh:
    fh.write(text.replace(old, 'doc["v"] = "recomputed-stamp"'))
PY
cp "$DEPLOY/../web/games.json" "$REPO/web/games.json"
printf 'hub\n' > "$REPO/web/index.html"
printf '{}\n' > "$REPO/web/version.json"
printf 'arena live<script src="./settings.js?v=1"></script>\n' > "$REPO/web/$ARENA_LIVE/index.html"
printf 'arena controls\n' > "$REPO/web/$ARENA_LIVE/settings.js"
printf 'arena v0\n' > "$REPO/web/games/arena/v0/index.html"
printf 'fire v2\n' > "$REPO/web/games/fire/v2/index.html"
printf 'kings v1\n' > "$REPO/web/games/kings/v1/index.html"
printf 'what is this v1\n' > "$REPO/web/games/what-is-this/v1/index.html"
printf '<link href="./style.css?v=1"><script src="./main.js?v=1"></script>\n' > "$REPO/web/labs/julibrot/index.html"
# main.js imports lab.js statically, exactly as the shipped page does: the
# fixture has to carry the same import for the assembly check to mean anything.
printf 'import { openLab } from "./lab.js?v=1"; fetch("./future.js?v=10");\n' > "$REPO/web/labs/julibrot/main.js"
printf 'globalThis.JULIBROT_WORKER_URL = "./worker.js?v=1"; import("./pkg/ember_lab_julibrot.js?v=1"); fetch("./pkg/ember_lab_julibrot_bg.wasm?v=1"); export function openLab() {}\n' > "$REPO/web/labs/julibrot/lab.js"
printf '<canvas id="julibrot"></canvas><script type="module">import { openLab } from "./lab.js?v=1";</script>\n' > "$REPO/web/labs/julibrot/drive.html"
printf 'import("./pkg/ember_lab_julibrot.js?v=1"); fetch("./pkg/ember_lab_julibrot_bg.wasm?v=1");\n' > "$REPO/web/labs/julibrot/worker.js"
printf 'julibrot style\n' > "$REPO/web/labs/julibrot/style.css"
printf 'pub const PROTO_VERSION: u16 = 15;\n' > "$REPO/crates/arena-core/src/proto.rs"
printf 'pub const PROTO_VERSION: u16 = 1;\n' > "$REPO/crates/fire-core/src/proto.rs"
printf 'pub const PROTO_VERSION: u16 = 1;\n' > "$REPO/crates/kings-core/src/proto.rs"

mkdir -p "$SEED/games/arena/v17" "$SEED/games/fire/v1" "$SEED/games/kings/old" "$SEED/games/pong/v1/pkg"
printf 'keep arena\n' > "$SEED/games/arena/v17/frozen.txt"
printf 'archived arena<script src="./settings.js?v=archived"></script>\n' > "$SEED/games/arena/v17/index.html"
printf 'archived controls\n' > "$SEED/games/arena/v17/settings.js"
printf 'keep fire\n' > "$SEED/games/fire/v1/frozen.txt"
printf 'keep kings\n' > "$SEED/games/kings/old/frozen.txt"
printf 'frozen pong\n' > "$SEED/games/pong/v1/index.html"
printf 'frozen pong js\n' > "$SEED/games/pong/v1/pkg/pong.js"
printf 'frozen pong wasm\n' > "$SEED/games/pong/v1/pkg/pong_bg.wasm"
printf '{"v":"seed","proto":14,"ws":"wss://old.example","hosts":[{"name":"new-host","ws":"wss://new.example","proto":15,"version":"r2"}]}\n' > "$SEED/server.json"
export SHIM_PAGES_SEED="$SEED"

echo "== ordinary build assembles all four games and the Julibrot lab =="
: > "$SHIM_LOG"
rm -f "$SHIM_GIT_INDEX"
if (cd "$REPO" && bash deploy/deploy-pages.sh) > "$TMP/build.log" 2>&1; then
    ok "the shimmed build-and-publish run succeeded"
else
    bad "the shimmed build-and-publish run failed"
    tail -40 "$TMP/build.log" >&2
fi
ARGV="$(cat "$SHIM_LOG")"
contains "$ARGV" "cargo [build] [--target] [wasm32-unknown-unknown] [--release] [-p] [what-is-this] [--lib]" "what-is-this is built as a wasm library"
contains "$ARGV" "release/what_is_this.wasm" "what-is-this is passed to wasm-bindgen"
contains "$ARGV" "cargo [build] [--target] [wasm32-unknown-unknown] [--release] [-p] [ember-julibrot-app] [--lib]" "Julibrot is built as a wasm library"
contains "$ARGV" "[--out-dir] [web/labs/julibrot/pkg]" "Julibrot wasm-bindgen output stays in the lab"
contains "$ARGV" "release/ember_lab_julibrot.wasm" "Julibrot artifact is passed to wasm-bindgen"
for f in index.html pkg/what_is_this.js pkg/what_is_this_bg.wasm; do
    if [ -f "$SHIM_PUBLISHED/games/what-is-this/v1/$f" ]; then
        ok "assembled what-is-this $f"
    else
        bad "assembled what-is-this is missing $f"
    fi
done
for f in index.html main.js lab.js drive.html worker.js style.css pkg/ember_lab_julibrot.js pkg/ember_lab_julibrot_bg.wasm; do
    if [ -f "$SHIM_PUBLISHED/labs/julibrot/$f" ]; then
        ok "assembled Julibrot $f"
    else
        bad "assembled Julibrot is missing $f"
    fi
done
STAMP="$(jget "$SHIM_PUBLISHED/server.json" 'd["v"]')"
is "$STAMP" "recomputed-stamp" "address recompute changed the deploy stamp"
is "$(jget "$SHIM_PUBLISHED/server.json" 'd["ws"]')" "wss://new.example" "address recompute changed the legacy address"
if cmp -s "$REPO/web/$ARENA_LIVE/settings.js" "$SHIM_PUBLISHED/$ARENA_LIVE/settings.js"; then
    ok "Arena settings.js is copied byte-for-byte beside its live page"
else
    bad "Arena settings.js was omitted or changed"
fi
contains "$(cat "$SHIM_PUBLISHED/$ARENA_LIVE/index.html")" "src=\"./settings.js?v=$STAMP\"" "Arena settings loader uses the final recomputed deploy stamp"
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
for f in index.html main.js lab.js drive.html worker.js; do
    contains "$(cat "$SHIM_PUBLISHED/labs/julibrot/$f")" "?v=$STAMP" "Julibrot $f uses the deploy stamp"
    if grep -qE '\?v=1([^0-9]|$)' "$SHIM_PUBLISHED/labs/julibrot/$f"; then
        bad "assembled Julibrot $f retained ?v=1"
    else
        ok "assembled Julibrot $f has no stale ?v=1 cache key"
    fi
    if grep -qE '\?v=1([^0-9]|$)' "$REPO/web/labs/julibrot/$f"; then
        ok "Julibrot source $f remains pinned at ?v=1"
    else
        bad "Julibrot source $f was rewritten"
    fi
done
contains "$(cat "$SHIM_PUBLISHED/labs/julibrot/lab.js")" "JULIBROT_WORKER_URL = \"./worker.js?v=$STAMP\"" "the worker bootstrap URL uses the deploy stamp"
contains "$(cat "$SHIM_PUBLISHED/labs/julibrot/main.js")" "./future.js?v=10" "a future two-digit cache key is not partly rewritten"
contains "$(cat "$SHIM_PUBLISHED/labs/julibrot/main.js")" "from \"./lab.js?v=$STAMP\"" "the page's static import of the lab module is stamped"
is "$(jget "$SHIM_PUBLISHED/games.json" '[v["path"] for g in d["games"] if g.get("kind") == "lab" for v in g["versions"] if v.get("live")][0]')" "labs/julibrot/" "the live Julibrot catalog path was published"

mkdir -p "$EXPECTED"
cp -R "$SEED/games" "$EXPECTED/"
for spec in "${ARENA_LIVE#games/} arena" "arena/v0 arena" "fire/v2 fire" "kings/v1 kings"; do
    # shellcheck disable=SC2086
    set -- $spec
    live="$1"
    bundle="$2"
    rm -rf "$EXPECTED/games/$live"
    mkdir -p "$EXPECTED/games/$live/pkg"
    cp "$REPO/web/games/$live/index.html" "$EXPECTED/games/$live/"
    if [ "games/$live" = "$ARENA_LIVE" ]; then
        # Expected output is authored independently of the publisher's rewrite.
        printf 'arena live<script src="./settings.js?v=%s"></script>\n' "$STAMP" > "$EXPECTED/games/$live/index.html"
        cp "$REPO/web/games/$live/settings.js" "$EXPECTED/games/$live/"
    fi
    printf 'shim js for %s\n' "$bundle" > "$EXPECTED/games/$live/pkg/$bundle.js"
    printf 'shim wasm for %s\n' "$bundle" > "$EXPECTED/games/$live/pkg/${bundle}_bg.wasm"
done
for game in arena fire kings; do
    if diff -r "$EXPECTED/games/$game" "$SHIM_PUBLISHED/games/$game" > "$TMP/$game.diff"; then
        ok "$game Pages tree is unchanged"
    else
        bad "$game Pages tree changed"
        cat "$TMP/$game.diff" >&2
    fi
done

echo "== prebuilt mode fails closed on missing game and Julibrot artifacts =="
rm "$REPO/web/pkg/what_is_this_bg.wasm" "$REPO/web/labs/julibrot/pkg/ember_lab_julibrot.js" "$REPO/web/labs/julibrot/pkg/ember_lab_julibrot_bg.wasm"
: > "$SHIM_LOG"
if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh) > "$TMP/missing.log" 2>&1; then
    bad "prebuilt mode accepted a missing artifact"
else
    ok "prebuilt mode refused a missing artifact"
fi
contains "$(cat "$TMP/missing.log")" "web/labs/julibrot/pkg/ember_lab_julibrot.js" "the failure lists the missing Julibrot JavaScript"
contains "$(cat "$TMP/missing.log")" "web/labs/julibrot/pkg/ember_lab_julibrot_bg.wasm" "the failure lists the missing Julibrot wasm"
contains "$(cat "$TMP/missing.log")" "web/pkg/what_is_this_bg.wasm" "the failure lists the missing game wasm"
if grep -q '^cargo' "$SHIM_LOG"; then bad "the refused prebuilt run invoked cargo"; else ok "the refused prebuilt run invoked no cargo"; fi

echo "== complete prebuilt mode skips every build tool =="
printf 'shim wasm for what_is_this\n' > "$REPO/web/pkg/what_is_this_bg.wasm"
printf 'shim js for ember_lab_julibrot\n' > "$REPO/web/labs/julibrot/pkg/ember_lab_julibrot.js"
printf 'shim wasm for ember_lab_julibrot\n' > "$REPO/web/labs/julibrot/pkg/ember_lab_julibrot_bg.wasm"
: > "$SHIM_LOG"
rm -f "$SHIM_GIT_INDEX"
if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh) > "$TMP/prebuilt.log" 2>&1; then
    ok "complete prebuilt mode published"
else
    bad "complete prebuilt mode failed"
    tail -40 "$TMP/prebuilt.log" >&2
fi
if grep -Eq '^(cargo|wasm-bindgen)' "$SHIM_LOG"; then bad "complete prebuilt mode invoked a build tool"; else ok "complete prebuilt mode invoked no build tool"; fi

echo "== a missing Arena controls script is refused before any publish =="
mv "$REPO/web/$ARENA_LIVE/settings.js" "$TMP/settings.js.saved"
: > "$SHIM_LOG"
if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh) > "$TMP/missing-settings.log" 2>&1; then
    bad "a live Arena page without settings.js was accepted"
else
    ok "a live Arena page without settings.js was refused"
fi
contains "$(cat "$TMP/missing-settings.log")" "settings.js" "the missing-controls failure names settings.js"
if grep -q '^git \[push\]' "$SHIM_LOG"; then bad "missing settings.js reached a publish"; else ok "missing settings.js never reached a publish"; fi
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
    if grep -q '^git \[push\]' "$SHIM_LOG"; then bad "$fixture settings cache token reached a publish"; else ok "$fixture settings cache token never reached a publish"; fi
done
cp "$TMP/arena-index.saved" "$REPO/web/$ARENA_LIVE/index.html"

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
old = '"web/$LAB_JULIBROT_LIVE/lab.js" "web/$LAB_JULIBROT_LIVE/drive.html" \\\n    '
assert text.count(old) == 1
with open(p, "w", encoding="utf-8", newline="") as fh:
    fh.write(text.replace(old, ""))
PY
: > "$SHIM_LOG"
if (cd "$REPO" && EMBER_PAGES_PREBUILT=1 bash deploy/deploy-pages.sh) > "$TMP/unshipped-lab.log" 2>&1; then
    bad "a deploy that omits the statically imported lab module was accepted"
else
    ok "a deploy that omits the statically imported lab module was refused"
fi
cp "$DEPLOY/deploy-pages.sh" "$REPO/deploy/"

echo "== every live catalog path must be assembled =="
"$PY" - "$REPO/web/games.json" <<'PY'
import json, sys
p = sys.argv[1]
with open(p, encoding="utf-8") as fh:
    d = json.load(fh)
d["games"].append({"id": "not-assembled", "versions": [{"path": "games/not-assembled/v1/", "live": True}]})
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
