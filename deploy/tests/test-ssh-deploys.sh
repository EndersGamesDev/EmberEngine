#!/usr/bin/env bash
# The two workstation deploys, driven end to end against PATH shims.
#
#   bash deploy/tests/test-ssh-deploys.sh
#
# NO HOST IS CONTACTED. `ssh`, `scp`, `cargo` and `sleep` are replaced by the
# scripts in deploy/tests/shims, which log their argv and answer the few
# things the deploys read back. The git side is real: the test builds a small
# repository with a gh-pages branch and a bare `origin`, so `git archive`,
# `git show <ref>:…`, the gh-pages worktree, the commit and the push are all
# the genuine article.
#
# What it is here to catch: the arena deploy taking the fire deploy's keys off
# the shared entry, a build stamped from the working tree rather than the ref
# being deployed, and a launch that forgets the host name.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
DEPLOY="$(cd "$HERE/.." && pwd)"
# shellcheck source=deploy/tests/lib.sh
. "$HERE/lib.sh"

TMP="$(mktemp -d -t ember-sshtest-XXXXXX)"
trap 'rm -rf "$TMP"' EXIT

# --- shims -----------------------------------------------------------------
# Copied rather than used in place: the checkout may live on a filesystem that
# does not carry the executable bit (a Windows tree seen from WSL).
SHIMS="$TMP/shims"
mkdir -p "$SHIMS"
for s in ssh scp cargo sleep; do
    cp "$HERE/shims/$s" "$SHIMS/$s"
    chmod +x "$SHIMS/$s"
done
export PATH="$SHIMS:$PATH"
export SHIM_LOG="$TMP/argv.log"
export SHIM_HOST_NAME=amber-otter
: > "$SHIM_LOG"

# --- a small repository that looks enough like this one ---------------------
REPO="$TMP/repo"
ORIGIN="$TMP/origin.git"
git init -q --bare "$ORIGIN"
mkdir -p "$REPO/crates/arena-core/src" "$REPO/crates/fire-core/src" "$REPO/assets/models/fire"
cd "$REPO"
git init -q -b main .
git remote add origin "$ORIGIN"
git config user.name "ember tests"
git config user.email "tests@ember.local"
printf '[workspace]\nmembers = []\n' > Cargo.toml
printf '# lock\n' > Cargo.lock
printf 'pub const PROTO_VERSION: u16 = 12;\n' > crates/arena-core/src/proto.rs
printf 'pub const PROTO_VERSION: u16 = 1;\n' > crates/fire-core/src/proto.rs
printf 'placeholder\n' > assets/models/fire/placeholder.txt
cp -r "$DEPLOY" "$REPO/deploy"
rm -rf "$REPO/deploy/tests"
git add -A
git commit -qm "first"
OLD_SHA="$(git rev-parse --short HEAD)"

# A gh-pages branch carrying the protocol keys deploy-pages.sh writes, so the
# legacy address keys have something to be computed against.
git checkout -q --orphan gh-pages
git rm -rq --cached .
find . -mindepth 1 -maxdepth 1 ! -name .git -exec rm -rf {} +
printf '{"v": "1", "proto": 12, "fire_proto": 1}\n' > server.json
git add server.json
git commit -qm "pages"
git checkout -q main
git push -q origin main gh-pages

BOOK_OF() {
    # The server.json as it now stands on the bare origin's gh-pages.
    git -C "$ORIGIN" show gh-pages:server.json > "$TMP/book.json"
    echo "$TMP/book.json"
}

echo "== deploy-pong-online.sh =="
T0="$(date +%s)"
if EMBER_HOST=fakehost bash "$REPO/deploy/deploy-pong-online.sh" > "$TMP/arena.log" 2>&1; then
    ok "the arena deploy ran through in $(( $(date +%s) - T0 ))s"
else
    bad "the arena deploy FAILED"
    tail -30 "$TMP/arena.log" >&2
fi
ARENA_LOG="$(cat "$TMP/arena.log")"
contains "$ARENA_LOG" "publishes as 'amber-otter'" "it resolved the host's name over ssh"
ARGV="$(cat "$SHIM_LOG")"
contains "$ARGV" "EMBER_HOST_NAME='' bash -s" "host-name.sh was piped to the host"
contains "$ARGV" "EMBER_BUILD_VERSION=r1 EMBER_BUILD_COMMIT=$OLD_SHA" "the build was stamped with the ref"
contains "$ARGV" "EMBER_HOST_NAME=amber-otter RUST_LOG=info nohup" "the server was launched with its name in the environment"
case "$ARGV" in
    *"arena-server --name"*) bad "the launch used a --name flag an older binary would reject" ;;
    *) ok "the launch used no --name flag" ;;
esac

