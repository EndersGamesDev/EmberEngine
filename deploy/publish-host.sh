#!/usr/bin/env bash
# Write one host's entry into the address book (docs/hosts.md §3).
#
#   publish-host.sh --name amber-otter \
#       --game arena --url wss://x.trycloudflare.com --proto 12 \
#       --game fire  --url wss://y.trycloudflare.com --proto 1 \
#       --version r211 --commit 502414c --by end@specht \
#       --book /tmp/pages/server.json
#
#   publish-host.sh --name amber-otter --remove --book …/server.json
#   publish-host.sh --recompute --book …/server.json
#   publish-host.sh --name amber-otter --game fire … --repo git@…:me/x.git \
#       --branch gh-pages --file host.json
#
# Flags:
#   --name <host>      the entry's merge key, [a-z0-9-]{3,32}
#   --game <id> --url <ws url> --proto <n>
#                      one game's two keys; repeat the triple per game
#   --version rN       the build the host is running THAT GAME from; with no
#                      --game in the call it is the arena's
#   --commit <sha>     its short sha, same rule
#   --by <text>        free text: who deployed it from where (optional)
#   --drop-game <id>   retire one game on this host: delete its four keys from
#                      the entry and leave the rest of it alone; repeatable
#   --remove           delete the entry instead of writing it
#   --recompute        touch nothing but the legacy address keys
#   --book <path>      rewrite this file in place (no git)
#   --repo <url|path> --branch <b>
#                      clone, rewrite, commit and push
#   --file <name>      file inside the book repo; default server.json.
#                      host.json switches to MIRROR mode: the file is the
#                      single host entry on its own, not the whole book
#
# THIS IS THE ONLY WRITER of server.json's host list. It exists because the
# thing it replaced — an inline `d["ws"] = url` in each deploy script — could
# only express "one server per game", and it expressed it by overwriting:
# deploying the arena from a second machine took the first one out of the book.
#
# Two rules do the work, and both are about not destroying someone else's
# state. Entries MERGE: a publish that carries only fire's keys leaves the
# same host's arena keys alone, because the arena is still running there and
# nothing in this call knows otherwise. And the legacy top-level address keys
# are RECOMPUTED rather than assigned: they exist for the frozen pages, which
# read `ws` and nothing else, so they must always name a host that speaks the
# protocol the live pages ship — which is not necessarily the host that just
# published.
set -euo pipefail

# `|| true`, or the guard below is dead code: under `set -e` an assignment
# whose command substitution fails IS a failing command, so a box with neither
# interpreter exited here with status 1 and not one byte of output — a deploy
# that stopped mid-sentence with nothing naming the cause. The interpreter is
# also RUN once rather than merely found: on Windows `python` is usually an App
# Execution Alias that resolves on PATH and is not an interpreter.
PY="$(command -v python3 || command -v python || true)"
[ -n "$PY" ] && "$PY" -c '' >/dev/null 2>&1 \
    || { echo "publish-host: need a working python3 (or python) on PATH" >&2; exit 1; }

NAME=""
VERSION=""
COMMIT=""
BY=""
REMOVE=""
RECOMPUTE=""
BOOK=""
REPO=""
BRANCH=""
FILE="server.json"
# Flattened triples: id url proto, id url proto, …
GAMES=()
# Flattened `drop=<id>`, appended after the triples; the python tells them
# apart by their key, so both arrive in one argv tail.
DROPS=()

usage() {
    sed -n '2,33p' "$0" >&2
    exit 2
}

while [ $# -gt 0 ]; do
    case "$1" in
        --name)      NAME="${2:-}"; shift 2 ;;
        --version)   VERSION="${2:-}"; shift 2 ;;
        --commit)    COMMIT="${2:-}"; shift 2 ;;
        --by)        BY="${2:-}"; shift 2 ;;
        --book)      BOOK="${2:-}"; shift 2 ;;
        --repo)      REPO="${2:-}"; shift 2 ;;
        --branch)    BRANCH="${2:-}"; shift 2 ;;
        --file)      FILE="${2:-}"; shift 2 ;;
        --remove)    REMOVE=1; shift ;;
        --recompute) RECOMPUTE=1; shift ;;
        # The triple is order-free on purpose: the deploy scripts build these
        # arguments from variables, and requiring a fixed order there would
        # have been one more thing to get subtly wrong.
        --game)      GAMES+=("game=${2:-}"); shift 2 ;;
        --drop-game) DROPS+=("drop=${2:-}"); shift 2 ;;
        --url)       GAMES+=("url=${2:-}"); shift 2 ;;
        --proto)     GAMES+=("proto=${2:-}"); shift 2 ;;
        -h|--help)   usage ;;
        *) echo "publish-host: unknown argument '$1'" >&2; usage ;;
    esac
