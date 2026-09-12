#!/usr/bin/env bash
# Validate the tracked GitHub repository-ruleset request bodies.
#
#   bash deploy/tests/test-rulesets.sh
#
# The suite uses only the shell and Python's standard JSON support so the policy
# remains checkable without GitHub access or an additional package.
set -u

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
# shellcheck source=deploy/tests/lib.sh
. "$HERE/lib.sh"

cd "$ROOT"
mapfile -t RULESETS < <(find deploy/rulesets -maxdepth 1 -type f -name '*.json' -print | sort)

echo "== ruleset JSON =="
if [ "${#RULESETS[@]}" -eq 0 ]; then
    bad "no tracked ruleset payloads were found"
else
    for ruleset in "${RULESETS[@]}"; do
        if python3 - "$ruleset" <<'PY'
import json
import pathlib
import sys

value = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
if not isinstance(value, dict):
    raise SystemExit("ruleset root is not an object")
PY
        then
            ok "$ruleset parses as one JSON object"
        else
            bad "$ruleset is not a valid JSON object"
        fi
    done
fi

echo "== ruleset policy =="
if python3 - <<'PY'
import fnmatch
import json
import pathlib
import re

root = pathlib.Path(".")
ruleset_root = root / "deploy" / "rulesets"
expected_names = {
    "branch-names.json",
    "develop.json",
    "gh-pages-frozen.json",
    "main-authorization.json",
    "main-integrity.json",
    "other-tags.json",
    "release-tags-authorization.json",
    "release-tags-integrity.json",
}
paths = sorted(ruleset_root.glob("*.json"))
if {path.name for path in paths} != expected_names:
    raise SystemExit("ruleset file set differs from the eight documented payloads")

documents = {path.name: json.loads(path.read_text(encoding="utf-8")) for path in paths}
for name, document in documents.items():
    if document.get("enforcement") != "active":
        raise SystemExit(f"{name}: enforcement is not active")
    if document.get("target") not in {"branch", "tag"}:
        raise SystemExit(f"{name}: target is not branch or tag")
    if not isinstance(document.get("bypass_actors"), list):
        raise SystemExit(f"{name}: bypass_actors is not an array")
    if not isinstance(document.get("rules"), list):
        raise SystemExit(f"{name}: rules is not an array")

