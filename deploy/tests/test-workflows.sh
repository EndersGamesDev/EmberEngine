#!/usr/bin/env bash
# Validate the checked-in workflow contract without contacting GitHub.
#
#   bash deploy/tests/test-workflows.sh
#
# PyYAML performs the full parse when available. A strict shell fallback checks
# encoding, indentation, document shape and unique required top-level keys so a
# minimal host still rejects malformed workflow scaffolding; setting
# EMBER_TEST_NO_PYYAML=1 exercises that path on a host that has PyYAML.
set -u

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
# shellcheck source=deploy/tests/lib.sh
. "$HERE/lib.sh"

cd "$ROOT"
mapfile -t WORKFLOWS < <(git ls-files '.github/workflows/*.yml' '.github/workflows/*.yaml')

strict_shell_yaml() {
    local file="$1"
    if LC_ALL=C grep -q $'\r\|\t' "$file"; then
        return 1
    fi
    if [ "$(LC_ALL=C head -c 3 "$file" | od -An -tx1 | tr -d ' \n')" = efbbbf ]; then
        return 1
    fi
    awk '
        /^[[:space:]]*($|#)/ { next }
        {
            match($0, /^ */)
            if (RLENGTH % 2 != 0) bad = 1
        }
        /^[A-Za-z_][A-Za-z0-9_-]*:/ {
            key = $1
            sub(/:$/, "", key)
            if (++seen[key] > 1) bad = 1
        }
        END {
            if (seen["name"] != 1 || seen["on"] != 1 || seen["jobs"] != 1) bad = 1
            exit bad
        }
    ' "$file"
}

echo "== workflow YAML =="
if [ "${#WORKFLOWS[@]}" -eq 0 ]; then
    bad "no tracked workflow files were found"
elif [ -z "${EMBER_TEST_NO_PYYAML:-}" ] && python3 -c 'import yaml' >/dev/null 2>&1; then
    for workflow in "${WORKFLOWS[@]}"; do
        if python3 - "$workflow" <<'PY'
import pathlib
import sys
import yaml

path = pathlib.Path(sys.argv[1])
document = yaml.safe_load(path.read_text(encoding="utf-8"))
if not isinstance(document, dict):
    raise SystemExit("workflow root is not a mapping")
PY
        then
            ok "$workflow parses with PyYAML"
        else
            bad "$workflow does not parse with PyYAML"
        fi
    done
else
    for workflow in "${WORKFLOWS[@]}"; do
        if strict_shell_yaml "$workflow"; then
            ok "$workflow passes the strict shell YAML fallback"
        else
            bad "$workflow fails the strict shell YAML fallback"
        fi
    done
fi

echo "== workflow policy =="
if grep -Eq '^  promote:' .github/workflows/ci.yml; then
    bad "ci.yml still defines a promote job"
else
    ok "ci.yml has no promote job"
fi

if grep -En 'ci-passed|heartbeat' "${WORKFLOWS[@]}" > /dev/null; then
    bad "a workflow references a retired branch or heartbeat"
else
    ok "workflows reference neither retired integration state nor heartbeat"
fi

TAG_PATTERN='*-[0-9]*.[0-9]*.[0-9]*'
if grep -Fq -- "- '$TAG_PATTERN'" .github/workflows/release.yml; then
    ok "release.yml uses the documented $TAG_PATTERN tag trigger"
else
    bad "release.yml does not use the documented $TAG_PATTERN tag trigger"
fi
if grep -Fq -- "workflow trigger omits the \`refs/tags/\` prefix and uses \`$TAG_PATTERN\`" docs/branching.md; then
    ok "branching.md states the same $TAG_PATTERN tag trigger"
else
    bad "branching.md does not state the $TAG_PATTERN tag trigger"
fi

PAGES_BRANCH=main
if grep -Fq "branches: [$PAGES_BRANCH]" .github/workflows/pages.yml; then
    ok "pages.yml uses the documented $PAGES_BRANCH branch"
else
    bad "pages.yml does not use the documented $PAGES_BRANCH branch"
fi
if grep -Fq "\`.github/workflows/pages.yml\` runs only when \`$PAGES_BRANCH\` advances" docs/branching.md; then
    ok "branching.md states the same $PAGES_BRANCH Pages branch"
else
    bad "branching.md does not state the $PAGES_BRANCH Pages branch"
fi

summary workflows
