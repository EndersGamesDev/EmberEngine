#!/usr/bin/env bash
# Validate the pinned generated TypeScript pipeline and its failure fixture.
#
#   bash deploy/tests/test-typescript.sh
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
# shellcheck source=deploy/tests/lib.sh
. "$HERE/lib.sh"

cd "$ROOT"

echo "== exact pinned projects and package metadata =="
if "$PY" - package.json package-lock.json tsconfig.web.json tsconfig.worker.json tsconfig.node.json <<'PY'
import json
import pathlib
import sys

package, lock, web, worker, node = [
    json.loads(pathlib.Path(path).read_text(encoding="utf-8")) for path in sys.argv[1:]
]
expected_web = {
    "compilerOptions": {
        "target": "ES2022",
        "module": "ES2022",
        "moduleResolution": "Bundler",
        "lib": ["ES2022", "DOM", "DOM.Iterable"],
        "types": [],
        "strict": True,
        "noUncheckedIndexedAccess": True,
        "exactOptionalPropertyTypes": True,
        "noImplicitOverride": True,
        "noPropertyAccessFromIndexSignature": True,
        "noFallthroughCasesInSwitch": True,
        "useUnknownInCatchVariables": True,
        "verbatimModuleSyntax": True,
        "isolatedModules": True,
        "noImplicitReturns": True,
        "noUnusedLocals": True,
        "noUnusedParameters": True,
        "allowUnreachableCode": False,
        "allowUnusedLabels": False,
        "forceConsistentCasingInFileNames": True,
        "skipLibCheck": False,
        "noEmitOnError": True,
        "sourceMap": False,
        "declaration": False,
        "removeComments": True,
        "newLine": "lf",
        "rootDir": "target/web-generated/ts",
        "outDir": "target/web-generated/js",
    },
    "include": ["target/web-generated/ts/**/*.ts"],
}
expected_worker = {
    "extends": "./tsconfig.web.json",
    "compilerOptions": {
        "lib": ["ES2022", "WebWorker"],
        "rootDir": "target/web-generated/worker-ts",
        "outDir": "target/web-generated/worker-js",
    },
    "include": ["target/web-generated/worker-ts/**/*.ts"],
}
expected_node = {
    "extends": "./tsconfig.web.json",
    "compilerOptions": {
        "module": "Node16",
        "moduleResolution": "Node16",
        "lib": ["ES2022"],
        "types": ["node"],
        "rootDir": "target/web-generated/node-ts",
        "outDir": "target/web-generated/node-js",
    },
    "include": [
        "target/web-generated/node-ts/**/*.mts",
        "target/web-generated/node-ts/**/*.cts",
    ],
}
if web != expected_web or worker != expected_worker or node != expected_node:
    raise SystemExit(1)
if package.get("packageManager") != "npm@11.19.0":
    raise SystemExit(1)
if package.get("engines") != {"node": "24.20.0"}:
    raise SystemExit(1)
if package.get("devDependencies") != {
    "@types/node": "24.13.4",
    "typescript": "5.9.3",
}:
    raise SystemExit(1)
if set(package.get("scripts", {}).values()) != {
    "tsc -p tsconfig.web.json --noEmit",
    "tsc -p tsconfig.worker.json --noEmit",
    "tsc -p tsconfig.node.json --noEmit",
}:
    raise SystemExit(1)
packages = lock.get("packages", {})
if packages.get("node_modules/typescript", {}).get("version") != "5.9.3":
    raise SystemExit(1)
if packages.get("node_modules/@types/node", {}).get("version") != "24.13.4":
    raise SystemExit(1)
PY
then
    ok "package metadata and all three TypeScript projects are exact"
else
    bad "package metadata or a TypeScript project differs from the pinned contract"
fi

