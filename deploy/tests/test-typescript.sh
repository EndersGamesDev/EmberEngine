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
    web/games/fire/v2/race.js
    web/games/fire/v2/garage.js
    web/games/end-game/v12/main.js
    web/games/end-game/v12/dialogue.js
    web/games/end-game/v12/castle-audio.js
    web/games/end-game/v12/castle-ui.js
    web/games/end-game/v12/quality.js
    web/games/end-game/v12/voice-lines.js
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
if ! wasm_types_check="$(bash deploy/stage-wasm-types.sh --check 2>&1)"; then
    skip_execution "$wasm_types_check"
fi

bash deploy/check-toolchain.sh

TMP="$(mktemp -d -t ember-typescript-test-XXXXXX)"
SOURCE_ID="typescript-test"
SCOPED="target/web-generated/$SOURCE_ID"
PICK_IMPORT_ASSIGNMENT="target/web-generated/node-ts/pick-import-assignment"
trap 'rm -rf "$TMP" "$PICK_IMPORT_ASSIGNMENT"' EXIT

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

function renderedLocation(source, position) {
  const point = source.getLineAndCharacterOfPosition(position);
  return `${source.fileName}:${point.line + 1}:${point.character + 1}`;
}

function failure(source, position, message) {
  const point = source.getLineAndCharacterOfPosition(position);
  const renderedLine = point.line + 1;
  const column = point.character + 1;
  const mapPath = `${source.fileName}.map.json`;
  if (fs.existsSync(mapPath)) {
    const lineMap = JSON.parse(fs.readFileSync(mapPath, 'utf8'));
    const templateLine = lineMap.lines[point.line];
    if (Number.isInteger(templateLine)) {
      return `${lineMap.template_path}(${templateLine},${column}): ${message} `
        + `[rendered ${lineMap.rendered_path}(${renderedLine},${column})]`;
    }
  }
  return `${renderedLocation(source, position)}: ${message}`;
}