done

if [ -z "$RECOMPUTE" ] && [ -z "$NAME" ]; then
    echo "publish-host: --name is required (except with --recompute)" >&2
    exit 2
fi
if [ -z "$BOOK" ] && [ -z "$REPO" ]; then
    echo "publish-host: one of --book <path> or --repo <url> is required" >&2
    exit 2
fi
if [ -n "$BOOK" ] && [ -n "$REPO" ]; then
    echo "publish-host: --book and --repo are alternatives, not a pair" >&2
    exit 2
fi
if [ -n "$REPO" ] && [ -z "$BRANCH" ]; then
    echo "publish-host: --repo needs --branch" >&2
    exit 2
fi

WORK=""
cleanup() { [ -n "$WORK" ] && rm -rf "$WORK"; return 0; }
trap cleanup EXIT

if [ -n "$REPO" ]; then
    WORK="$(mktemp -d -t ember-book-XXXXXX)"
    echo "== fetching $REPO ($BRANCH) =="
    # Fetch ONE branch one commit deep, rather than cloning. `git clone` picks
    # the remote's default branch, and `--depth 1` implies `--single-branch`,
    # so a plain shallow clone of the project repository brings back `main`
    # and no `origin/gh-pages` at all — the book branch would then look
    # missing, get restarted as an orphan, and the push would be rejected as
    # a non-fast-forward. Asking for the branch by name cannot make that
    # mistake, and a book branch carrying wasm bundles is not something a
    # host should download in full to add one line to a JSON file.
    git init -q "$WORK"
    git -C "$WORK" remote add origin "$REPO"
    if git -C "$WORK" fetch -q --depth 1 origin "$BRANCH" 2>/dev/null; then
        git -C "$WORK" checkout -q -B "$BRANCH" FETCH_HEAD
    else
        # The branch legitimately may not exist yet: a fork whose gh-pages
        # nobody has created is the normal first-run state for a mirror, and
        # failing here would mean "publish once by hand first", which is the
        # thing this script exists to remove. HEAD is unborn in a repository
        # this script just created, so the branch simply starts empty — no
        # orphan dance, and no chance of sweeping another branch's tree in.
        echo "   $BRANCH does not exist yet; starting it"
        git -C "$WORK" checkout -q -b "$BRANCH"
    fi
    TARGET="$WORK/$FILE"
else
    TARGET="$BOOK"
fi

# One python for the whole of the JSON, because every rule here is about the
# relationship between entries and no part of it is expressible in `sed`.
"$PY" - "$TARGET" "$FILE" "$NAME" "$VERSION" "$COMMIT" "$BY" \
    "${REMOVE:-}" "${RECOMPUTE:-}" \
    "${GAMES[@]+"${GAMES[@]}"}" "${DROPS[@]+"${DROPS[@]}"}" <<'PY'
import json, os, re, sys, time

path, filename, name, version, commit, by, remove, recompute = sys.argv[1:9]
raw_tail = sys.argv[9:]
raw_games = [i for i in raw_tail if not i.startswith("drop=")]
drops = [i[len("drop="):] for i in raw_tail if i.startswith("drop=")]
mirror = os.path.basename(filename) != "server.json"

NAME_RE = re.compile(r"^[a-z0-9-]{3,32}$")
ID_RE = re.compile(r"^[a-z][a-z0-9-]*$")
URL_RE = re.compile(r"^wss?://\S+$")


def die(msg):
    sys.stderr.write("publish-host: %s\n" % msg)
    raise SystemExit(1)


def write_json(path, doc):
    """Write the book through a temp file and a rename.

    Nothing here may leave a half-written book behind: this script's own first
    rule is that a book which will not parse is never overwritten, and a
    truncated write is exactly how such a book gets made.
    """
    tmp = path + ".tmp"
    with open(tmp, "w") as fh:
        json.dump(doc, fh, indent=2)
        fh.write("\n")
    os.replace(tmp, path)


# --- the game-id -> key derivation, the one rule a new game must not need code for.
# The arena was first and owns the bare names; everyone else is prefixed.
def addr_key(game_id):
    return "ws" if game_id == "arena" else "%s_ws" % game_id


def proto_key(game_id):
    return "proto" if game_id == "arena" else "%s_proto" % game_id


def version_key(game_id):
    return "version" if game_id == "arena" else "%s_version" % game_id


def commit_key(game_id):
    return "commit" if game_id == "arena" else "%s_commit" % game_id


def game_of(key):
    """Inverse of proto_key: the game id a top-level protocol key belongs to."""
    if key == "proto":
        return "arena"
    if key.endswith("_proto"):
        return key[: -len("_proto")]
    return None


# --- arguments -------------------------------------------------------------
if name and not NAME_RE.match(name):
    die("host name '%s' is not [a-z0-9-]{3,32}" % name)

