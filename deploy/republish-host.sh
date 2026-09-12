#!/usr/bin/env bash
# Fetch one host.sh-managed machine's current entry and publish it to an
# explicit address-book mirror from a workstation that has that mirror's key.
#
#   bash deploy/republish-host.sh <ssh alias> --repo <url> --branch <branch>
#                                  [--per-host]
#
# The host only needs to serve ~/ember-host/run/host.json over ssh. All Git
# reads, commits and pushes happen here. Repeating the command with an
# unchanged entry is a no-op, including its `updated` timestamp.
#
# `--per-host` writes the mirror as `<host name>.json` instead of `host.json`,
# so ONE branch can carry every host's mirror instead of needing one branch per
# host. That matters because branch names are governed by a repository ruleset:
# a new host otherwise needs an owner-run ruleset change before its address can
# be published at all. The file name is the host's own name, taken from the
# entry it served, so no caller can name the file.
#
# One name is refused: a host called `server` would write `server.json`, and
# publish-host.sh decides mirror mode by that exact basename, so this script
# would hand a host's scheduler the whole address book to overwrite with a
# single entry. The name is generated from a hash and 576 combinations do not
# include it, but "cannot happen" is not a check, and the cost of being wrong
# here is every other host's address.
set -euo pipefail

SELF_DIR="$(cd "$(dirname "$0")" && pwd)"
REMOTE="${1:-}"
[ -n "$REMOTE" ] || { sed -n '2,9p' "$0" >&2; exit 2; }
[[ "$REMOTE" =~ ^[a-zA-Z0-9][a-zA-Z0-9._-]*$ ]] \
    || { echo "republish-host: '$REMOTE' is not an ssh alias" >&2; exit 2; }
shift

REPO=""
BRANCH=""
PER_HOST=""
while [ $# -gt 0 ]; do
    case "$1" in
        --repo) REPO="${2:-}"; shift 2 ;;
        --branch) BRANCH="${2:-}"; shift 2 ;;
        --per-host) PER_HOST=1; shift ;;
        *) echo "republish-host: unknown argument '$1'" >&2; exit 2 ;;
    esac
done
[ -n "$REPO" ] || { echo "republish-host: an explicit --repo is required" >&2; exit 2; }
[ -n "$BRANCH" ] || { echo "republish-host: an explicit --branch is required" >&2; exit 2; }

# Windows puts an App Execution Alias stub named python3 on PATH: it prints
# "Python was not found" and runs nothing, so the first name `command -v`
# finds may be unusable and the next one has to be tried.
PY=""
for cand in python3 python; do
    c="$(command -v "$cand" 2>/dev/null)" || continue
    "$c" -c "pass" >/dev/null 2>&1 || continue
    PY="$c"
    break
done
if [ -z "$PY" ]; then
    echo "republish-host: need a working python3 (or python) on PATH" >&2
    exit 1
fi

WORK="$(mktemp -d -t ember-republish-XXXXXX)"
cleanup() { rm -rf "$WORK"; }
trap cleanup EXIT
ENTRY="$WORK/host.json"
MIRROR="$WORK/published-host.json"

echo "== fetching host.json from $REMOTE =="
ssh -o BatchMode=yes -o ConnectTimeout=10 "$REMOTE" \
    'cat "$HOME/ember-host/run/host.json"' > "$ENTRY"

# Validate the entry and flatten it without eval. The first four lines are
# fixed fields; the remainder is one game/url/protocol triple per line.
mapfile -t FIELDS < <("$PY" - "$ENTRY" <<'PY'
import json, re, sys

with open(sys.argv[1], encoding="utf-8") as fh:
    entry = json.load(fh)
if not isinstance(entry, dict):
    raise SystemExit("republish-host: host.json is not an object")
name = entry.get("name", "")
if not re.match(r"^[a-z0-9-]{3,32}$", name):
    raise SystemExit("republish-host: host.json has an invalid name")
games = []
for key, url in entry.items():
    if key == "ws":
        game = "arena"
    elif key.endswith("_ws"):
        game = key[:-3]
    else:
        continue
    proto_key = "proto" if game == "arena" else game + "_proto"
    proto = entry.get(proto_key)
    if not isinstance(url, str) or not re.match(r"^wss?://\S+$", url):
        raise SystemExit("republish-host: invalid address for " + game)
    if not isinstance(proto, int):
        raise SystemExit("republish-host: missing numeric protocol for " + game)
    games.append((game, url, str(proto)))
if not games:
    raise SystemExit("republish-host: host.json advertises no games")
print(name)
print(entry.get("version", ""))
print(entry.get("commit", ""))
print(entry.get("by", ""))
for game, url, proto in sorted(games):
    print("\t".join((game, url, proto)))
PY
)
[ "${#FIELDS[@]}" -ge 5 ] || { echo "republish-host: could not read a complete host entry" >&2; exit 1; }

NAME="${FIELDS[0]}"
VERSION="${FIELDS[1]}"
COMMIT="${FIELDS[2]}"
BY="${FIELDS[3]}"
# Derived from the validated name, never from an argument — and `server` is
# refused, because publish-host.sh reads that exact basename as "this file is
# the whole book" and would replace every host's entry with this one.
FILE="host.json"
if [ -n "$PER_HOST" ]; then
    if [ "$NAME" = server ]; then
        echo "republish-host: host '$NAME' cannot use --per-host: server.json is the address book itself" >&2
        exit 2
    fi
    FILE="$NAME.json"
fi

# A read-only comparison prevents `publish-host.sh` from refreshing `updated`
# and creating a commit when the host's advertised state has not changed.
git init -q "$WORK/book"
git -C "$WORK/book" remote add origin "$REPO"
if git -C "$WORK/book" fetch -q --depth 1 origin "$BRANCH" 2>/dev/null \
        && git -C "$WORK/book" show "FETCH_HEAD:$FILE" > "$MIRROR" 2>/dev/null \
        && "$PY" - "$ENTRY" "$MIRROR" <<'PY'
import json, sys

with open(sys.argv[1], encoding="utf-8") as fh:
    source = json.load(fh)
with open(sys.argv[2], encoding="utf-8") as fh:
    current = json.load(fh)
if not isinstance(current, dict) or current.get("name") != source.get("name"):
    raise SystemExit(1)
keys = [key for key in source if key != "updated"]
raise SystemExit(0 if all(current.get(key) == source.get(key) for key in keys) else 1)
PY
then
    echo "== $NAME unchanged; nothing to push =="
    exit 0
fi

ARGS=()
for field in "${FIELDS[@]:4}"; do
    IFS=$'\t' read -r game url proto <<< "$field"
    ARGS+=(--game "$game" --url "$url" --proto "$proto")
done
[ -n "$VERSION" ] && ARGS+=(--version "$VERSION")
[ -n "$COMMIT" ] && ARGS+=(--commit "$COMMIT")
[ -n "$BY" ] && ARGS+=(--by "$BY")

echo "== merging $NAME into $REPO ($BRANCH) as $FILE =="
bash "$SELF_DIR/publish-host.sh" --repo "$REPO" --branch "$BRANCH" \
    --file "$FILE" --name "$NAME" "${ARGS[@]}"
