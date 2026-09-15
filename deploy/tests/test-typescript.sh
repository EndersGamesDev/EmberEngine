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

echo "== converted modules have no tracked JavaScript twin =="
converted_javascript=(
    web/hosts.js
    web/loader.js
    web/hosts.test.mjs
    web/loader.test.mjs
    deploy/check-hosts.mjs
    deploy/check-hosts.test.mjs
)
for path in "${converted_javascript[@]}"; do
    if [ -n "$(git ls-files -- "$path")" ]; then
        bad "converted JavaScript remains tracked: $path"
    fi
done
frozen_javascript=0
while IFS= read -r path; do
    case "$path" in
        web/games/*/v*/*|web/labs/*|tools/*) frozen_javascript=$((frozen_javascript + 1)) ;;
    esac
done < <(git ls-files '*.js' '*.mjs' '*.cjs')
[ "$frozen_javascript" -gt 0 ] || bad "the frozen JavaScript allowlist matched no files"
ok "converted names are absent and the phase allowlist preserves $frozen_javascript frozen files"

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

source_ast_gate() {
    node - "$@" <<'NODE'
const fs = require('node:fs');
const path = require('node:path');
const ts = require('typescript');

const files = process.argv.slice(2);
const failures = [];
const checkedDeclarations = new Set();

function location(source, position) {
  const point = source.getLineAndCharacterOfPosition(position);
  return `${source.fileName}:${point.line + 1}:${point.character + 1}`;
}

function reject(source, node, message) {
  failures.push(`${location(source, node.getStart(source))}: ${message}`);
}

function unwrap(expression) {
  let current = expression;
  while (ts.isParenthesizedExpression(current)) current = current.expression;
  return current;
}

function declarationCandidates(sourcePath, specifier) {
  const resolved = path.resolve(path.dirname(sourcePath), specifier);
  const extension = path.extname(resolved);
  const stem = extension ? resolved.slice(0, -extension.length) : resolved;
  return [`${stem}.d.ts`, `${stem}.d.mts`, `${stem}.d.cts`];
}

function scanWasmDeclaration(declaration) {
  if (checkedDeclarations.has(declaration) || !fs.existsSync(declaration)) return;
  checkedDeclarations.add(declaration);
  const text = fs.readFileSync(declaration, 'utf8');
  const source = ts.createSourceFile(declaration, text, ts.ScriptTarget.ES2022, true);
  function visit(node) {
    if (node.kind === ts.SyntaxKind.AnyKeyword) reject(source, node, 'wasm declaration exposes AnyKeyword');
    ts.forEachChild(node, visit);
  }
  visit(source);
}

function checkSpecifier(source, node, specifier) {
  if (!specifier.startsWith('./') && !specifier.startsWith('../')) return;
  const bare = specifier.split(/[?#]/, 1)[0];
  const extension = path.posix.extname(bare);
  if (!extension) reject(source, node, 'relative import must name an emitted extension');
  if (['.ts', '.mts', '.cts'].includes(extension)) {
    reject(source, node, 'relative import names a TypeScript source extension');
  }
  for (const declaration of declarationCandidates(source.fileName, bare)) {
    if (declaration.split(path.sep).includes('wasm')) scanWasmDeclaration(declaration);
  }
}

function scan(file) {
  const text = fs.readFileSync(file, 'utf8');
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.ES2022, true);
  for (const match of text.matchAll(/@ts-(?:ignore|expect-error)/g)) {
    const position = match.index === undefined ? 0 : match.index;
    failures.push(`${location(source, position)}: forbidden directive ${match[0]}`);
  }
  function visit(node) {
    if (node.kind === ts.SyntaxKind.AnyKeyword) reject(source, node, 'forbidden AnyKeyword');
    if (ts.isAsExpression(node)) {
      const inner = unwrap(node.expression);
      if (ts.isAsExpression(inner) && inner.type.kind === ts.SyntaxKind.UnknownKeyword) {
        reject(source, node, 'double assertion through unknown');
      }
    }
    if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node))
      && node.moduleSpecifier && ts.isStringLiteralLike(node.moduleSpecifier)) {
      checkSpecifier(source, node.moduleSpecifier, node.moduleSpecifier.text);
    }
    if (ts.isCallExpression(node) && node.expression.kind === ts.SyntaxKind.ImportKeyword
      && node.arguments.length === 1 && ts.isStringLiteralLike(node.arguments[0])) {
      checkSpecifier(source, node.arguments[0], node.arguments[0].text);
    }
    ts.forEachChild(node, visit);
  }
  visit(source);
}

for (const file of files) scan(file);
if (failures.length) {
  process.stderr.write(`${failures.join('\n')}\n`);
  process.exitCode = 1;
}
NODE
}

echo "== generated hashes are stable =="
EMBER_SOURCE_SHA="$SOURCE_ID" cargo run --locked -p ember-webgen -- --out "$SCOPED"
WEBGEN_BIN="${CARGO_TARGET_DIR:-target}/debug/ember-webgen"
[ -x "$WEBGEN_BIN" ] || bad "the diagnostic mapper binary was not built: $WEBGEN_BIN"
mapped_tsc() {
    npx --no-install tsc "$@" 2>&1 | "$WEBGEN_BIN" --map-diagnostics
}
hash_tree > "$TMP/first.sha256"
ok "one generator run produced a hashed tree; Rust tests own repeat-render stability"
if diff -qr "$SCOPED/ts" target/web-generated/ts >/dev/null; then
    ok "fixed compiler inputs exactly match the source-scoped tree"
else
    bad "fixed compiler inputs differ from the source-scoped tree"
fi

echo "== rendered production sources pass the compiler-API policy =="
mapfile -d '' production_sources < <(
    find target/web-generated/ts target/web-generated/worker-ts target/web-generated/node-ts \
        -type f \( -name '*.ts' -o -name '*.mts' -o -name '*.cts' \) \
        ! -name '*.d.ts' ! -name '*.d.mts' ! -name '*.d.cts' \
        ! -name '*.test.ts' ! -name '*.test.mts' ! -name '*.test.cts' \
        -print0 2>/dev/null | LC_ALL=C sort -z
)
if [ "${#production_sources[@]}" -eq 0 ]; then
    bad "the generator rendered no production TypeScript sources"
elif source_ast_gate "${production_sources[@]}"; then
    ok "every rendered production source passes the compiler-API policy"
else
    bad "a rendered production source violates the compiler-API policy"
fi

echo "== compiler-API policy rejection fixtures =="
mkdir -p "$TMP/ast/wasm"
ast_fixture() {
    local name="$1" expected="$2"
    cp "deploy/tests/fixtures/$name.ts.j2" "$TMP/ast/$name.ts"
    if source_ast_gate "$TMP/ast/$name.ts" > "$TMP/$name.log" 2>&1; then
        bad "$name was accepted by the compiler-API policy"
    else
        contains "$(cat "$TMP/$name.log")" "$expected" "$name exercises $expected"
    fi
}
ast_fixture typescript-any "forbidden AnyKeyword"
ast_fixture typescript-ignore "forbidden directive @ts-ignore"
ast_fixture typescript-expect-error "forbidden directive @ts-expect-error"
ast_fixture typescript-double-assertion "double assertion through unknown"
ast_fixture typescript-extensionless-import "relative import must name an emitted extension"
ast_fixture typescript-source-import "relative import names a TypeScript source extension"
cp deploy/tests/fixtures/typescript-wasm-any.ts.j2 "$TMP/ast/typescript-wasm-any.ts"
cp deploy/tests/fixtures/typescript-wasm-any.d.ts.j2 "$TMP/ast/wasm/fake.d.ts"
if source_ast_gate "$TMP/ast/typescript-wasm-any.ts" > "$TMP/typescript-wasm-any.log" 2>&1; then
    bad "a wasm declaration exposing AnyKeyword was accepted"
else
    contains "$(cat "$TMP/typescript-wasm-any.log")" "wasm declaration exposes AnyKeyword" \
        "an imported wasm glue declaration cannot expose AnyKeyword"
fi

mapped_tsc -p tsconfig.web.json --noEmit
ok "generated declarations and exhaustive consumers compile"

cp deploy/tests/fixtures/wasm-bindgen-symbol.d.ts.j2 \
    target/web-generated/ts/wasm-bindgen-symbol.d.ts
mapped_tsc -p tsconfig.web.json --noEmit
rm -f target/web-generated/ts/wasm-bindgen-symbol.d.ts
ok "current wasm-bindgen disposal declarations compile under ES2022"

echo "== an omitted variant fails exhaustiveness =="
cp deploy/tests/fixtures/missing-variant.ts.j2 target/web-generated/ts/missing-variant.ts
"$PY" - deploy/tests/fixtures/missing-variant.ts.j2 \
    target/web-generated/ts/missing-variant.ts.map.json <<'PY'
import json
import pathlib
import sys

template, output = map(pathlib.Path, sys.argv[1:])
line_count = len(template.read_text(encoding="utf-8").splitlines())
line_map = {
    "version": 1,
    "rendered_path": "ts/missing-variant.ts",
    "template_path": "deploy/tests/fixtures/missing-variant.ts.j2",
    "lines": list(range(1, line_count + 1)),
}
output.write_text(json.dumps(line_map, indent=2) + "\n", encoding="utf-8")
PY
if mapped_tsc -p tsconfig.web.json --noEmit > "$TMP/compile-fail.log" 2>&1; then
    bad "the missing-variant consumer compiled"
else
    code="$(grep -oE 'TS[0-9]+' "$TMP/compile-fail.log" | LC_ALL=C sort -u | tr '\n' ' ' | sed 's/ $//')"
    is "$code" "TS2322" "the omitted variant fails with exactly TS2322"
    contains "$(cat "$TMP/compile-fail.log")" \
        "deploy/tests/fixtures/missing-variant.ts.j2(15," \
        "the diagnostic names the template path and line"
    contains "$(cat "$TMP/compile-fail.log")" \
        "[rendered target/web-generated/ts/missing-variant.ts(15," \
        "the mapped diagnostic retains the rendered coordinate"
    printf 'compile-fail error code: %s\n' "$code"
fi
rm -f target/web-generated/ts/missing-variant.ts \
    target/web-generated/ts/missing-variant.ts.map.json
mapped_tsc -p tsconfig.web.json --noEmit
ok "the clean generated tree compiles after the failure fixture"

echo "== emitted browser modules and explicit Node suites =="
mapped_tsc -p tsconfig.web.json
mapped_tsc -p tsconfig.node.json
node_suites=(
    target/web-generated/node-js/hosts.test.mjs
    target/web-generated/node-js/loader.test.mjs
    target/web-generated/node-js/check-hosts.test.mjs
)
for suite in "${node_suites[@]}"; do
    [ -f "$suite" ] || bad "the emitted Node suite is missing: $suite"
done
node --test "${node_suites[@]}"
ok "all three explicit emitted Node suites pass"

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
