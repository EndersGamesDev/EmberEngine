#!/usr/bin/env bash
# Validate the checked-in workflow contract without contacting GitHub.
#
#   bash deploy/tests/test-workflows.sh
#
# PyYAML performs the full parse and structural policy checks when available.
# A strict shell fallback checks encoding, indentation, document shape and
# normalized quoted or unquoted keys; EMBER_TEST_NO_PYYAML=1 exercises it.
set -u

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
# shellcheck source=deploy/tests/lib.sh
. "$HERE/lib.sh"

cd "$ROOT"
mapfile -t WORKFLOWS < <(git ls-files '.github/workflows/*.yml' '.github/workflows/*.yaml')
HAVE_PYYAML=""
if [ -z "${EMBER_TEST_NO_PYYAML:-}" ] && python3 -c 'import yaml' >/dev/null 2>&1; then
    HAVE_PYYAML=1
fi

strict_shell_yaml() {
    local file="$1"
    if LC_ALL=C grep -q $'\r\|\t' "$file"; then
        return 1
    fi
    if [ "$(LC_ALL=C head -c 3 "$file" | od -An -tx1 | tr -d ' \n')" = efbbbf ]; then
        return 1
    fi
    awk '
        function trim(value) {
            sub(/^[[:space:]]+/, "", value)
            sub(/[[:space:]]+$/, "", value)
            return value
        }
        function key_of(value, first, last, quote) {
            sub(/:.*/, "", value)
            value = trim(value)
            first = substr(value, 1, 1)
            last = substr(value, length(value), 1)
            quote = sprintf("%c", 39)
            if ((first == "\"" && last == "\"") || (first == quote && last == quote)) {
                value = substr(value, 2, length(value) - 2)
            }
            return value
        }
        /^[[:space:]]*($|#)/ { next }
        {
            match($0, /^ */)
            indent = RLENGTH
            if (indent % 2 != 0) bad = 1
            if (indent == 0 && index($0, ":") > 0) {
                key = key_of($0)
                if (++seen[key] > 1) bad = 1
            }
        }
        END {
            if (seen["name"] != 1 || seen["on"] != 1 || seen["jobs"] != 1) bad = 1
            exit bad
        }
    ' "$file"
}

fallback_job_names() {
    local file="$1"
    awk '
        function trim(value) {
            sub(/^[[:space:]]+/, "", value)
            sub(/[[:space:]]+$/, "", value)
            return value
        }
        function key_of(value, first, last, quote) {
            sub(/:.*/, "", value)
            value = trim(value)
            first = substr(value, 1, 1)
            last = substr(value, length(value), 1)
            quote = sprintf("%c", 39)
            if ((first == "\"" && last == "\"") || (first == quote && last == quote)) {
                value = substr(value, 2, length(value) - 2)
            }
            return value
        }
        /^[[:space:]]*($|#)/ { next }
        {
            match($0, /^ */)
            indent = RLENGTH
            if (indent == 0 && index($0, ":") > 0) {
                key = key_of($0)
                inside = key == "jobs"
                next
            }
            if (inside && indent == 2 && index($0, ":") > 0) {
                print key_of($0)
            }
        }
    ' "$file"
}

workflow_has_no_job() {
    local file="$1" job="$2"
    if [ -n "$HAVE_PYYAML" ]; then
        python3 - "$file" "$job" <<'PY'
import pathlib
import sys
import yaml

document = yaml.safe_load(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
jobs = document.get("jobs") if isinstance(document, dict) else None
if not isinstance(jobs, dict):
    raise SystemExit(2)
raise SystemExit(1 if sys.argv[2] in jobs else 0)
PY
    elif fallback_job_names "$file" | grep -Fxq "$job"; then
        return 1
    else
        return 0
    fi
}

workflow_tag_pattern() {
    local file="$1"
    if [ -n "$HAVE_PYYAML" ]; then
        python3 - "$file" <<'PY'
import pathlib
import sys
import yaml

document = yaml.safe_load(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
trigger = document.get("on", document.get(True))
tags = trigger.get("push", {}).get("tags") if isinstance(trigger, dict) else None
if not isinstance(tags, list) or len(tags) != 1 or not isinstance(tags[0], str):
    raise SystemExit(1)
print(tags[0])
PY
    else
        local -a patterns
        mapfile -t patterns < <(sed -nE "s/^[[:space:]]+- ['\"]([^'\"]+)['\"][[:space:]]*$/\1/p" "$file")
        [ "${#patterns[@]}" -eq 1 ] || return 1
        printf '%s\n' "${patterns[0]}"
    fi
}

runtime_tag_pattern() {
    local file="$1"
    if [ -n "$HAVE_PYYAML" ]; then
        python3 - "$file" <<'PY'
import pathlib
import re
import sys
import yaml

document = yaml.safe_load(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
patterns = []
for job in document.get("jobs", {}).values():
    for step in job.get("steps", []):
        if step.get("id") != "tag" or not isinstance(step.get("run"), str):
            continue
        patterns.extend(re.findall(r"(?m)^\s*pattern='([^']+)'\s*$", step["run"]))
if len(patterns) != 1:
    raise SystemExit(1)
print(patterns[0])
PY
    else
        local -a patterns
        mapfile -t patterns < <(sed -nE "s/^[[:space:]]*pattern='([^']+)'[[:space:]]*$/\1/p" "$file")
        [ "${#patterns[@]}" -eq 1 ] || return 1
        printf '%s\n' "${patterns[0]}"
    fi
}

runtime_regex_matches_contract() {
    local file="$1" pattern tag
    pattern="$(runtime_tag_pattern "$file")" || return 1
    for tag in ember-1.0.0 what-is-this-1.2.0; do
        [[ "$tag" =~ $pattern ]] || return 1
    done
    for tag in v1.0.0 ember-1.0.1 ember-01.0.0 ember-1.0.0-rc1; do
        [[ "$tag" =~ $pattern ]] && return 1
    done
    return 0
}

ci_trigger_matches_contract() {
    local file="$1"
    if [ -n "$HAVE_PYYAML" ]; then
        python3 - "$file" <<'PY'
import pathlib
import sys
import yaml

document = yaml.safe_load(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
trigger = document.get("on", document.get(True))
if not isinstance(trigger, dict):
    raise SystemExit(1)
push = trigger.get("push")
pull = trigger.get("pull_request")
if not isinstance(push, dict) or push.get("branches") != ["develop"] or "paths-ignore" in push:
    raise SystemExit(1)
if not isinstance(pull, dict) or pull.get("branches") != ["develop"] or "paths-ignore" in pull:
    raise SystemExit(1)
PY
    else
        [ "$(grep -Fxc '    branches: [develop]' "$file")" -eq 2 ] \
            && ! grep -Fq 'paths-ignore:' "$file"
    fi
}

release_order_matches_contract() {
    local file="$1"
    if [ -n "$HAVE_PYYAML" ]; then
        python3 - "$file" <<'PY'
import pathlib
import sys
import yaml

document = yaml.safe_load(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
jobs = document.get("jobs", {})
concurrency = document.get("concurrency")
if concurrency != {"group": "release", "cancel-in-progress": False, "queue": "max"}:
    raise SystemExit(1)
expected = {
    "draft": {"validate", "build"},
    "promote": {"validate", "draft"},
    "discard-failed-draft": {"validate", "draft", "promote"},
    "publish": {"validate", "promote"},
}
for name, needs in expected.items():
    job = jobs.get(name)
    if not isinstance(job, dict):
        raise SystemExit(1)
    actual = job.get("needs", [])
    if isinstance(actual, str):
        actual = [actual]
    if set(actual) != needs:
        raise SystemExit(1)

def runs(name):
    return "\n".join(step.get("run", "") for step in jobs[name].get("steps", []) if isinstance(step.get("run"), str))

if "--draft" not in runs("draft") or "ember-pages.tar.gz" not in runs("draft"):
    raise SystemExit(1)
if "refs/heads/main" not in runs("promote"):
    raise SystemExit(1)
if "release delete" not in runs("discard-failed-draft"):
    raise SystemExit(1)
if "deploy/github-releases.sh --apply --tag" not in runs("publish") or "--draft" in runs("publish"):
    raise SystemExit(1)
PY
    else
        local jobs
        jobs="$(fallback_job_names "$file")"
        grep -Fxq draft <<< "$jobs" \
            && grep -Fxq promote <<< "$jobs" \
            && grep -Fxq discard-failed-draft <<< "$jobs" \
            && grep -Fxq publish <<< "$jobs" \
            && grep -Fq 'needs: [validate, draft]' "$file" \
            && grep -Fq 'needs: [validate, promote]' "$file" \
            && grep -Fq '  queue: max' "$file" \
            && grep -Fq 'run: bash deploy/github-releases.sh --apply --tag "$TAG"' "$file"
    fi
}

pages_matches_contract() {
    local file="$1"
    if [ -n "$HAVE_PYYAML" ]; then
        python3 - "$file" <<'PY'
import pathlib
import sys
import yaml

document = yaml.safe_load(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
trigger = document.get("on", document.get(True))
workflow_run = trigger.get("workflow_run") if isinstance(trigger, dict) else None
if not isinstance(workflow_run, dict) or workflow_run.get("workflows") != ["release"] or workflow_run.get("types") != ["completed"]:
    raise SystemExit(1)
job = document.get("jobs", {}).get("deploy")
if not isinstance(job, dict) or "workflow_run.conclusion == 'success'" not in str(job.get("if", "")):
    raise SystemExit(1)
checkout_refs = [step.get("with", {}).get("ref") for step in job.get("steps", []) if str(step.get("uses", "")).startswith("actions/checkout@")]
if checkout_refs != ["main"]:
    raise SystemExit(1)
runs = "\n".join(step.get("run", "") for step in job.get("steps", []) if isinstance(step.get("run"), str))
if "isDraft" not in runs or "publishedAt" not in runs or "ember-pages.tar.gz" not in runs:
    raise SystemExit(1)
PY
    else
        grep -Fq 'workflows: [release]' "$file" \
            && grep -Fq 'types: [completed]' "$file" \
            && grep -Fq 'ref: main' "$file" \
            && grep -Fq 'publishedAt' "$file"
    fi
}

write_quoted_promote_fixture() {
    local file="$1"
    while IFS= read -r line; do printf '%s\n' "$line"; done > "$file" <<'YAML'
name: quoted job fixture
'on':
  workflow_dispatch:
jobs:
  'promote':
    runs-on: ubuntu-latest
    steps:
      - run: 'true'
YAML
}

write_permissive_regex_fixture() {
    local file="$1"
    while IFS= read -r line; do printf '%s\n' "$line"; done > "$file" <<'YAML'
name: permissive regex fixture
'on':
  workflow_dispatch:
jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - id: tag
        run: |
          pattern='^(.+)-([0-9]+\.[0-9]+\.[0-9]+)$'
          [[ "$TAG" =~ $pattern ]]
YAML
}

TEST_WORK="$(mktemp -d "${TMPDIR:?}/ember-workflow-test.XXXXXX")"
trap 'rm -r -- "$TEST_WORK"' EXIT
QUOTED_PROMOTE="$TEST_WORK/quoted-promote.yml"
PERMISSIVE_REGEX="$TEST_WORK/permissive-regex.yml"
write_quoted_promote_fixture "$QUOTED_PROMOTE"
write_permissive_regex_fixture "$PERMISSIVE_REGEX"

echo "== workflow YAML =="
if [ "${#WORKFLOWS[@]}" -eq 0 ]; then
    bad "no tracked workflow files were found"
elif [ -n "$HAVE_PYYAML" ]; then
    for workflow in "${WORKFLOWS[@]}"; do
        if python3 - "$workflow" <<'PY'
import pathlib
import sys
import yaml

document = yaml.safe_load(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
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
if workflow_has_no_job .github/workflows/ci.yml promote; then
    ok "ci.yml has no promote job"
else
    bad "ci.yml defines a promote job"
fi
if workflow_has_no_job "$QUOTED_PROMOTE" promote; then
    bad "the structural job check accepted a quoted promote job"
else
    ok "the structural job check rejects a quoted promote job"
fi

if grep -En 'ci-passed|heartbeat' "${WORKFLOWS[@]}" > /dev/null; then
    bad "a workflow references a retired branch or heartbeat"
else
    ok "workflows reference neither retired integration state nor heartbeat"
fi

if ci_trigger_matches_contract .github/workflows/ci.yml; then
    ok "ci.yml gates pull requests and every push to develop"
else
    bad "ci.yml does not provide exact-SHA develop gates"
fi

TAG_PATTERN='*-[0-9]*.[0-9]*.[0-9]*'
if actual_pattern="$(workflow_tag_pattern .github/workflows/release.yml)" && [ "$actual_pattern" = "$TAG_PATTERN" ]; then
    ok "release.yml uses the documented $TAG_PATTERN tag trigger"
else
    bad "release.yml does not use the documented $TAG_PATTERN tag trigger"
fi
if grep -Fq -- "workflow trigger omits the \`refs/tags/\` prefix and uses \`$TAG_PATTERN\`" docs/branching.md; then
    ok "branching.md states the same $TAG_PATTERN tag trigger"
else
    bad "branching.md does not state the $TAG_PATTERN tag trigger"
fi
if runtime_regex_matches_contract .github/workflows/release.yml; then
    ok "release.yml runtime regex accepts and rejects the policy fixtures"
else
    bad "release.yml runtime regex does not enforce the release-tag grammar"
fi
if runtime_regex_matches_contract "$PERMISSIVE_REGEX"; then
    bad "the runtime regex check accepted a permissive fixture"
else
    ok "the runtime regex check rejects a permissive fixture"
fi

if release_order_matches_contract .github/workflows/release.yml; then
    ok "release.yml queues tag runs and publishes only after promotion"
else
    bad "release.yml does not preserve its queue, draft, promotion, cleanup and publication contract"
fi
if pages_matches_contract .github/workflows/pages.yml; then
    ok "pages.yml waits for release success and selects a published asset at main"
else
    bad "pages.yml does not enforce its release-success and main-asset contract"
fi
if grep -Fq "\`.github/workflows/pages.yml\` runs after the \`release\` workflow completes successfully" docs/branching.md; then
    ok "branching.md states the release-completion Pages trigger"
else
    bad "branching.md does not state the release-completion Pages trigger"
fi

summary workflows
