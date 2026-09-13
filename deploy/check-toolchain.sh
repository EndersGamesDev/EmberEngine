#!/usr/bin/env bash
# Print and verify the pinned Node, npm and TypeScript toolchain.
#
#   bash deploy/check-toolchain.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# Windows may resolve an App Execution Alias named python3 that cannot run.
PY=""
for candidate in python3 python; do
    interpreter="$(command -v "$candidate" 2>/dev/null)" || continue
    "$interpreter" -c "pass" >/dev/null 2>&1 || continue
    PY="$interpreter"
    break
done
[ -n "$PY" ] || { echo "toolchain: need a working python3 or python on PATH" >&2; exit 1; }

readarray -t expected < <("$PY" - package.json package-lock.json <<'PY'
import json
import pathlib
import sys

package = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
lock = json.loads(pathlib.Path(sys.argv[2]).read_text(encoding="utf-8"))
print(package["engines"]["node"])
print(package["packageManager"].removeprefix("npm@"))
print(lock["packages"]["node_modules/typescript"]["version"])
PY
)

active_node="$(node --version)"
active_node="${active_node#v}"
active_npm="$(npm --version)"
active_tsc="$(npx --no-install tsc --version)"
active_tsc="${active_tsc#Version }"

printf 'node %s\n' "$active_node"
printf 'npm %s\n' "$active_npm"
printf 'tsc %s\n' "$active_tsc"

[ "$active_node" = "${expected[0]}" ] || { printf 'toolchain: node must be %s\n' "${expected[0]}" >&2; exit 1; }
[ "$active_npm" = "${expected[1]}" ] || { printf 'toolchain: npm must be %s\n' "${expected[1]}" >&2; exit 1; }
[ "$active_tsc" = "${expected[2]}" ] || { printf 'toolchain: tsc must be %s\n' "${expected[2]}" >&2; exit 1; }