echo "== no tracked generated or naked TypeScript outputs =="
while IFS= read -r -d '' path; do
    case "$path" in
        target/*) bad "tracked target output: $path" ;;
        *.d.ts) bad "tracked declaration output: $path" ;;
        *.ts.j2|*.mts.j2|*.cts.j2) ;;
        *.ts|*.mts|*.cts) bad "tracked TypeScript outside a template: $path" ;;
    esac
done < <(git ls-files -z)
ok "tracked outputs were inspected"

skip_execution() {
    local reason="$1"
    if [ "${EMBER_TYPED_WEB_REQUIRED:-}" = 1 ]; then
        echo "ERROR: $reason; typed web execution is required" >&2
        exit 1
    fi
    echo "SKIP: $reason; typed web execution not run"
    summary typescript
    exit 0
}

if ! command -v node >/dev/null 2>&1; then
    skip_execution "node absent"
fi
NODE_MODULES_DIR="${EMBER_TYPED_WEB_NODE_MODULES_DIR:-node_modules}"
if [ ! -x "$NODE_MODULES_DIR/.bin/tsc" ]; then
    skip_execution "node_modules absent"
fi
if ! bash deploy/check-toolchain.sh >/dev/null 2>&1; then
    skip_execution "toolchain version mismatch"
fi

bash deploy/check-toolchain.sh

TMP="$(mktemp -d -t ember-typescript-test-XXXXXX)"
trap 'rm -rf "$TMP"' EXIT
SOURCE_ID="typescript-test"
SCOPED="target/web-generated/$SOURCE_ID"

hash_tree() {
    (cd "$SCOPED" && find . -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum)
}

echo "== generated hashes are stable =="
EMBER_SOURCE_SHA="$SOURCE_ID" cargo run --locked -p ember-webgen -- --out "$SCOPED"
hash_tree > "$TMP/first.sha256"
ok "one generator run produced a hashed tree; Rust tests own repeat-render stability"
if diff -qr "$SCOPED/ts" target/web-generated/ts >/dev/null; then
    ok "fixed compiler inputs exactly match the source-scoped tree"
else
    bad "fixed compiler inputs differ from the source-scoped tree"
fi
npx --no-install tsc -p tsconfig.web.json --noEmit
ok "generated declarations and exhaustive consumers compile"

cp deploy/tests/fixtures/wasm-bindgen-symbol.d.ts.j2 \
    target/web-generated/ts/wasm-bindgen-symbol.d.ts
npx --no-install tsc -p tsconfig.web.json --noEmit
rm -f target/web-generated/ts/wasm-bindgen-symbol.d.ts
ok "current wasm-bindgen disposal declarations compile under ES2022"

echo "== an omitted variant fails exhaustiveness =="
cp deploy/tests/fixtures/missing-variant.ts.j2 target/web-generated/ts/missing-variant.ts
if npx --no-install tsc -p tsconfig.web.json --noEmit > "$TMP/compile-fail.log" 2>&1; then
    bad "the missing-variant consumer compiled"
else
    code="$(grep -oE 'TS[0-9]+' "$TMP/compile-fail.log" | LC_ALL=C sort -u | tr '\n' ' ' | sed 's/ $//')"
    is "$code" "TS2322" "the omitted variant fails with exactly TS2322"
    printf 'compile-fail error code: %s\n' "$code"
fi
rm -f target/web-generated/ts/missing-variant.ts
npx --no-install tsc -p tsconfig.web.json --noEmit
ok "the clean generated tree compiles after the failure fixture"

echo "== unavailable optional toolchains skip for distinct reasons =="
mkdir -p "$TMP/node-free-path" "$TMP/mismatch-path"
for command in date dirname git python3; do
    ln -s "$(command -v "$command")" "$TMP/node-free-path/$command"
done
node_absent="$(EMBER_TYPED_WEB_REQUIRED= PATH="$TMP/node-free-path" /bin/bash "$0")"
contains "$node_absent" "SKIP: node absent; typed web execution not run" \
    "node absence is explicit and successful"

for command in bash date dirname git python3; do
    ln -s "$(command -v "$command")" "$TMP/mismatch-path/$command"
done
ln -s "$ROOT/deploy/tests/shims/node" "$TMP/mismatch-path/node"
ln -s "$ROOT/deploy/tests/shims/npm" "$TMP/mismatch-path/npm"
ln -s "$ROOT/deploy/tests/shims/npx" "$TMP/mismatch-path/npx"
version_mismatch="$(EMBER_TYPED_WEB_REQUIRED= SHIM_NODE_VERSION=v24.19.0 \
    PATH="$TMP/mismatch-path" /bin/bash "$0")"
contains "$version_mismatch" "SKIP: toolchain version mismatch; typed web execution not run" \
    "a version mismatch is explicit and successful"

node_modules_absent="$(EMBER_TYPED_WEB_REQUIRED= \
    EMBER_TYPED_WEB_NODE_MODULES_DIR="$TMP/missing-node-modules" /bin/bash "$0")"
contains "$node_modules_absent" "SKIP: node_modules absent; typed web execution not run" \
    "missing node_modules is explicit and successful"
if EMBER_TYPED_WEB_NODE_MODULES_DIR="$TMP/missing-node-modules" \
    EMBER_TYPED_WEB_REQUIRED=1 /bin/bash "$0" > "$TMP/required.log" 2>&1
then
    bad "required typed-web execution accepted missing node_modules"
else
    required="$(cat "$TMP/required.log")"
    contains "$required" "ERROR: node_modules absent; typed web execution is required" \
        "required typed-web execution fails on missing node_modules"
fi

summary typescript