ci_lines = (root / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8").splitlines()
job_names = set()
inside_jobs = False
inside_job = False
for line in ci_lines:
    if line == "jobs:":
        inside_jobs = True
        inside_job = False
        continue
    if inside_jobs and line and not line.startswith(" "):
        inside_jobs = False
        inside_job = False
    if inside_jobs and re.fullmatch(r"  [A-Za-z0-9_-]+:", line):
        inside_job = True
        continue
    match = re.fullmatch(r"    name:\s*(.+)", line) if inside_job else None
    if match:
        value = match.group(1).strip()
        if len(value) >= 2 and value[0] == value[-1] and value[0] in {"'", '"'}:
            value = value[1:-1]
        job_names.add(value)
if not job_names:
    raise SystemExit("ci.yml contains no parsed job names")

develop = documents["develop.json"]
if develop["bypass_actors"]:
    raise SystemExit("develop.json has bypass actors")
if develop["conditions"]["ref_name"] != {"include": ["refs/heads/develop"], "exclude": []}:
    raise SystemExit("develop.json does not target only develop")
develop_rules = {rule["type"]: rule for rule in develop["rules"]}
if set(develop_rules) != {"deletion", "non_fast_forward", "pull_request", "required_status_checks"}:
    raise SystemExit("develop.json has the wrong rule set")
pull_request = develop_rules["pull_request"]["parameters"]
expected_pull_request = {
    "allowed_merge_methods": ["merge"],
    "dismiss_stale_reviews_on_push": True,
    "require_code_owner_review": False,
    "require_last_push_approval": False,
    "required_approving_review_count": 0,
    "required_review_thread_resolution": False,
}
if pull_request != expected_pull_request:
    raise SystemExit("develop.json has the wrong pull-request parameters")
status = develop_rules["required_status_checks"]["parameters"]
if status.get("strict_required_status_checks_policy") is not True or status.get("do_not_enforce_on_create") is not False:
    raise SystemExit("develop.json status checks are not strict on every update")
contexts = {item.get("context") for item in status.get("required_status_checks", [])}
if contexts != job_names:
    raise SystemExit(f"develop contexts {sorted(contexts)} differ from ci job names {sorted(job_names)}")
if any(item.get("integration_id") != 15368 for item in status["required_status_checks"]):
    raise SystemExit("develop status checks are not bound to GitHub Actions integration 15368")

main_integrity = documents["main-integrity.json"]
if main_integrity["bypass_actors"]:
    raise SystemExit("main integrity has bypass actors")
if main_integrity["conditions"]["ref_name"] != {"include": ["refs/heads/main"], "exclude": []}:
    raise SystemExit("main integrity does not target only main")
if {rule["type"] for rule in main_integrity["rules"]} != {"deletion", "non_fast_forward"}:
    raise SystemExit("main integrity does not contain deletion and non-fast-forward only")

main_authorization = documents["main-authorization.json"]
if main_authorization["bypass_actors"] != [{"actor_type": "DeployKey", "bypass_mode": "always"}]:
    raise SystemExit("main authorization does not have exactly one DeployKey bypass category")
if {rule["type"] for rule in main_authorization["rules"]} != {"creation", "update"}:
    raise SystemExit("main authorization does not contain creation and update only")

release_authorization = documents["release-tags-authorization.json"]
expected_signers = [
    {"actor_id": 322515484, "actor_type": "User", "bypass_mode": "always"},
    {"actor_id": 196965598, "actor_type": "User", "bypass_mode": "always"},
]
if release_authorization["bypass_actors"] != expected_signers:
    raise SystemExit("release-tag authorization does not name exactly the two release actors")
if [rule["type"] for rule in release_authorization["rules"]] != ["creation"]:
    raise SystemExit("release-tag authorization does not contain creation only")

docs = (root / "docs" / "branching.md").read_text(encoding="utf-8")
documented_branches = set()
inside_branch_table = False
for line in docs.splitlines():
    if line == "| Branch | Meaning | Writers | Update path |":
        inside_branch_table = True
        continue
    if not inside_branch_table or line == "|---|---|---|---|":
        continue
    match = re.fullmatch(r"\| `([^`]+)` \|.*", line)
    if match:
        documented_branches.add("refs/heads/" + match.group(1))
        continue
    break
expected_branches = {
    "refs/heads/develop",
    "refs/heads/main",
    "refs/heads/feature/**",
    "refs/heads/lane/**",
    # Host address-book mirrors: runtime state written by host schedulers,
    # never integrated or released, so they get a namespace of their own
    # rather than continuous machine writes under the feature prefix.
    "refs/heads/hosts/**",
}
if documented_branches != expected_branches:
    raise SystemExit(f"documented branch names differ from policy: {sorted(documented_branches)}")

branch_names = documents["branch-names.json"]
if branch_names.get("name") != "branch names" or branch_names.get("target") != "branch":
    raise SystemExit("branch names has the wrong name or target")
if branch_names["bypass_actors"]:
    raise SystemExit("branch names has bypass actors")
if [rule["type"] for rule in branch_names["rules"]] != ["creation"]:
    raise SystemExit("branch names does not contain creation only")
branch_name_condition = branch_names["conditions"]["ref_name"]
if branch_name_condition.get("include") != ["~ALL"]:
    raise SystemExit("branch names does not include all branch refs")
if len(branch_name_condition.get("exclude", [])) != len(expected_branches) or set(branch_name_condition["exclude"]) != documented_branches:
    raise SystemExit("branch-name exclusions differ from the documented branch table")

gh_pages_frozen = documents["gh-pages-frozen.json"]
if gh_pages_frozen.get("name") != "gh-pages frozen" or gh_pages_frozen.get("target") != "branch":
    raise SystemExit("gh-pages frozen has the wrong name or target")
if gh_pages_frozen["bypass_actors"]:
    raise SystemExit("gh-pages frozen has bypass actors")
if gh_pages_frozen["conditions"]["ref_name"] != {"include": ["refs/heads/gh-pages"], "exclude": []}:
    raise SystemExit("gh-pages frozen does not target only gh-pages")
gh_pages_rules = gh_pages_frozen["rules"]
if [rule["type"] for rule in gh_pages_rules] != ["update", "deletion"]:
    raise SystemExit("gh-pages frozen does not contain update and deletion only")
if gh_pages_rules[0].get("parameters") != {"update_allows_fetch_and_merge": False}:
    raise SystemExit("gh-pages frozen update permits fetch and merge")

dangerous = {"deletion", "non_fast_forward", "required_signatures"}
for name, document in documents.items():
    types = {rule["type"] for rule in document["rules"]}
    update_on_tags = document["target"] == "tag" and "update" in types
    if (types & dangerous or update_on_tags) and document["bypass_actors"]:
        raise SystemExit(f"{name}: an integrity rule has a bypass actor")

release_pattern = "refs/tags/*-[0-9]*.[0-9]*.[0-9]*"
authorization_condition = release_authorization["conditions"]["ref_name"]
integrity_condition = documents["release-tags-integrity.json"]["conditions"]["ref_name"]
if authorization_condition != {"include": [release_pattern], "exclude": []}:
    raise SystemExit("release-tag authorization has the wrong include pattern")
if integrity_condition != authorization_condition:
    raise SystemExit("release-tag rulesets do not use the same include pattern")
if documents["other-tags.json"]["conditions"]["ref_name"] != {"include": ["~ALL"], "exclude": [release_pattern]}:
    raise SystemExit("other-tags.json does not exclude the release pattern from all tags")

release_text = (root / ".github" / "workflows" / "release.yml").read_text(encoding="utf-8")
pattern_match = re.search(r"^\s*pattern='([^']+)'\s*$", release_text, re.MULTILINE)
if not pattern_match:
    raise SystemExit("release.yml runtime tag-validation regex was not found")
runtime_pattern = re.compile(pattern_match.group(1))
accepted = ["ember-1.0.0", "what-is-this-1.2.0"]
rejected = ["v1.0.0", "1.0.0"]
for tag in accepted:
    if not fnmatch.fnmatchcase("refs/tags/" + tag, release_pattern) or runtime_pattern.fullmatch(tag) is None:
        raise SystemExit(f"accepted fixture did not match both policy layers: {tag}")
for tag in rejected:
    if fnmatch.fnmatchcase("refs/tags/" + tag, release_pattern) or runtime_pattern.fullmatch(tag) is not None:
        raise SystemExit(f"rejected fixture matched a policy layer: {tag}")
if not fnmatch.fnmatchcase("refs/tags/ember-01.0.0", release_pattern) or runtime_pattern.fullmatch("ember-01.0.0") is not None:
    raise SystemExit("the ruleset fixture does not demonstrate the documented coarse fnmatch boundary")

if release_pattern not in docs or "test-rulesets.sh" not in docs:
    raise SystemExit("branching.md does not name the ruleset pattern and contract suite")
PY
then
    ok "payloads implement the documented branch, actor, integrity and tag-pattern policy"
else
    bad "payloads do not implement the documented ruleset policy"
fi

summary rulesets
