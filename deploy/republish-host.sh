#!/usr/bin/env bash
# Fetch one host.sh-managed machine's current entry and merge it into the
# upstream address book from a workstation that already has push rights.
#
#   bash deploy/republish-host.sh <ssh alias> [--repo <url> --branch <branch>]
#
# The host only needs to serve ~/ember-host/run/host.json over ssh. All Git
# reads, commits and pushes happen here. Repeating the command with an
# unchanged entry is a no-op, including its `updated` timestamp.
set -euo pipefail

SELF_DIR="$(cd "$(dirname "$0")" && pwd)"
REMOTE="${1:-}"
[ -n "$REMOTE" ] || { sed -n '2,9p' "$0" >&2; exit 2; }
[[ "$REMOTE" =~ ^[a-zA-Z0-9][a-zA-Z0-9._-]*$ ]] \
    || { echo "republish-host: '$REMOTE' is not an ssh alias" >&2; exit 2; }
shift

REPO="git@github.com:EndersGamesDev/EmberEngine.git"
BRANCH="gh-pages"
while [ $# -gt 0 ]; do
    case "$1" in
        --repo) REPO="${2:-}"; shift 2 ;;
        --branch) BRANCH="${2:-}"; shift 2 ;;
        *) echo "republish-host: unknown argument '$1'" >&2; exit 2 ;;
    esac
done
[ -n "$REPO" ] || { echo "republish-host: --repo needs a value" >&2; exit 2; }
[ -n "$BRANCH" ] || { echo "republish-host: --branch needs a value" >&2; exit 2; }

PY="$(command -v python3 || command -v python || true)"
[ -n "$PY" ] && "$PY" -c '' >/dev/null 2>&1 \
    || { echo "republish-host: need a working python3 (or python) on PATH" >&2; exit 1; }

WORK="$(mktemp -d -t ember-republish-XXXXXX)"
cleanup() { rm -rf "$WORK"; }
trap cleanup EXIT
ENTRY="$WORK/host.json"
BOOK="$WORK/server.json"

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

# A read-only comparison prevents `publish-host.sh` from refreshing `updated`
# and creating a commit when the host's advertised state has not changed.
git init -q "$WORK/book"
git -C "$WORK/book" remote add origin "$REPO"
if git -C "$WORK/book" fetch -q --depth 1 origin "$BRANCH" 2>/dev/null \
        && git -C "$WORK/book" show FETCH_HEAD:server.json > "$BOOK" 2>/dev/null \
        && "$PY" - "$ENTRY" "$BOOK" <<'PY'
import json, sys

with open(sys.argv[1], encoding="utf-8") as fh:
    source = json.load(fh)
with open(sys.argv[2], encoding="utf-8") as fh:
    book = json.load(fh)
current = next((h for h in book.get("hosts", [])
                if isinstance(h, dict) and h.get("name") == source.get("name")), None)
if current is None:
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

echo "== merging $NAME into $REPO ($BRANCH) =="
bash "$SELF_DIR/publish-host.sh" --repo "$REPO" --branch "$BRANCH" \
    --file server.json --name "$NAME" "${ARGS[@]}"