BOOK="$(BOOK_OF)"
is "$(jget "$BOOK" 'len(d["hosts"])')" "1" "one host in the book"
is "$(jget "$BOOK" 'd["hosts"][0]["name"]')" "amber-otter" "under the name the host gave"
is "$(jget "$BOOK" 'd["hosts"][0]["ws"]')" "wss://test-arena.trycloudflare.com" "with the minted address"
is "$(jget "$BOOK" 'd["hosts"][0]["proto"]')" "12" "and the protocol from the deployed ref"
is "$(jget "$BOOK" 'd["hosts"][0]["version"]')" "r1" "version"
is "$(jget "$BOOK" 'd["hosts"][0]["commit"]')" "$OLD_SHA" "commit"
is "$(jget "$BOOK" 'bool(d["hosts"][0]["by"])')" "True" "and a by line"
is "$(jget "$BOOK" 'd["ws"]')" "wss://test-arena.trycloudflare.com" "the legacy ws was recomputed onto it"
is "$(jget "$BOOK" 'd["proto"]')" "12" "and the top-level protocol was not touched"

echo "== deploy-fire-online.sh merges onto the same entry =="
: > "$SHIM_LOG"
if EMBER_HOST=fakehost bash "$REPO/deploy/deploy-fire-online.sh" > "$TMP/fire.log" 2>&1; then
    ok "the fire deploy ran through"
else
    bad "the fire deploy FAILED"
    tail -30 "$TMP/fire.log" >&2
fi
ARGV="$(cat "$SHIM_LOG")"
contains "$ARGV" "EMBER_HOST_NAME=amber-otter RUST_LOG=info nohup" "fire-server was launched with the name too"
BOOK="$(BOOK_OF)"
is "$(jget "$BOOK" 'len(d["hosts"])')" "1" "still one host: both games are on one machine"
is "$(jget "$BOOK" 'd["hosts"][0]["fire_ws"]')" "wss://test-fire.trycloudflare.com" "fire's address was added"
is "$(jget "$BOOK" 'd["hosts"][0]["fire_proto"]')" "1" "with fire's own protocol"
is "$(jget "$BOOK" 'd["hosts"][0]["ws"]')" "wss://test-arena.trycloudflare.com" "and the arena's address SURVIVED"
is "$(jget "$BOOK" 'd["hosts"][0]["fire_version"]')" "r1" "fire published its build stamp on its own key"
is "$(jget "$BOOK" 'd["hosts"][0]["version"]')" "r1" "and left the arena's bare stamp alone"
is "$(jget "$BOOK" 'd["fire_ws"]')" "wss://test-fire.trycloudflare.com" "legacy fire_ws recomputed"
is "$(jget "$BOOK" 'd["ws"]')" "wss://test-arena.trycloudflare.com" "legacy ws untouched by a fire deploy"

echo "== a second host does not evict the first =="
: > "$SHIM_LOG"
if SHIM_HOST_NAME=flint-heron SHIM_TUNNEL=other EMBER_HOST=otherbox \
        bash "$REPO/deploy/deploy-pong-online.sh" > "$TMP/arena2.log" 2>&1; then
    ok "the second host deployed"
else
    bad "the second host's deploy FAILED"
    tail -30 "$TMP/arena2.log" >&2
fi
BOOK="$(BOOK_OF)"
is "$(jget "$BOOK" 'len(d["hosts"])')" "2" "two hosts in the book"
is "$(jget "$BOOK" 'sorted(h["name"] for h in d["hosts"])')" "['amber-otter', 'flint-heron']" "both by name"
is "$(jget "$BOOK" '[h for h in d["hosts"] if h["name"]=="amber-otter"][0]["fire_ws"]')" \
    "wss://test-fire.trycloudflare.com" "the first host kept its fire address"
is "$(jget "$BOOK" '[h for h in d["hosts"] if h["name"]=="flint-heron"][0]["ws"]')" \
    "wss://other-arena.trycloudflare.com" "and the second published its own"

echo "== EMBER_REF deploys an older commit, and says so =="
# Move the protocol on main, and leave the tree dirty as well: a named ref has
# nothing to do with what is being edited here, so the dirty-tree refusal must
# not fire.
cd "$REPO"
printf 'pub const PROTO_VERSION: u16 = 13;\n' > crates/arena-core/src/proto.rs
git commit -qam "bump the arena protocol to 13"
NEW_SHA="$(git rev-parse --short HEAD)"
printf '// uncommitted scribble\n' >> crates/arena-core/src/proto.rs
: > "$SHIM_LOG"
if SHIM_HOST_NAME=quiet-raven EMBER_HOST=oldbox EMBER_REF="$OLD_SHA" \
        bash "$REPO/deploy/deploy-pong-online.sh" > "$TMP/arena3.log" 2>&1; then
    ok "a named ref deploys over a dirty tree"
else
    bad "the pinned-ref deploy FAILED"
    tail -30 "$TMP/arena3.log" >&2
fi
ARGV="$(cat "$SHIM_LOG")"
contains "$ARGV" "EMBER_BUILD_VERSION=r1 EMBER_BUILD_COMMIT=$OLD_SHA" "stamped from the ref, not from HEAD"
BOOK="$(BOOK_OF)"
is "$(jget "$BOOK" '[h for h in d["hosts"] if h["name"]=="quiet-raven"][0]["proto"]')" "12" \
    "the entry says the protocol of the commit it was built from"
