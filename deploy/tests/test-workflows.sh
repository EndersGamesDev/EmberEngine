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

workspace_native_packages_match_contract() {
    local file="$1"
    if [ -n "$HAVE_PYYAML" ]; then
        python3 - "$file" <<'PY'
import pathlib
import shlex
import sys
import yaml

document = yaml.safe_load(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
steps = document.get("jobs", {}).get("workspace", {}).get("steps")
if not isinstance(steps, list):
    raise SystemExit(1)

native = [(index, step) for index, step in enumerate(steps) if step.get("name") == "native build packages"]
toolchain = [(index, step) for index, step in enumerate(steps) if step.get("name") == "toolchain (pinned by rust-toolchain.toml)"]
if len(native) != 1 or len(toolchain) != 1 or native[0][0] >= toolchain[0][0]:
    raise SystemExit(1)

run = native[0][1].get("run")
if not isinstance(run, str):
    raise SystemExit(1)
commands = [shlex.split(line) for line in run.splitlines() if line.strip()]
if ["sudo", "apt-get", "update"] not in commands:
    raise SystemExit(1)
prefix = ["sudo", "apt-get", "install", "-y", "--no-install-recommends"]
installs = [command for command in commands if command[:len(prefix)] == prefix]
required = {"pkg-config", "libudev-dev", "libasound2-dev", "mesa-vulkan-drivers"}
if len(installs) != 1 or not required.issubset(installs[0][len(prefix):]):
    raise SystemExit(1)
PY
    else
        awk '
            function trim(value) {
                sub(/^[[:space:]]+/, "", value)
                sub(/[[:space:]]+$/, "", value)
                return value
            }
            /^[[:space:]]*($|#)/ { next }
            {
                match($0, /^ */)
                indent = RLENGTH
                text = substr($0, indent + 1)
                if (indent == 0 && text == "jobs:") {
                    inside_jobs = 1
                    next
                }
                if (inside_jobs && indent == 0) {
                    inside_jobs = 0
                    inside_workspace = 0
                }
                if (inside_jobs && indent == 2 && text ~ /:$/) {
                    job = text
                    sub(/:$/, "", job)
                    inside_workspace = job == "workspace"
                    next
                }
                if (!inside_workspace) next
                if (indent == 6 && text ~ /^- /) {
                    step++
                    step_name = ""
                    sub(/^- /, "", text)
                    if (text ~ /^name:/) {
                        sub(/^name:[[:space:]]*/, "", text)
                        step_name = trim(text)
                    }
                    if (step_name == "native build packages") {
                        native_count++
                        native_step = step
                    }
                    if (step_name == "toolchain (pinned by rust-toolchain.toml)") {
                        toolchain_count++
                        toolchain_step = step
                    }
                }
                if (step_name == "native build packages") {
                    native_text = native_text " " trim(text)
                }
            }
            END {
                padded = " " native_text " "
                if (native_count != 1 || toolchain_count != 1 || native_step >= toolchain_step) exit 1
                if (index(padded, " sudo apt-get update ") == 0) exit 1
                if (index(padded, " sudo apt-get install -y --no-install-recommends ") == 0) exit 1
                if (index(padded, " pkg-config ") == 0) exit 1
                if (index(padded, " libudev-dev ") == 0) exit 1
                if (index(padded, " libasound2-dev ") == 0) exit 1
                if (index(padded, " mesa-vulkan-drivers ") == 0) exit 1
            }
        ' "$file"
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

release_tag_object_matches_contract() {
    local file="$1"
    if [ -n "$HAVE_PYYAML" ]; then
        python3 - "$file" <<'PY'
import pathlib
import sys
import yaml

class UniqueKeyLoader(yaml.SafeLoader):
    pass


def construct_unique_mapping(loader, node, deep=False):
    loader.flatten_mapping(node)
    mapping = {}
    for key_node, value_node in node.value:
        key = loader.construct_object(key_node, deep=deep)
        if key in mapping:
            raise yaml.constructor.ConstructorError(
                "while constructing a mapping",
                node.start_mark,
                "found duplicate key %r" % key,
                key_node.start_mark,
            )
        mapping[key] = loader.construct_object(value_node, deep=deep)
    return mapping


UniqueKeyLoader.add_constructor(
    yaml.resolver.BaseResolver.DEFAULT_MAPPING_TAG,
    construct_unique_mapping,
)
try:
    document = yaml.load(
        pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"),
        Loader=UniqueKeyLoader,
    )
except yaml.YAMLError:
    raise SystemExit(1)
steps = document.get("jobs", {}).get("validate", {}).get("steps")
if not isinstance(steps, list):
    raise SystemExit(1)

checkouts = [(index, step) for index, step in enumerate(steps) if step.get("uses") == "actions/checkout@v4"]
fetches = [(index, step) for index, step in enumerate(steps) if step.get("name") == "fetch the tag object"]
annotated = [(index, step) for index, step in enumerate(steps) if step.get("name") == "require an annotated tag"]
if len(checkouts) != 1 or len(fetches) != 1 or len(annotated) != 1:
    raise SystemExit(1)
if fetches[0][0] != checkouts[0][0] + 1 or fetches[0][0] >= annotated[0][0]:
    raise SystemExit(1)

step = fetches[0][1]
if step.get("env") != {"TAG": "${{ github.ref_name }}"}:
    raise SystemExit(1)
run = step.get("run")
if not isinstance(run, str):
    raise SystemExit(1)
commands = [line.strip() for line in run.splitlines() if line.strip()]
required = {
    "set -euo pipefail",
    'git fetch --no-tags --force origin "+refs/tags/${TAG}:refs/tags/${TAG}"',
    'test "$(git cat-file -t "refs/tags/$TAG")" = tag',
}
if not required.issubset(commands):
    raise SystemExit(1)
PY
    else
        awk '
            function record_step_field(value) {
                if (value == "uses: actions/checkout@v4") checkout[step] = 1
                if (value == "name: fetch the tag object") fetch[step] = 1
                if (value == "name: require an annotated tag") annotated[step] = 1
                if (value == "env:") {
                    section = "env"
                } else if (value ~ /^run:/) {
                    section = "run"
                } else {
                    section = ""
                }
            }
            /^  validate:$/ {
                inside = 1
                next
            }
            inside && /^  [^ ]/ {
                inside = 0
            }
            !inside {
                next
            }
            /^      - / {
                step++
                record_step_field(substr($0, 9))
                next
            }
            /^        [^ ]/ {
                record_step_field(substr($0, 9))
                next
            }
            section == "env" && /^          [^[:space:]#][^:]*:/ {
                env_count[step]++
                if ($0 == "          TAG: ${{ github.ref_name }}") tag_env[step] = 1
                next
            }
            section == "run" {
                if ($0 == "          set -euo pipefail") strict[step] = 1
                if ($0 == "          git fetch --no-tags --force origin \"+refs/tags/${TAG}:refs/tags/${TAG}\"") refspec[step] = 1
                if ($0 == "          test \"$(git cat-file -t \"refs/tags/$TAG\")\" = tag") object_type[step] = 1
            }
            END {
                for (candidate = 1; candidate <= step; candidate++) {
                    if (checkout[candidate]) {
                        checkout_count++
                        checkout_step = candidate
                    }
                    if (fetch[candidate]) {
                        fetch_count++
                        fetch_step = candidate
                    }
                    if (annotated[candidate]) {
                        annotated_count++
                        annotated_step = candidate
                    }
                }
                if (checkout_count != 1 || fetch_count != 1 || annotated_count != 1) exit 1
                if (fetch_step != checkout_step + 1 || fetch_step >= annotated_step) exit 1
                if (env_count[fetch_step] != 1 || !tag_env[fetch_step]) exit 1
                if (!strict[fetch_step] || !refspec[fetch_step] || !object_type[fetch_step]) exit 1
            }
        ' "$file"
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
if "refs/tags/*-[0-9]*.[0-9]*.[0-9]*" not in runs:
    raise SystemExit(1)
# The host gate deploys nothing by itself, so two things about it are the
# contract and neither is its presence: WHERE it runs and whether it can stop
# the job. It has to run after the asset is on disk, because it reads that
# tree's own games.json, server.json and hosts.js, and before anything
# publishes, because a site whose pages have no host to join must not reach a
# player. And it has to be able to fail the job: `continue-on-error` or an `if`
# turns the same step, in the same place, into a log line — which is the exact
# shape a green build with a broken site would have.
steps = job.get("steps", [])


def step_index(predicate):
    for index, step in enumerate(steps):
        if predicate(step):
            return index
    return -1


extract = step_index(lambda s: s.get("name") == "extract the release asset")
gate = step_index(lambda s: "check-hosts.mjs" in str(s.get("run", "")))
configure = step_index(lambda s: str(s.get("uses", "")).startswith("actions/configure-pages@"))
upload = step_index(lambda s: str(s.get("uses", "")).startswith("actions/upload-pages-artifact@"))
deploy = step_index(lambda s: str(s.get("uses", "")).startswith("actions/deploy-pages@"))
if min(extract, gate, configure, upload, deploy) < 0:
    raise SystemExit(1)
if not extract < gate < configure < upload < deploy:
    raise SystemExit(1)
if steps[gate].get("continue-on-error") is not None or steps[gate].get("if") is not None:
    raise SystemExit(1)
# It must read the EXTRACTED tree rather than this checkout's own web/
# directory: the asset is what ships, and the two differ by every commit made
# since the tag.
gate_run = str(steps[gate].get("run", ""))
if "--tree" not in gate_run or "pages-site" not in gate_run:
    raise SystemExit(1)
# node 22 or newer, pinned before the gate: the probe opens a real WebSocket
# from the global, and a runtime without one would fail every game for a
# reason that has nothing to do with the hosts.
setup = step_index(lambda s: str(s.get("uses", "")).startswith("actions/setup-node@"))
if setup < 0 or setup > gate:
    raise SystemExit(1)
if int(str(steps[setup].get("with", {}).get("node-version", "0")).split(".")[0]) < 22:
    raise SystemExit(1)
PY
    else
        grep -Fq 'workflows: [release]' "$file" \
            && grep -Fq 'types: [completed]' "$file" \
            && grep -Fq 'ref: main' "$file" \
            && grep -Fq 'publishedAt' "$file" \
            && grep -Fq 'refs/tags/*-[0-9]*.[0-9]*.[0-9]*' "$file" \
            && awk '
                # A step is a block: `continue-on-error` sits beside `run`, not
                # inside it, and may come either side of it, so the flag is
                # collected per step and judged when the step ends.
                function flush() { if (isgate && soft_here) soft = 1 }
                /^      - / { flush(); isgate = 0; soft_here = 0 }
                /^      - name: extract the release asset$/ { extract = NR }
                /uses: actions\/setup-node@/ { setup = NR }
                /check-hosts\.mjs/ { gate = NR; isgate = 1 }
                /^        (continue-on-error|if):/ { soft_here = 1 }
                /uses: actions\/configure-pages@/ { configure = NR }
                /uses: actions\/deploy-pages@/ { deploy = NR }
                END {
                    flush()
                    exit !(extract && setup && gate && configure && deploy \
                        && !soft \
                        && extract < gate && setup < gate \
                        && gate < configure && configure < deploy)
                }
            ' "$file"
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

write_missing_native_package_fixture() {
    local source="$1" file="$2"
    sed 's/ mesa-vulkan-drivers//' "$source" > "$file"
}

write_missing_release_tag_object_fixture() {
    local source="$1" file="$2"
    awk '
        $0 == "      - name: fetch the tag object" {
            skipping = 1
            next
        }
        skipping && /^      - / {
            skipping = 0
        }
        !skipping {
            print
        }
    ' "$source" > "$file"
}

write_extra_release_tag_env_fixture() {
    local source="$1" file="$2"
    awk '
        $0 == "      - name: fetch the tag object" {
            fetch = 1
        }
        fetch && $0 == "          TAG: ${{ github.ref_name }}" {
            print
            print "          EXTRA: harmless"
            fetch = 0
            next
        }
        {
            print
        }
    ' "$source" > "$file"
}

write_duplicate_release_tag_env_fixture() {
    local source="$1" file="$2"
    awk '
        $0 == "      - name: fetch the tag object" {
            fetch = 1
        }
        fetch && $0 == "          TAG: ${{ github.ref_name }}" {
            print "          TAG: wrong"
            print
            fetch = 0
            next
        }
        {
            print
        }
    ' "$source" > "$file"
}

write_named_release_checkout_fixture() {
    local source="$1" file="$2"
    awk '
        !replaced && $0 == "      - uses: actions/checkout@v4" {
            print "      - name: check out the release source"
            print "        uses: actions/checkout@v4"
            replaced = 1
            next
        }
        {
            print
        }
    ' "$source" > "$file"
}

write_missing_host_gate_fixture() {
    local source="$1" file="$2"
    awk '
        $0 == "      - name: prove every live game has a host to join" {
            skipping = 1
            next
        }
        skipping && /^      - / {
            skipping = 0
        }
        !skipping {
            print
        }
    ' "$source" > "$file"
}

# The gate moved to the END of the job: every step still present, every name
# still spelled the same, and the site already deployed by the time it runs.
# This is the fixture that separates "the step exists" from "the step gates".
#
# The step is captured to the next step boundary rather than by a line count,
# because a writer that assumed a fixed length would silently produce a
# fixture missing half the step — and a malformed fixture that fails the
# contract for the wrong reason is a test that proves nothing.
write_late_host_gate_fixture() {
    local source="$1" file="$2"
    awk '
        $0 == "      - name: prove every live game has a host to join" {
            holding = 1
            held = $0
            next
        }
        holding && /^      - / { holding = 0 }
        holding { held = held "\n" $0; next }
        { print }
        END {
            if (held == "") exit 1
            print held
        }
    ' "$source" > "$file" || return 1
}

# The gate in its right place, doing nothing: `continue-on-error` makes a
# failing check a log line, and the deploy proceeds over it.
write_soft_host_gate_fixture() {
    local source="$1" file="$2"
    awk '
        $0 == "      - name: prove every live game has a host to join" {
            print
            print "        continue-on-error: true"
            next
        }
        { print }
    ' "$source" > "$file"
}

TEST_WORK="$(mktemp -d -t ember-workflow-test-XXXXXX)" || {
    echo "test-workflows: unable to create fixture directory" >&2
    exit 1
}
trap 'rm -r -- "$TEST_WORK"' EXIT
QUOTED_PROMOTE="$TEST_WORK/quoted-promote.yml"
PERMISSIVE_REGEX="$TEST_WORK/permissive-regex.yml"
MISSING_NATIVE_PACKAGE="$TEST_WORK/missing-native-package.yml"
MISSING_RELEASE_TAG_OBJECT="$TEST_WORK/missing-release-tag-object.yml"
EXTRA_RELEASE_TAG_ENV="$TEST_WORK/extra-release-tag-env.yml"
DUPLICATE_RELEASE_TAG_ENV="$TEST_WORK/duplicate-release-tag-env.yml"
NAMED_RELEASE_CHECKOUT="$TEST_WORK/named-release-checkout.yml"
MISSING_HOST_GATE="$TEST_WORK/missing-host-gate.yml"
LATE_HOST_GATE="$TEST_WORK/late-host-gate.yml"
SOFT_HOST_GATE="$TEST_WORK/soft-host-gate.yml"
write_quoted_promote_fixture "$QUOTED_PROMOTE"
write_permissive_regex_fixture "$PERMISSIVE_REGEX"
write_missing_native_package_fixture .github/workflows/ci.yml "$MISSING_NATIVE_PACKAGE"
write_missing_release_tag_object_fixture .github/workflows/release.yml "$MISSING_RELEASE_TAG_OBJECT"
write_extra_release_tag_env_fixture .github/workflows/release.yml "$EXTRA_RELEASE_TAG_ENV"
write_duplicate_release_tag_env_fixture .github/workflows/release.yml "$DUPLICATE_RELEASE_TAG_ENV"
write_named_release_checkout_fixture .github/workflows/release.yml "$NAMED_RELEASE_CHECKOUT"
write_missing_host_gate_fixture .github/workflows/pages.yml "$MISSING_HOST_GATE"
write_late_host_gate_fixture .github/workflows/pages.yml "$LATE_HOST_GATE" \
    || bad "the late host-gate fixture writer captured no step"
write_soft_host_gate_fixture .github/workflows/pages.yml "$SOFT_HOST_GATE"

FIXTURES_READY=1
for fixture in \
    "$QUOTED_PROMOTE" \
    "$PERMISSIVE_REGEX" \
    "$MISSING_NATIVE_PACKAGE" \
    "$MISSING_RELEASE_TAG_OBJECT" \
    "$EXTRA_RELEASE_TAG_ENV" \
    "$DUPLICATE_RELEASE_TAG_ENV" \
    "$NAMED_RELEASE_CHECKOUT" \
    "$MISSING_HOST_GATE" \
    "$LATE_HOST_GATE" \
    "$SOFT_HOST_GATE"
do
    if [ -s "$fixture" ]; then
        ok "$(basename "$fixture") fixture exists and is non-empty"
    else
        bad "$(basename "$fixture") fixture does not exist or is empty"
        FIXTURES_READY=""
    fi
done
if [ -z "$FIXTURES_READY" ]; then
    summary workflows
    exit 1
fi

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
if workspace_native_packages_match_contract .github/workflows/ci.yml; then
    ok "ci.yml installs the workspace native packages before the toolchain"
else
    bad "ci.yml does not install the workspace native packages before the toolchain"
fi
if workspace_native_packages_match_contract "$MISSING_NATIVE_PACKAGE"; then
    bad "the workspace native-package check accepted a fixture with one package removed"
else
    ok "the workspace native-package check rejects a fixture with one package removed"
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
if release_tag_object_matches_contract .github/workflows/release.yml; then
    ok "release.yml fetches the annotated tag object immediately after checkout"
else
    bad "release.yml does not fetch the annotated tag object immediately after checkout"
fi
if release_tag_object_matches_contract "$MISSING_RELEASE_TAG_OBJECT"; then
    bad "the release tag-object check accepted a fixture without the fetch step"
else
    ok "the release tag-object check rejects a fixture without the fetch step"
fi
if release_tag_object_matches_contract "$EXTRA_RELEASE_TAG_ENV"; then
    bad "the release tag-object check accepted a fixture with an extra fetch environment entry"
else
    ok "the release tag-object check rejects a fixture with an extra fetch environment entry"
fi
if release_tag_object_matches_contract "$DUPLICATE_RELEASE_TAG_ENV"; then
    bad "the release tag-object check accepted a fixture with duplicate fetch environment keys"
else
    ok "the release tag-object check rejects a fixture with duplicate fetch environment keys"
fi
if release_tag_object_matches_contract "$NAMED_RELEASE_CHECKOUT"; then
    ok "the release tag-object check accepts checkout with name before uses"
else
    bad "the release tag-object check rejects checkout with name before uses"
fi
if pages_matches_contract .github/workflows/pages.yml; then
    ok "pages.yml waits for release success and selects a published asset at main"
else
    bad "pages.yml does not enforce its release-success and main-asset contract"
fi
if pages_matches_contract "$MISSING_HOST_GATE"; then
    bad "the pages check accepted a fixture with no host gate at all"
else
    ok "the pages check rejects a fixture with no host gate"
fi
if pages_matches_contract "$LATE_HOST_GATE"; then
    bad "the pages check accepted a host gate that runs after the deploy"
else
    ok "the pages check rejects a host gate that runs after the deploy"
fi
if pages_matches_contract "$SOFT_HOST_GATE"; then
    bad "the pages check accepted a host gate that cannot fail the job"
else
    ok "the pages check rejects a host gate marked continue-on-error"
fi
if grep -Fq "\`.github/workflows/pages.yml\` runs after the \`release\` workflow completes successfully" docs/branching.md; then
    ok "branching.md states the release-completion Pages trigger"
else
    bad "branching.md does not state the release-completion Pages trigger"
fi
if grep -Fq "\`node deploy/check-hosts.mjs --tree\` against the extracted asset and fails the deploy" docs/branching.md; then
    ok "branching.md states the host gate between extraction and deployment"
else
    bad "branching.md does not state the host gate between extraction and deployment"
fi

summary workflows