# The triples arrive flattened and order-free (game=, url=, proto= in any
# order); a new game= closes the previous triple.
games, cur = [], {}
for item in raw_games:
    k, _, v = item.partition("=")
    if k == "game" and cur:
        games.append(cur)
        cur = {}
    cur[k] = v
if cur:
    games.append(cur)

for g in games:
    gid, url, proto = g.get("game", ""), g.get("url", ""), g.get("proto", "")
    if not ID_RE.match(gid):
        die("game id '%s' is not [a-z][a-z0-9-]*" % gid)
    if not URL_RE.match(url):
        die("--url for '%s' is '%s'; expected ws:// or wss://" % (gid, url))
    try:
        g["proto"] = int(proto)
    except ValueError:
        die("--proto for '%s' is '%s'; expected an integer" % (gid, proto))

# --drop-game retires ONE game on a host that keeps running the others. It
# exists because a merge can never take a key away, so the address of a game
# that was shut down for good stayed in the entry and kept winning the legacy
# recompute every time the host redeployed anything else — a dead address
# pinned on a live machine, with `--remove` (which drops the still-running
# games too) as the only cure.
for gid in drops:
    if not ID_RE.match(gid):
        die("--drop-game id '%s' is not [a-z][a-z0-9-]*" % gid)
    if any(g["game"] == gid for g in games):
        die("--game and --drop-game both name '%s'; decide which" % gid)
if drops and remove:
    die("--remove deletes the whole entry; --drop-game is the alternative")
if drops and recompute:
    die("--recompute touches nothing but the legacy keys; --drop-game needs --name")

# --- load ------------------------------------------------------------------
# A book that will not parse is NOT overwritten. The old inline publishers
# started from `{}` on a parse error, which turns one bad byte on gh-pages
# into the silent loss of every other host's entry.
doc = {}
if os.path.exists(path):
    with open(path) as fh:
        text = fh.read().strip()
    if text:
        try:
            doc = json.loads(text)
        except ValueError as e:
            die("%s exists but is not JSON (%s); refusing to overwrite it" % (path, e))
    if not isinstance(doc, dict):
        die("%s is not a JSON object; refusing to overwrite it" % path)

before = json.dumps(doc, sort_keys=True)
now = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


# The build stamp is PER GAME, by the same derivation as the address keys: the
# arena owns the bare `version`/`commit`, everyone else is `<id>_version` and
# `<id>_commit`. It used to be one pair for the whole entry, which meant a
# fire-only deploy stamped the host as running the newest ARENA build too —
# and the legacy recompute, which ranks by that number and holds no socket to
# check, then pointed `ws` at an older arena than the one it passed over.
# A call with no --game at all (a bare --version) is the arena's, as it always
# was.
stamp_ids = [g["game"] for g in games] or ["arena"]


def apply_entry(entry):
    """Merge this call's fields onto one host entry, in place."""
    entry["name"] = name
    for g in games:
        entry[addr_key(g["game"])] = g["url"]
        entry[proto_key(g["game"])] = g["proto"]
    for gid in stamp_ids:
        if version:
            entry[version_key(gid)] = version
        if commit:
            entry[commit_key(gid)] = commit
    for gid in drops:
        for k in (addr_key(gid), proto_key(gid), version_key(gid), commit_key(gid)):
            entry.pop(k, None)
    if by:
        entry["by"] = by
    entry["updated"] = now
    return entry


# --- mirror mode: the file IS one entry ------------------------------------
if mirror:
    if recompute:
        die("--recompute is meaningless for %s: a mirror file has no legacy keys" % filename)
    if remove:
        if os.path.exists(path):
            os.remove(path)
            print("removed %s" % path)
            print("CHANGED")
        else:
            print("%s does not exist" % path)
            print("UNCHANGED")
        raise SystemExit(0)
    if doc and doc.get("name") not in (None, "", name):
        # A mirror file holds exactly one host. Merging a second host's keys
        # onto the first host's entry would publish a chimera: one name, two
        # machines' addresses, and a page that connects to the wrong box.
        die("%s already holds host '%s'; a mirror file is one host" % (path, doc.get("name")))
    apply_entry(doc)
    after = json.dumps(doc, sort_keys=True)
    if after == before:
        print("UNCHANGED")
        raise SystemExit(0)
    write_json(path, doc)
    print("wrote mirror entry %s to %s" % (name, path))
    print("CHANGED")
    raise SystemExit(0)

# --- the book --------------------------------------------------------------
hosts = doc.get("hosts")
if hosts is None:
    hosts = []
elif not isinstance(hosts, list):
    die("%s has a 'hosts' key that is not a list" % path)

