#!/usr/bin/env bash
# Keep completed-work ledgers in CHANGELOG.md instead of letting them spread
# through plans, READMEs, and operating notes.
#
#   bash deploy/tests/test-done-lists.sh
#   bash deploy/tests/test-done-lists.sh --self-test
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
PY="${PYTHON:-python3}"

case "${1:-}" in
    "") ;;
    --self-test) ;;
    *)
        echo "usage: $0 [--self-test]" >&2
        exit 2
        ;;
esac

"$PY" - "$REPO" "${1:-}" <<'PY'
import re
import subprocess
import sys


repo, mode = sys.argv[1:]

date = r"(?:\s*(?:[-—]\s*)?(?:\(\s*)?\d{4}-\d{2}-\d{2}(?:\s*\))?)?"
state = r"(?:done|completed|shipped)"
emphasis = r"(?:\*\*|__)?"
checked = re.compile(r"^\s*(?:[-+*]|\d+[.)])\s+\[[xX]\](?:\s|$)")
heading = re.compile(
    rf"^\s*#{{1,6}}\s+{emphasis}{state}{date}{emphasis}\s*$",
    re.IGNORECASE,
)
bullet = re.compile(
    rf"^\s*(?:[-+*]|\d+[.)])\s+{emphasis}{state}{date}{emphasis}\s*$",
    re.IGNORECASE,
)


def forbidden(line):
    if checked.match(line):
        return "checked task"
    if heading.match(line):
        return "finished-work heading"
    if bullet.match(line):
        return "finished-work bullet"
    return None


# Every entry is (tracked path, line number, exact line): reason. Exact quotes
# make an exception fail closed when surrounding documentation changes.
EXCEPTIONS = {
    # No exceptions are needed in the current tree. Add one only when a line
    # matches the syntax above but is demonstrably not a finished-work list.
}


def self_test():
    cases = [
        ("- [x] released the map", "checked task"),
        ("  * [X] published the bundle", "checked task"),
        ("- [ ] still open", None),
        ("## Done", "finished-work heading"),
        ("### **Completed (2026-09-04)**", "finished-work heading"),
        ("#### Shipped — 2026-09-04", "finished-work heading"),
        ("- Done", "finished-work bullet"),
        ("1. __Shipped 2026-09-04__", "finished-work bullet"),
        ("| Shipped | release evidence |", None),
        ("## Work completed this week", None),
        ("- Shipped builds stay immutable.", None),
        ("This feature is done.", None),
    ]
    failures = []
    for line, expected in cases:
        actual = forbidden(line)
        if actual != expected:
            failures.append(f"{line!r}: got {actual!r}, want {expected!r}")
    if failures:
        for failure in failures:
            print(f"SELF-TEST FAIL: {failure}", file=sys.stderr)
        return 1
    print(f"self-test: {len(cases)} cases passed")
    return 0


if mode == "--self-test":
    raise SystemExit(self_test())

tracked = subprocess.run(
    ["git", "ls-files", "-z", "--", "*.md"],
    cwd=repo,
    check=True,
    stdout=subprocess.PIPE,
).stdout.split(b"\0")

findings = []
used_exceptions = set()
for raw_path in tracked:
    if not raw_path:
        continue
    path = raw_path.decode("utf-8", errors="surrogateescape")
    if path == "CHANGELOG.md":
        continue
    with open(f"{repo}/{path}", encoding="utf-8") as source:
        for number, raw_line in enumerate(source, 1):
            line = raw_line.rstrip("\n")
            kind = forbidden(line)
            if kind is None:
                continue
            key = (path, number, line)
            if key in EXCEPTIONS:
                used_exceptions.add(key)
                continue
            findings.append((path, number, line, kind))

stale = set(EXCEPTIONS) - used_exceptions
for path, number, line in sorted(stale):
    print(
        f"STALE EXCEPTION {path}:{number}: {line!r}: {EXCEPTIONS[(path, number, line)]}",
        file=sys.stderr,
    )
for path, number, line, kind in findings:
    print(f"{path}:{number}: {kind}: {line!r}", file=sys.stderr)

if stale or findings:
    print(
        f"done-list scan failed: {len(findings)} finding(s), "
        f"{len(stale)} stale exception(s)",
        file=sys.stderr,
    )
    raise SystemExit(1)

print(f"done-list scan: {len(tracked) - 1} tracked Markdown path(s), no findings")
PY