function reject(source, node, message) {
  failures.push(failure(source, node.getStart(source), message));
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

function scanWasmDeclaration(declaration, importedNames, origin) {
  const selection = importedNames === null ? '*' : [...importedNames].sort().join(',');
  const checkedKey = `${declaration}\0${selection}`;
  if (checkedDeclarations.has(checkedKey) || !fs.existsSync(declaration)) return;
  checkedDeclarations.add(checkedKey);
  const text = fs.readFileSync(declaration, 'utf8');
  const source = ts.createSourceFile(declaration, text, ts.ScriptTarget.ES2022, true);
  const declarations = new Map();
  const bindings = new Map();
  const starExports = [];
  function add(map, name, value) {
    const entries = map.get(name) || [];
    entries.push(value);
    map.set(name, entries);
  }
  function moduleText(statement) {
    return statement.moduleSpecifier && ts.isStringLiteralLike(statement.moduleSpecifier)
      ? statement.moduleSpecifier.text
      : null;
  }
  for (const statement of source.statements) {
    if (ts.isVariableStatement(statement)) {
      for (const declarationNode of statement.declarationList.declarations) {
        if (ts.isIdentifier(declarationNode.name)) add(declarations, declarationNode.name.text, declarationNode);
      }
    } else if (statement.name && ts.isIdentifier(statement.name)) {
      add(declarations, statement.name.text, statement);
    }
    if (ts.isImportDeclaration(statement) && statement.importClause) {
      const specifier = moduleText(statement);
      if (!specifier) continue;
      const clause = statement.importClause;
      if (clause.name) add(bindings, clause.name.text, { specifier, imported: null });
      if (clause.namedBindings && ts.isNamespaceImport(clause.namedBindings)) {
        add(bindings, clause.namedBindings.name.text, { specifier, imported: null });
      } else if (clause.namedBindings) {
        for (const element of clause.namedBindings.elements) {
          add(bindings, element.name.text, {
            specifier,
            imported: (element.propertyName || element.name).text,
          });
        }
      }
    }
    if (ts.isExportDeclaration(statement)) {
      const specifier = moduleText(statement);
      if (!statement.exportClause) {
        if (specifier) starExports.push(specifier);
      } else if (ts.isNamespaceExport(statement.exportClause)) {
        add(bindings, statement.exportClause.name.text, { specifier, imported: null });
      } else {
        for (const element of statement.exportClause.elements) {
          const target = (element.propertyName || element.name).text;
          add(bindings, element.name.text, specifier
            ? { specifier, imported: target }
            : { local: target });
        }
      }
    }
  }
  const visited = new Set();
  const visitedNames = new Set();
  function scanExternal(specifier, names) {
    let found = false;
    for (const candidate of declarationCandidates(declaration, specifier)) {
      if (!fs.existsSync(candidate)) continue;
      found = true;
      scanWasmDeclaration(candidate, names, origin);
    }
    if (!found) visit(source);
  }
  function visitNamed(name) {
    if (visitedNames.has(name)) return;
    visitedNames.add(name);
    let found = false;
    for (const candidate of declarations.get(name) || []) {
      found = true;
      visit(candidate);
    }
    for (const binding of bindings.get(name) || []) {
      found = true;
      if (binding.local) visitNamed(binding.local);
      else scanExternal(binding.specifier, binding.imported === null ? null : new Set([binding.imported]));
    }
    if (!found && starExports.length) {
      found = true;
      for (const specifier of starExports) scanExternal(specifier, new Set([name]));
    }
    if (!found) visit(source);
  }
  function visitEntityName(name) {
    if (ts.isIdentifier(name)) {
      visitNamed(name.text);
    } else if (ts.isQualifiedName(name)) {
      if (ts.isIdentifier(name.left)) {
        const namespaceBindings = bindings.get(name.left.text) || [];
        if (namespaceBindings.length) {
          for (const binding of namespaceBindings) {
            if (binding.local) visitNamed(binding.local);
            else scanExternal(binding.specifier, new Set([name.right.text]));
          }
        } else {
          visitNamed(name.left.text);
        }
      } else {
        visitEntityName(name.left);
      }
    }
  }
  function visitHeritageExpression(expression) {
    if (ts.isIdentifier(expression)) {
      visitNamed(expression.text);
    } else if (ts.isPropertyAccessExpression(expression)
      && ts.isIdentifier(expression.expression)) {
      const namespaceBindings = bindings.get(expression.expression.text) || [];
      if (namespaceBindings.length) {
        for (const binding of namespaceBindings) {
          if (binding.local) visitNamed(binding.local);
          else scanExternal(binding.specifier, new Set([expression.name.text]));
        }
      } else {
        visit(source);
      }
    } else {
      visit(source);
    }
  }
  function visit(node) {
    if (visited.has(node)) return;
    visited.add(node);
    if (node.kind === ts.SyntaxKind.AnyKeyword) {
      reject(
        origin.source,
        origin.node,
        `wasm declaration exposes AnyKeyword at ${renderedLocation(source, node.getStart(source))}`,
      );
    }
    if (ts.isTypeReferenceNode(node)) {
      visitEntityName(node.typeName);
    }
    if (ts.isTypeQueryNode(node)) {
      visitEntityName(node.exprName);
    }
    if (ts.isExpressionWithTypeArguments(node)) {
      visitHeritageExpression(node.expression);
    }
    if (ts.isImportEqualsDeclaration(node)) {
      const reference = node.moduleReference;
      if (ts.isExternalModuleReference(reference)
        && reference.expression && ts.isStringLiteralLike(reference.expression)) {
        scanExternal(reference.expression.text, null);
      } else if (ts.isIdentifier(reference) || ts.isQualifiedName(reference)) {
        visitEntityName(reference);
      } else {
        visit(source);
      }
    }
    if (ts.isImportTypeNode(node) && ts.isLiteralTypeNode(node.argument)
      && ts.isStringLiteralLike(node.argument.literal)) {
      const names = node.qualifier && ts.isIdentifier(node.qualifier)
        ? new Set([node.qualifier.text])
        : null;
      scanExternal(node.argument.literal.text, names);
    }
    if (ts.isExportDeclaration(node) && node.moduleSpecifier
      && ts.isStringLiteralLike(node.moduleSpecifier)) {
      if (!node.exportClause || ts.isNamespaceExport(node.exportClause)) {
        scanExternal(node.moduleSpecifier.text, null);
      } else {
        for (const element of node.exportClause.elements) {
          scanExternal(
            node.moduleSpecifier.text,
            new Set([(element.propertyName || element.name).text]),
          );
        }
      }
    }
    ts.forEachChild(node, visit);
  }
  if (importedNames === null) visit(source);
  else for (const name of importedNames) visitNamed(name);
}

function namedImports(node) {
  const clause = node.importClause;
  if (!clause || clause.name || !clause.namedBindings || ts.isNamespaceImport(clause.namedBindings)) {
    return null;
  }
  return new Set(clause.namedBindings.elements.map(
    (element) => (element.propertyName || element.name).text,
  ));
}

function checkSpecifier(source, node, specifier, importedNames = null) {
  if (!specifier.startsWith('./') && !specifier.startsWith('../')) return;
  const bare = specifier.split(/[?#]/, 1)[0];
  const extension = path.posix.extname(bare);
  if (!extension) reject(source, node, 'relative import must name an emitted extension');
  if (['.ts', '.mts', '.cts'].includes(extension)) {
    reject(source, node, 'relative import names a TypeScript source extension');
  }
  for (const declaration of declarationCandidates(source.fileName, bare)) {
    if (declaration.split(path.sep).includes('wasm')) {
      scanWasmDeclaration(declaration, importedNames, { source, node });
    }
  }
}

function checkImportEqualsTarget(source, node) {
  const reference = node.moduleReference;
  if (ts.isExternalModuleReference(reference)
    && reference.expression && ts.isStringLiteralLike(reference.expression)) {
    checkSpecifier(source, reference.expression, reference.expression.text);
    return;
  }
  if (!ts.isIdentifier(reference) && !ts.isQualifiedName(reference)) return;
  let root = reference;
  let selected = null;
  while (ts.isQualifiedName(root)) {
    selected = selected || root.right.text;
    root = root.left;
  }
  if (!ts.isIdentifier(root)) return;
  for (const statement of source.statements) {
    if (ts.isImportEqualsDeclaration(statement) && statement !== node
      && statement.name.text === root.text) {
      checkImportEqualsTarget(source, statement);
    }
    if (ts.isImportDeclaration(statement) && statement.importClause
      && statement.moduleSpecifier && ts.isStringLiteralLike(statement.moduleSpecifier)) {
      const bindings = statement.importClause.namedBindings;
      if (bindings && ts.isNamespaceImport(bindings) && bindings.name.text === root.text) {
        checkSpecifier(
          source,
          statement.moduleSpecifier,
          statement.moduleSpecifier.text,
          selected === null ? null : new Set([selected]),
        );
      }
    }
  }
}

function shadowsPick(source) {
  let shadowed = false;
  function visit(node) {
    const namedDeclaration = ts.isTypeAliasDeclaration(node)
      || ts.isInterfaceDeclaration(node)
      || ts.isClassDeclaration(node)
      || ts.isFunctionDeclaration(node)
      || ts.isEnumDeclaration(node)
      || ts.isModuleDeclaration(node)
      || ts.isTypeParameterDeclaration(node)
      || ts.isVariableDeclaration(node);
    const importedBinding = ts.isImportClause(node)
      || ts.isImportSpecifier(node)
      || ts.isNamespaceImport(node)
      || ts.isImportEqualsDeclaration(node);
    if ((namedDeclaration || importedBinding)
      && node.name && ts.isIdentifier(node.name) && node.name.text === 'Pick') {
      shadowed = true;
      return;
    }
    ts.forEachChild(node, visit);
  }
  visit(source);
  return shadowed;
}

function importTypeNames(node, pickIsShadowed) {
  if (node.qualifier && ts.isIdentifier(node.qualifier)) return new Set([node.qualifier.text]);
  const parent = node.parent;
  if (!ts.isTypeReferenceNode(parent) || !ts.isIdentifier(parent.typeName)
    || parent.typeName.text !== 'Pick' || parent.typeArguments?.[0] !== node
    || pickIsShadowed) return null;
  const selection = parent.typeArguments[1];
  const names = new Set();
  function collect(candidate) {
    if (ts.isLiteralTypeNode(candidate) && ts.isStringLiteralLike(candidate.literal)) {
      names.add(candidate.literal.text);
      return true;
    }
    if (ts.isUnionTypeNode(candidate)) return candidate.types.every(collect);
    return false;
  }
  return selection && collect(selection) && names.size ? names : null;
}

function scan(file) {
  const text = fs.readFileSync(file, 'utf8');
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.ES2022, true);
  const pickIsShadowed = shadowsPick(source);
  const typeAliases = new Map();
  const interfaces = new Map();
  const callables = new Map();
  function collectDeclarations(node) {
    if (ts.isTypeAliasDeclaration(node)) typeAliases.set(node.name.text, node.type);
    if (ts.isInterfaceDeclaration(node)) interfaces.set(node.name.text, node);
    if (ts.isFunctionDeclaration(node) && node.name) callables.set(node.name.text, node);
    if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name)
      && node.initializer
      && (ts.isArrowFunction(node.initializer) || ts.isFunctionExpression(node.initializer))) {
      callables.set(node.name.text, node.initializer);
    }
    ts.forEachChild(node, collectDeclarations);
  }
  collectDeclarations(source);
  function isUnknownType(node) {
    return node?.kind === ts.SyntaxKind.UnknownKeyword;
  }
  function isPromiseUnknown(node, seen = new Set()) {
    if (!node) return false;
    if (ts.isParenthesizedTypeNode(node)) return isPromiseUnknown(node.type, seen);
    if (!ts.isTypeReferenceNode(node) || !ts.isIdentifier(node.typeName)) return false;
    if (node.typeName.text === 'Promise') {
      return node.typeArguments?.length === 1 && isUnknownType(node.typeArguments[0]);
    }
    if (seen.has(node.typeName.text)) return false;
    const alias = typeAliases.get(node.typeName.text);
    if (!alias) return false;
    seen.add(node.typeName.text);
    return isPromiseUnknown(alias, seen);
  }
  function functionTypeReturnsPromiseUnknown(node, seen = new Set()) {
    if (!node) return false;
    if (ts.isParenthesizedTypeNode(node)) {
      return functionTypeReturnsPromiseUnknown(node.type, seen);
    }
    if (ts.isFunctionTypeNode(node)) return isPromiseUnknown(node.type);
    if (!ts.isTypeReferenceNode(node) || !ts.isIdentifier(node.typeName)
      || seen.has(node.typeName.text)) return false;
    const alias = typeAliases.get(node.typeName.text);
    if (!alias) return false;
    seen.add(node.typeName.text);
    return functionTypeReturnsPromiseUnknown(alias, seen);
  }
  function contextualPropertyReturnsPromiseUnknown(fn) {
    let property = fn.parent;
    while (ts.isParenthesizedExpression(property) || ts.isConditionalExpression(property)
      || ts.isAsExpression(property)) property = property.parent;
    if (!ts.isPropertyAssignment(property)) return false;
    const propertyName = ts.isIdentifier(property.name) || ts.isStringLiteralLike(property.name)
      ? property.name.text
      : null;
    if (propertyName === null) return false;
    let owner = property.parent;
    while (owner && !ts.isFunctionLike(owner)) owner = owner.parent;
    if (!owner?.type || !ts.isTypeReferenceNode(owner.type)
      || !ts.isIdentifier(owner.type.typeName)) return false;
    const declaration = interfaces.get(owner.type.typeName.text);
    if (!declaration) return false;
    for (const member of declaration.members) {
      if (!ts.isPropertySignature(member) || !member.type || !member.name) continue;
      const memberName = ts.isIdentifier(member.name) || ts.isStringLiteralLike(member.name)
        ? member.name.text
        : null;
      if (memberName === propertyName && functionTypeReturnsPromiseUnknown(member.type)) return true;
    }
    return false;
  }
  function callableReturnsPromiseUnknown(fn) {
    if (fn.type && isPromiseUnknown(fn.type)) return true;
    const parent = fn.parent;
    if (ts.isVariableDeclaration(parent)
      && functionTypeReturnsPromiseUnknown(parent.type)) return true;
    return contextualPropertyReturnsPromiseUnknown(fn);
  }
  function directFunctionResult(node, fn) {
    let current = node;
    while (current.parent && (ts.isAwaitExpression(current.parent)
      || ts.isParenthesizedExpression(current.parent)
      || ts.isAsExpression(current.parent))) current = current.parent;
    if (fn.body === current) return true;
    return ts.isReturnStatement(current.parent) && current.parent.expression === current;
  }
  function argumentAcceptsUnknown(call, expression) {
    if (!ts.isIdentifier(call.expression)) return false;
    const callable = callables.get(call.expression.text);
    if (!callable) return false;
    const index = call.arguments.indexOf(expression);
    return index >= 0 && isUnknownType(callable.parameters[index]?.type);
  }
  function nonLiteralImportIsNarrowed(call) {
    let current = call;
    let awaited = false;
    while (current.parent) {
      const parent = current.parent;
      if (ts.isAwaitExpression(parent)) {
        awaited = true;
        current = parent;
        continue;
      }
      if (ts.isParenthesizedExpression(parent)) {
        current = parent;
        continue;
      }
      if (ts.isAsExpression(parent)) {
        if (awaited && isUnknownType(parent.type)) return true;
        current = parent;
        continue;
      }
      if (awaited && ts.isVariableDeclaration(parent) && parent.initializer === current) {
        return isUnknownType(parent.type);
      }
      if (awaited && ts.isCallExpression(parent) && argumentAcceptsUnknown(parent, current)) {
        return true;
      }
      break;
    }
    let owner = call.parent;
    while (owner && !ts.isFunctionLike(owner)) owner = owner.parent;
    return !!owner && directFunctionResult(call, owner) && callableReturnsPromiseUnknown(owner);
  }
  for (const match of text.matchAll(/@ts-(?:ignore|expect-error)/g)) {
    const position = match.index === undefined ? 0 : match.index;
    failures.push(failure(source, position, `forbidden directive ${match[0]}`));
  }
  function visit(node) {
    if (node.kind === ts.SyntaxKind.AnyKeyword) reject(source, node, 'forbidden AnyKeyword');
    if (ts.isAsExpression(node)) {
      const inner = unwrap(node.expression);
      if (ts.isAsExpression(inner) && inner.type.kind === ts.SyntaxKind.UnknownKeyword) {
        reject(source, node, 'double assertion through unknown');
      }
    }
    if (ts.isImportDeclaration(node)
      && node.moduleSpecifier && ts.isStringLiteralLike(node.moduleSpecifier)) {
      checkSpecifier(source, node.moduleSpecifier, node.moduleSpecifier.text, namedImports(node));
    }
    if (ts.isImportEqualsDeclaration(node)) checkImportEqualsTarget(source, node);
    if (ts.isExportDeclaration(node)
      && node.moduleSpecifier && ts.isStringLiteralLike(node.moduleSpecifier)) {
      checkSpecifier(source, node.moduleSpecifier, node.moduleSpecifier.text);
    }
    if (ts.isCallExpression(node) && node.expression.kind === ts.SyntaxKind.ImportKeyword) {
      if (node.arguments.length === 1 && ts.isStringLiteralLike(node.arguments[0])) {
        checkSpecifier(source, node.arguments[0], node.arguments[0].text);
      } else if (!nonLiteralImportIsNarrowed(node)) {
        reject(
          source,
          node,
          'non-literal dynamic import must narrow its result immediately to unknown',
        );
      }
    }
    if (ts.isImportTypeNode(node) && ts.isLiteralTypeNode(node.argument)
      && ts.isStringLiteralLike(node.argument.literal)) {
      checkSpecifier(source, node, node.argument.literal.text, importTypeNames(node, pickIsShadowed));
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
bash deploy/stage-wasm-types.sh "$SOURCE_ID"
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
cp deploy/tests/fixtures/typescript-wasm-any.d.ts.j2 "$TMP/ast/wasm/fake.d.ts"
cp deploy/tests/fixtures/typescript-wasm-any.d.ts.j2 "$TMP/ast/wasm/fake.d.cts"
cp deploy/tests/fixtures/typescript-wasm-imported.d.ts.j2 "$TMP/ast/wasm/imported.d.ts"
cp deploy/tests/fixtures/typescript-wasm-bridge.d.ts.j2 "$TMP/ast/wasm/bridge.d.ts"
cp deploy/tests/fixtures/typescript-wasm-payload.d.ts.j2 "$TMP/ast/wasm/payload.d.ts"
cat > "$TMP/ast/wasm/assignment-alias.d.ts" <<'TS'
import Model = require('./assignment-model.js');
import Alias = Model.Unsafe;
export function clean(): Alias;
TS
cat > "$TMP/ast/wasm/assignment-model.d.ts" <<'TS'
declare namespace Model { type Unsafe = any; }
export = Model;
TS
write_fixture_map() {
    "$PY" - "$1" "$2" <<'PY'
import json
import pathlib
import sys

template, rendered = map(pathlib.Path, sys.argv[1:])
line_count = len(template.read_text(encoding="utf-8").splitlines())
line_map = {
    "version": 1,
    "rendered_path": rendered.as_posix(),
    "template_path": template.as_posix(),
    "lines": list(range(1, line_count + 1)),
}
rendered.with_name(rendered.name + ".map.json").write_text(
    json.dumps(line_map, indent=2) + "\n",
    encoding="utf-8",
)
PY
}

cp deploy/tests/fixtures/typescript-dynamic-import-unknown-negative.ts.j2 \
    "$TMP/ast/typescript-dynamic-import-unknown-negative.ts"
write_fixture_map deploy/tests/fixtures/typescript-dynamic-import-unknown-negative.ts.j2 \
    "$TMP/ast/typescript-dynamic-import-unknown-negative.ts"
cp deploy/tests/fixtures/typescript-dynamic-import-unknown-positive.ts.j2 \
    "$TMP/ast/typescript-dynamic-import-unknown-positive.ts"
write_fixture_map deploy/tests/fixtures/typescript-dynamic-import-unknown-positive.ts.j2 \
    "$TMP/ast/typescript-dynamic-import-unknown-positive.ts"
if source_ast_gate "$TMP/ast/typescript-dynamic-import-unknown-negative.ts" \
    > "$TMP/typescript-dynamic-import-unknown-negative.log" 2>&1; then
    bad "an untyped non-literal dynamic import was accepted"
else
    rejection="$(cat "$TMP/typescript-dynamic-import-unknown-negative.log")"
    contains "$rejection" \
        "non-literal dynamic import must narrow its result immediately to unknown" \
        "an untyped non-literal dynamic import is rejected"
    printf 'dynamic-import rejection: %s\n' "$rejection"
fi
if source_ast_gate "$TMP/ast/typescript-dynamic-import-unknown-positive.ts"; then
    ok "a non-literal dynamic import in a contextually typed Promise<unknown> position passes"
else
    bad "a non-literal dynamic import in a contextually typed Promise<unknown> position was rejected"
fi

for fixture in named-clean named-transitive namespace overload variable export-alias inherited \
    type-alias import-type named-reexport nested-type nested-typeof pick-shadow pick-selected; do
    cp "deploy/tests/fixtures/typescript-wasm-$fixture.ts.j2" \
        "$TMP/ast/typescript-wasm-$fixture.ts"
    write_fixture_map "deploy/tests/fixtures/typescript-wasm-$fixture.ts.j2" \
        "$TMP/ast/typescript-wasm-$fixture.ts"
done
cp deploy/tests/fixtures/typescript-wasm-pick-import-assignment.cts.j2 \
    "$TMP/ast/typescript-wasm-pick-import-assignment.cts"
write_fixture_map deploy/tests/fixtures/typescript-wasm-pick-import-assignment.cts.j2 \
    "$TMP/ast/typescript-wasm-pick-import-assignment.cts"
cp deploy/tests/fixtures/typescript-wasm-import-assignment-source.cts.j2 \
    "$TMP/ast/typescript-wasm-import-assignment-source.cts"
write_fixture_map deploy/tests/fixtures/typescript-wasm-import-assignment-source.cts.j2 \
    "$TMP/ast/typescript-wasm-import-assignment-source.cts"
cp deploy/tests/fixtures/typescript-wasm-import-assignment-declaration.ts.j2 \
    "$TMP/ast/typescript-wasm-import-assignment-declaration.ts"
write_fixture_map deploy/tests/fixtures/typescript-wasm-import-assignment-declaration.ts.j2 \
    "$TMP/ast/typescript-wasm-import-assignment-declaration.ts"

mkdir -p "$PICK_IMPORT_ASSIGNMENT/wasm"
cp deploy/tests/fixtures/typescript-wasm-pick-import-assignment.cts.j2 \
    "$PICK_IMPORT_ASSIGNMENT/consumer.cts"
cp deploy/tests/fixtures/typescript-wasm-import-assignment-source.cts.j2 \
    "$PICK_IMPORT_ASSIGNMENT/direct.cts"
cat > "$PICK_IMPORT_ASSIGNMENT/keep.cts" <<'TS'
class Keep<T, K> { value!: T; key!: K; }
export = Keep;
TS
cat > "$PICK_IMPORT_ASSIGNMENT/wasm/fake.d.cts" <<'TS'
export function clean(): void;
export const leak: any;
TS
node_project_files="$(npx --no-install tsc -p tsconfig.node.json --noEmit --listFilesOnly)"
contains "$node_project_files" "$PICK_IMPORT_ASSIGNMENT/consumer.cts" \
    "the import-assignment Pick reproducer is inside the Node project"
contains "$node_project_files" "$PICK_IMPORT_ASSIGNMENT/direct.cts" \
    "the direct import-assignment reproducer is inside the Node project"
if mapped_tsc -p tsconfig.node.json --noEmit; then
    ok "the import-assignment reproducers type-check under the Node project"
else
    bad "the import-assignment reproducers do not type-check under the Node project"
fi
rm -rf "$PICK_IMPORT_ASSIGNMENT"
if source_ast_gate "$TMP/ast/typescript-wasm-named-clean.ts"; then
    ok "a named wasm import ignores an unreachable AnyKeyword sibling"
else
    bad "a clean named wasm import beside an AnyKeyword sibling was rejected"
fi
if source_ast_gate "$TMP/ast/typescript-wasm-named-transitive.ts" \
    > "$TMP/typescript-wasm-named-transitive.log" 2>&1; then
    bad "a named wasm import transitively exposing AnyKeyword was accepted"
else
    contains "$(cat "$TMP/typescript-wasm-named-transitive.log")" \
        "deploy/tests/fixtures/typescript-wasm-named-transitive.ts.j2(1," \
        "a transitive wasm failure names the template coordinate"
    contains "$(cat "$TMP/typescript-wasm-named-transitive.log")" \
        "[rendered $TMP/ast/typescript-wasm-named-transitive.ts(1," \
        "a transitive wasm failure retains the rendered coordinate"
    printf 'wasm named-transitive rejection: %s\n' \
        "$(cat "$TMP/typescript-wasm-named-transitive.log")"
fi
if source_ast_gate "$TMP/ast/typescript-wasm-namespace.ts" \
    > "$TMP/typescript-wasm-namespace.log" 2>&1; then
    bad "a namespace wasm import beside an AnyKeyword was accepted"
else
    contains "$(cat "$TMP/typescript-wasm-namespace.log")" \
        "wasm declaration exposes AnyKeyword" \
        "a namespace wasm import scans the whole declaration"
fi
if source_ast_gate "$TMP/ast/typescript-wasm-overload.ts" \
    > "$TMP/typescript-wasm-overload.log" 2>&1; then
    bad "an AnyKeyword in an earlier wasm overload was accepted"
else
    contains "$(cat "$TMP/typescript-wasm-overload.log")" \
        "wasm declaration exposes AnyKeyword" \
        "every overload of a named wasm import is scanned"
fi
if source_ast_gate "$TMP/ast/typescript-wasm-pick-selected.ts"; then
    ok "Fire's selected wasm import form ignores unreachable declarations"
else
    bad "Fire's selected wasm import form rejected an unreachable declaration"
fi

for fixture in variable export-alias inherited type-alias import-type named-reexport \
    nested-type nested-typeof pick-shadow pick-import-assignment \
    import-assignment-source import-assignment-declaration; do
    extension=ts
    case "$fixture" in
        pick-import-assignment|import-assignment-source) extension=cts ;;
    esac
    if source_ast_gate "$TMP/ast/typescript-wasm-$fixture.$extension" \
        > "$TMP/typescript-wasm-$fixture.log" 2>&1; then
        bad "the $fixture wasm declaration bypass was accepted"
    else
        rejection="$(cat "$TMP/typescript-wasm-$fixture.log")"
        contains "$rejection" "wasm declaration exposes AnyKeyword" \
            "the $fixture wasm declaration path is rejected"
        printf 'wasm %s rejection: %s\n' "$fixture" "$rejection"
    fi
done

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
end_game_modules=(
    target/web-generated/js/games/end-game/v12/main.js
    target/web-generated/js/games/end-game/v12/dialogue.js
    target/web-generated/js/games/end-game/v12/castle-audio.js
    target/web-generated/js/games/end-game/v12/castle-ui.js
    target/web-generated/js/games/end-game/v12/quality.js
    target/web-generated/js/games/end-game/v12/voice-lines.js
)
for module in "${end_game_modules[@]}"; do
    [ -f "$module" ] || bad "the emitted End Game module is missing: $module"
done
check_module_closure() {
node - "$@" <<'NODE'
const fs = require('node:fs');
const path = require('node:path');
const ts = require('typescript');
const expected = new Set(process.argv.slice(2).map(file => path.resolve(file)));
const pending = [path.resolve(process.argv[2])];
const visited = new Set();
while (pending.length) {
  const file = pending.pop();
  if (!file || visited.has(file)) continue;
  if (!expected.has(file)) throw new Error(`End Game import closure contains unexpected module ${file}`);
  visited.add(file);
  const source = ts.createSourceFile(file, fs.readFileSync(file, 'utf8'), ts.ScriptTarget.ES2022, true);
  function follow(specifierNode) {
    if (!specifierNode || !ts.isStringLiteralLike(specifierNode)) return;
    const specifier = specifierNode.text;
    if (!specifier.startsWith('./')) return;
    pending.push(path.resolve(path.dirname(file), specifier));
  }
  function visit(node) {
    if (ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) follow(node.moduleSpecifier);
    if (ts.isCallExpression(node) && node.expression.kind === ts.SyntaxKind.ImportKeyword) {
      follow(node.arguments[0]);
    }
    ts.forEachChild(node, visit);
  }
  visit(source);
}
const missing = [...expected].filter(file => !visited.has(file));
if (missing.length) throw new Error(`End Game import closure omits ${missing.join(', ')}`);
NODE
}
check_module_closure "${end_game_modules[@]}"
ok "emitted End Game main import closure is exactly the six shipped modules"
closure_fixture="$TMP/module-closure"
mkdir -p "$closure_fixture"
cp deploy/tests/fixtures/module-closure-main.js.j2 "$closure_fixture/main.js"
cp deploy/tests/fixtures/module-closure-reexport.js.j2 "$closure_fixture/reexport.js"
cp deploy/tests/fixtures/module-closure-dynamic.js.j2 "$closure_fixture/dynamic.js"
if check_module_closure "$closure_fixture/main.js" "$closure_fixture/reexport.js" "$closure_fixture/dynamic.js"; then
    ok "module closure follows re-exports and dynamic sibling imports"
else
    bad "module closure missed a re-export or dynamic sibling import"
fi
node_suites=(
    target/web-generated/node-js/hosts.test.mjs
    target/web-generated/node-js/loader.test.mjs
    target/web-generated/node-js/check-hosts.test.mjs
    target/web-generated/node-js/fire.test.mjs
    target/web-generated/node-js/tools/end-game/castle.test.mjs
    target/web-generated/node-js/tools/end-game/dialogue.test.mjs
    target/web-generated/node-js/tools/end-game/guard.test.mjs
    target/web-generated/node-js/tools/end-game/quality.test.mjs
    target/web-generated/node-js/tools/end-game/main-boot.test.mjs
)
for suite in "${node_suites[@]}"; do
    [ -f "$suite" ] || bad "the emitted Node suite is missing: $suite"
done
node --test "${node_suites[@]}"
ok "all nine explicit emitted Node suites pass"

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