if remove:
    kept = [h for h in hosts if not (isinstance(h, dict) and h.get("name") == name)]
    if len(kept) == len(hosts):
        print("no entry named %s" % name)
    else:
        print("removed %s" % name)
    hosts = kept
elif not recompute:
    existing = None
    for h in hosts:
        if isinstance(h, dict) and h.get("name") == name:
            existing = h
            break
    if existing is None and drops and not games:
        # A retirement for a host the book does not carry. Creating the entry
        # here would publish a machine that had just been told to stop
        # advertising one.
        print("no entry named %s" % name)
    elif existing is None:
        # Built by the same merge, onto nothing. It used to be a second,
        # hand-written key order that had to be kept in step with apply_entry
        # by memory — and was not, the moment the stamp became per game.
        hosts.append(apply_entry({}))
        print("added %s" % name)
    else:
        # MERGE. This host may be running games this call says nothing about.
        apply_entry(existing)
        if drops:
            print("dropped %s from %s" % (", ".join(drops), name))
            if not any(k == "ws" or k.endswith("_ws") for k in existing):
                print("   %s now advertises no game at all; --remove deletes the entry" % name)
        print("updated %s" % name)

# Do not INTRODUCE an empty list into a book that never had one: on a
# --recompute with nothing to do that would be a change, and a change bumps
# `v`, and a bumped `v` makes every player re-download the bundles.
if hosts or "hosts" in doc:
    doc["hosts"] = hosts


# --- the legacy top-level address keys -------------------------------------
# Frozen pages on gh-pages read `ws`/`fire_ws` and know nothing about the
# list, so each of those must always name a host that speaks the protocol the
# LIVE pages ship — which deploy-pages.sh writes into the top-level protocol
# key. Recompute from the list rather than assigning the publisher's own
# address: a host that just deployed an older commit must not become the
# address a frozen page hands to every player.
def version_num(entry, gid):
    """The build number this entry claims FOR ONE GAME.

    The bare `version` is the fallback, not a mistake: every entry written
    before the stamp became per game carries only that, and reading such an
    entry as 0 would flip each legacy key to whichever new-format host
    published last — the same defect with the sign reversed.
    """
    raw = entry.get(version_key(gid))
    if raw is None:
        raw = entry.get("version")
    m = re.match(r"^r(\d+)", str(raw or ""))
    return int(m.group(1)) if m else 0


repointed = []
for key in [k for k in list(doc.keys()) if game_of(k) is not None]:
    gid = game_of(key)
    want = doc.get(key)
    ak = addr_key(gid)
    candidates = [
        h for h in hosts
        if isinstance(h, dict) and h.get(ak) and h.get(proto_key(gid)) == want
    ]
    if not candidates:
        # Left EXACTLY as it was, including absent. The alternative — clearing
        # it — would take every frozen page offline the moment the last host
        # on their protocol went away, when the address it already has may
        # well still work.
        continue
    # Newest build wins; `updated` breaks a tie between two hosts on the same
    # commit; the name breaks the remaining tie so the result is stable.
    best = max(candidates, key=lambda h: (version_num(h, gid), str(h.get("updated") or ""), str(h.get("name") or "")))
    if doc.get(ak) != best.get(ak):
        repointed.append("%s -> %s (%s)" % (ak, best.get(ak), best.get("name")))
    doc[ak] = best.get(ak)

for line in repointed:
    print("legacy %s" % line)

after = json.dumps(doc, sort_keys=True)
if after == before:
    print("UNCHANGED")
    raise SystemExit(0)

# `v` is the pages' cache-buster, so it moves exactly when the bytes do — not
# on every invocation, or every no-op publish would make every player
# re-download the wasm bundles.
doc["v"] = str(int(time.time()))
write_json(path, doc)
print("CHANGED")
PY

if [ -n "$REPO" ]; then
    (
        cd "$WORK"
        git add -- "$FILE" 2>/dev/null || true
        # `git add` cannot stage a deletion of a file it was told to add by
        # name, so stage the whole path either way.
        git add -A -- "$FILE" 2>/dev/null || true
        if git diff --cached --quiet; then
            echo "== $FILE unchanged; nothing to push =="
        else
            if [ -n "$REMOVE" ]; then
                MSG="Remove host $NAME"
            elif [ -n "$RECOMPUTE" ]; then
                MSG="Recompute the address book"
            else
                MSG="Publish host $NAME"
            fi
            # No co-author trailer: this commit is made by whoever is running
            # the host, on their own repository, and stamping it with anyone
            # else's name would be a lie about who published.
            git -c user.name="${GIT_AUTHOR_NAME:-ember publish-host}" \
                -c user.email="${GIT_AUTHOR_EMAIL:-ember@localhost}" \
                commit -q -m "$MSG"
            git push -q origin "$BRANCH"
            echo "== pushed: $MSG =="
        fi
    )
fi