is "$(jget "$BOOK" '[h for h in d["hosts"] if h["name"]=="quiet-raven"][0]["version"]')" "r1" \
    "and its version"

echo "== a dirty tree still refuses an unpinned deploy =="
if EMBER_HOST=fakehost bash "$REPO/deploy/deploy-pong-online.sh" > "$TMP/dirty.log" 2>&1; then
    bad "a dirty tree was deployed"
else
    ok "a dirty HEAD deploy is refused"
fi
contains "$(cat "$TMP/dirty.log")" "working tree is dirty" "and says why"
git checkout -q -- crates/arena-core/src/proto.rs

echo "== the newest build on the live protocol owns the legacy key =="
# main now speaks 13 while the pages still say 12, so the r2 host must NOT
# become `ws` — that is the whole point of recomputing rather than assigning.
: > "$SHIM_LOG"
if SHIM_HOST_NAME=misty-egret SHIM_TUNNEL=newest EMBER_HOST=newbox \
        bash "$REPO/deploy/deploy-pong-online.sh" > "$TMP/arena4.log" 2>&1; then
    ok "the newest-commit host deployed"
else
    bad "the newest-commit host FAILED"
    tail -30 "$TMP/arena4.log" >&2
fi
BOOK="$(BOOK_OF)"
is "$(jget "$BOOK" '[h for h in d["hosts"] if h["name"]=="misty-egret"][0]["proto"]')" "13" "it publishes protocol 13"
is "$(jget "$BOOK" '[h for h in d["hosts"] if h["name"]=="misty-egret"][0]["ws"]')" \
    "wss://newest-arena.trycloudflare.com" "at its own address"
is "$(jget "$BOOK" 'd["ws"]')" "wss://test-arena.trycloudflare.com" \
    "but the legacy ws still names a host the live pages can join"

echo "== another writer moved gh-pages, and a worktree still holds the local branch =="
# The two states that used to make a deploy fail permanently, together, because
# one caused the other. Nothing here fetches the LOCAL gh-pages, so a second
# writer's publish left it behind and the push was rejected as a
# non-fast-forward; and the rejection aborted the script before its
# `worktree remove`, so gh-pages stayed checked out in a temp directory and
# every later deploy — of either game, and the pages deploy — died at its own
# `worktree add`. Both times the tunnel had already been restarted, so the book
# was left naming a dead domain.
OTHER="$TMP/other"
git clone -q --branch gh-pages "$ORIGIN" "$OTHER"
git -C "$OTHER" config user.name "another writer"
git -C "$OTHER" config user.email "other@ember.local"
bash "$REPO/deploy/publish-host.sh" --book "$OTHER/server.json" --name distant-plover \
    --game arena --url wss://distant.example --proto 12 --version r1 >/dev/null
git -C "$OTHER" commit -qam "another writer publishes"
git -C "$OTHER" push -q origin gh-pages
: > "$SHIM_LOG"
if SHIM_HOST_NAME=coral-shrike SHIM_TUNNEL=coral EMBER_HOST=coralbox \
        bash "$REPO/deploy/deploy-pong-online.sh" > "$TMP/arena5.log" 2>&1; then
    ok "a deploy publishes over a local gh-pages that is behind origin"
else
    bad "the deploy FAILED with the local gh-pages behind origin"
    tail -30 "$TMP/arena5.log" >&2
fi
BOOK="$(BOOK_OF)"
is "$(jget "$BOOK" '[h["name"] for h in d["hosts"]].count("coral-shrike")')" "1" \
    "its entry reached the branch origin actually has"
is "$(jget "$BOOK" '[h["name"] for h in d["hosts"]].count("distant-plover")')" "1" \
    "and the other writer's entry was merged, not overwritten"
case "$(git -C "$REPO" worktree list)" in
    *ember-pages*) bad "the deploy left a gh-pages worktree registered" ;;
    *)             ok "and no gh-pages worktree of this checkout was created at all" ;;
esac

echo "== a worktree already holding gh-pages does not wedge a deploy =="
# The leaked state itself: an earlier failure left gh-pages checked out in a
# temp directory, and from then on every deploy of either game — and the pages
# deploy — died at its own `worktree add` with "already used by worktree",
# after restarting the server and minting a fresh tunnel.
git -C "$REPO" worktree add -q "$TMP/stale-pages" gh-pages
: > "$SHIM_LOG"
if SHIM_HOST_NAME=coral-shrike SHIM_TUNNEL=coral EMBER_HOST=coralbox \
        bash "$REPO/deploy/deploy-fire-online.sh" > "$TMP/fire2.log" 2>&1; then
    ok "the fire deploy runs with gh-pages checked out elsewhere"
else
    bad "the fire deploy FAILED with gh-pages checked out elsewhere"
    tail -30 "$TMP/fire2.log" >&2
fi
BOOK="$(BOOK_OF)"
is "$(jget "$BOOK" '[h for h in d["hosts"] if h["name"]=="coral-shrike"][0]["fire_ws"]')" \
    "wss://coral-fire.trycloudflare.com" "and published fire's address anyway"
git -C "$REPO" worktree remove --force "$TMP/stale-pages"

summary ssh-deploys
