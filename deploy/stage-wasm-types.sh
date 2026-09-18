#!/usr/bin/env bash
# Build wasm-bindgen declarations required by statically typed web templates.
# Usage: deploy/stage-wasm-types.sh [--check|--prebuilt] [source-sha]
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
cd "$ROOT"

MODE=build
if [ "${1:-}" = --check ]; then
    MODE=check
    shift
elif [ "${1:-}" = --prebuilt ]; then
    MODE=prebuilt
    shift
fi

SOURCE_SHA="${1:-${EMBER_SOURCE_SHA:-}}"

# package:wasm output stem:wasm-bindgen output directory
# Later page lanes append one entry here per converted game.
WASM_TYPE_CRATES=(
    "fire:fire:web/pkg"
)

fail() {
    echo "stage-wasm-types: $*" >&2
    exit 1
}

PY=""
for candidate in python3 python; do
    command_path="$(command -v "$candidate" 2>/dev/null)" || continue
    "$command_path" -c "pass" >/dev/null 2>&1 || continue
    PY="$command_path"
    break
done
[ -n "$PY" ] || fail "Python is unavailable"

if [ "$MODE" != prebuilt ]; then
    command -v cargo >/dev/null 2>&1 || fail "cargo is unavailable"
    command -v wasm-bindgen >/dev/null 2>&1 || fail "wasm-bindgen is unavailable"
    LOCKED_WASM_BINDGEN="$($PY - Cargo.lock <<'PY'
import pathlib
import sys
import tomllib

with pathlib.Path(sys.argv[1]).open("rb") as stream:
    lock = tomllib.load(stream)
versions = {package["version"] for package in lock["package"] if package["name"] == "wasm-bindgen"}
if len(versions) != 1:
    raise SystemExit("Cargo.lock does not contain exactly one wasm-bindgen version")
print(versions.pop())
PY
)"
    INSTALLED_WASM_BINDGEN="$(wasm-bindgen --version | awk '{print $2}')"
    [ "$INSTALLED_WASM_BINDGEN" = "$LOCKED_WASM_BINDGEN" ] \
        || fail "wasm-bindgen $INSTALLED_WASM_BINDGEN does not match Cargo.lock $LOCKED_WASM_BINDGEN"
fi

if [ "$MODE" = check ]; then
    target_lib="$(rustc --print target-libdir --target wasm32-unknown-unknown 2>/dev/null)" \
        || fail "the wasm32-unknown-unknown target is unavailable"
    [ -d "$target_lib" ] || fail "the wasm32-unknown-unknown target is unavailable"
    exit 0
fi

[ -n "$SOURCE_SHA" ] || fail "a source SHA argument or EMBER_SOURCE_SHA is required"
case "$SOURCE_SHA" in
    *[!A-Za-z0-9._-]*|'') fail "unsafe source SHA: $SOURCE_SHA" ;;
esac

CARGO_WASM_RELEASE="${CARGO_TARGET_DIR:-target}/wasm32-unknown-unknown/release"
for spec in "${WASM_TYPE_CRATES[@]}"; do
    IFS=: read -r package stem output_dir <<< "$spec"
    if [ "$MODE" = build ]; then
        cargo build --locked --target wasm32-unknown-unknown --release -p "$package" --lib
        wasm-bindgen --target web --out-dir "$output_dir" "$CARGO_WASM_RELEASE/$stem.wasm"
    fi
    for declaration in "$stem.d.ts" "${stem}_bg.wasm.d.ts"; do
        [ -f "$output_dir/$declaration" ] \
            || fail "missing generated declaration $output_dir/$declaration"
    done
    pkg_types="target/web-generated/$SOURCE_SHA/pkg-types/$stem"
    rm -rf "$pkg_types"
    mkdir -p "$pkg_types"
    cp "$output_dir/$stem.d.ts" "$output_dir/${stem}_bg.wasm.d.ts" "$pkg_types/"
    for destination in \
        "target/web-generated/$SOURCE_SHA/ts/wasm/$stem" \
        "target/web-generated/ts/wasm/$stem"
    do
        mkdir -p "$destination"
        cp "$pkg_types/$stem.d.ts" "$pkg_types/${stem}_bg.wasm.d.ts" "$destination/"
    done
done
