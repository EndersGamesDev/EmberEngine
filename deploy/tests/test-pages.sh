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

mkdir -p "$REPO/deploy" "$REPO/web/games/arena/v20" "$REPO/web/games/arena/v0"
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
printf 'arena v20\n' > "$REPO/web/games/arena/v20/index.html"
printf 'arena v0\n' > "$REPO/web/games/arena/v0/index.html"
printf 'fire v2\n' > "$REPO/web/games/fire/v2/index.html"
printf 'kings v1\n' > "$REPO/web/games/kings/v1/index.html"
printf 'what is this v1\n' > "$REPO/web/games/what-is-this/v1/index.html"
printf '<link href="./style.css?v=1"><script src="./main.js?v=1"></script>\n' > "$REPO/web/labs/julibrot/index.html"
printf 'globalThis.JULIBROT_WORKER_URL = "./worker.js?v=1"; import("./pkg/ember_lab_julibrot.js?v=1"); fetch("./pkg/ember_lab_julibrot_bg.wasm?v=1"); fetch("./future.js?v=10");\n' > "$REPO/web/labs/julibrot/main.js"
printf 'import("./pkg/ember_lab_julibrot.js?v=1"); fetch("./pkg/ember_lab_julibrot_bg.wasm?v=1");\n' > "$REPO/web/labs/julibrot/worker.js"
printf 'julibrot style\n' > "$REPO/web/labs/julibrot/style.css"
printf 'pub const PROTO_VERSION: u16 = 15;\n' > "$REPO/crates/arena-core/src/proto.rs"
printf 'pub const PROTO_VERSION: u16 = 1;\n' > "$REPO/crates/fire-core/src/proto.rs"
printf 'pub const PROTO_VERSION: u16 = 1;\n' > "$REPO/crates/kings-core/src/proto.rs"

mkdir -p "$SEED/games/arena/v17" "$SEED/games/fire/v1" "$SEED/games/kings/old" "$SEED/games/pong/v1/pkg"
printf 'keep arena\n' > "$SEED/games/arena/v17/frozen.txt"
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
for f in index.html main.js worker.js style.css pkg/ember_lab_julibrot.js pkg/ember_lab_julibrot_bg.wasm; do
    if [ -f "$SHIM_PUBLISHED/labs/julibrot/$f" ]; then
        ok "assembled Julibrot $f"
    else
        bad "assembled Julibrot is missing $f"
    fi
done
STAMP="$(jget "$SHIM_PUBLISHED/server.json" 'd["v"]')"
is "$STAMP" "recomputed-stamp" "address recompute changed the deploy stamp"
is "$(jget "$SHIM_PUBLISHED/server.json" 'd["ws"]')" "wss://new.example" "address recompute changed the legacy address"
for f in index.html main.js worker.js; do
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
contains "$(cat "$SHIM_PUBLISHED/labs/julibrot/main.js")" "JULIBROT_WORKER_URL = \"./worker.js?v=$STAMP\"" "the worker bootstrap URL uses the deploy stamp"
contains "$(cat "$SHIM_PUBLISHED/labs/julibrot/main.js")" "./future.js?v=10" "a future two-digit cache key is not partly rewritten"
is "$(jget "$SHIM_PUBLISHED/games.json" '[v["path"] for g in d["games"] if g.get("kind") == "lab" for v in g["versions"] if v.get("live")][0]')" "labs/julibrot/" "the live Julibrot catalog path was published"

mkdir -p "$EXPECTED"
cp -R "$SEED/games" "$EXPECTED/"
for spec in "arena/v20 arena" "arena/v0 arena" "fire/v2 fire" "kings/v1 kings"; do
    # shellcheck disable=SC2086
    set -- $spec
    live="$1"
    bundle="$2"
    rm -rf "$EXPECTED/games/$live"
    mkdir -p "$EXPECTED/games/$live/pkg"
    cp "$REPO/web/games/$live/index.html" "$EXPECTED/games/$live/"
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
