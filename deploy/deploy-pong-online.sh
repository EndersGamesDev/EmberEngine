#!/usr/bin/env bash
# Bring pong online multiplayer up end to end:
#   1. build + (re)start arena-server on the target host (127.0.0.1:7780)
#   2. (re)start a Cloudflare quick tunnel in front of it — this mints a
#      fresh https://…trycloudflare.com domain on EVERY restart
#   3. publish the new domain to server.json on GitHub Pages so the web
#      page finds the current server
#   4. health-check through the public URL
#
# Run from Windows (git-bash): bash deploy/deploy-pong-online.sh
# Requires ssh to the target host to work non-interactively from here.
#
# ACCOUNTS. The host may carry more than one. The arena historically ran as
# `ender`, deployed from a particular workstation, while this machine lands as
# `end`. This script only ever stops processes belonging to whoever the ssh
# lands as, and refuses to
# continue if the port is held by someone else. Run it from the machine whose
# account owns the running server, or it will tell you it cannot — which is
# the point: the previous version killed nothing, said nothing (the call was
# wrapped in `|| true`), then failed to bind, and reported a successful deploy
# over a server that had never started.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
# The target host. The default is the `sokol` ssh alias; export EMBER_HOST to
# choose another host without editing the script:
#
#     EMBER_HOST=newbox bash deploy/deploy-pong-online.sh
#
# It must be a name `ssh` already resolves and can log into without a prompt
# (BatchMode is on everywhere below), i.e. an entry in ~/.ssh/config with a key.
REMOTE="${EMBER_HOST:-sokol}"

# NOTE: SELinux on specht denied many ports (7778, 8890, 8891 tested); 7780 was
# the verified-bindable one there. A new host may differ — if the bind fails,
# test candidates before assuming the server is broken.
PORT=7780
BIND="127.0.0.1:$PORT"
cd "$REPO_DIR"

# Which commit this host runs. A host is allowed to stay on an older one
# (docs/hosts.md §7): it keeps serving the frozen pages that speak its
# protocol, and the address book carries the version so a page can prefer the
# newest host that speaks its own.
#
#     EMBER_REF=v12 bash deploy/deploy-pong-online.sh
REF="${EMBER_REF:-HEAD}"

# `git archive` below deploys the COMMITTED tree, so refuse a dirty one rather
# than shipping something other than what is in front of you. Only when the ref
# IS the working tree's HEAD: deploying a named older ref has nothing to do
# with what happens to be edited here, and refusing then would mean a host
# could not be pinned to an older build without stashing first.
if [ "$REF" = "HEAD" ]; then
    if ! git diff --quiet || ! git diff --cached --quiet; then
        echo "FAILED: working tree is dirty; this deploys the committed tree." >&2
        git status --short >&2
        exit 1
    fi
fi
git rev-parse --verify -q "$REF^{commit}" >/dev/null \
    || { echo "FAILED: EMBER_REF='$REF' names no commit here." >&2; exit 1; }

# The arena's crate and package names, read FROM THE REF rather than hardcoded.
# The arena was called pong until it was renamed, and §7's whole point is that
# a host may be pinned to an older commit — every published arena build up to
# v11 is on the far side of that rename. Hardcoding the new names meant those
# refs could not be deployed at all: the remote build died with "package ID
# specification `arena-server` did not match any packages", and had it
# survived, the protocol read would have come back empty from a path that does
# not exist in that tree.
if git cat-file -e "$REF:crates/arena-core/src/proto.rs" 2>/dev/null; then
    ARENA_CRATE=arena-core
    ARENA_PKG=arena-server
elif git cat-file -e "$REF:crates/pong-core/src/proto.rs" 2>/dev/null; then
    ARENA_CRATE=pong-core
    ARENA_PKG=pong-server
else
    echo "FAILED: $REF carries neither crates/arena-core nor crates/pong-core." >&2
    exit 1
fi
# `arena-serve[r]`: the bracket keeps pgrep's own command line from matching.
ARENA_PGREP="${ARENA_PKG%r}[r]"
# The public health check below runs wsbot from THIS checkout rather than from
# the ref, so it needs the name the working tree carries, which is the ref's
# name only when EMBER_REF is HEAD.
if [ -d "$REPO_DIR/crates/arena-server" ]; then
    LOCAL_ARENA_PKG=arena-server
else
    LOCAL_ARENA_PKG=pong-server
fi

# The build stamp the server reports in its Welcome, and the entry publishes.
# Read from the REF, not from HEAD: a deploy of an older commit must say it is
# an older commit or the preferred-host rule ranks it as the newest build.
VERSION="r$(git rev-list --count "$REF")"
COMMIT="$(git rev-parse --short "$REF")"
echo "== deploying $VERSION · $COMMIT (ref $REF) =="

echo "== checking $REMOTE is reachable =="
# Fail here with a sentence, rather than twenty lines further down with a raw
# scp error. Especially relevant while the games are being moved to a new host:
# the usual cause is EMBER_HOST naming something ssh has no config entry for,
# or the wireproxy tunnel to it not being up on this workstation.
if ! ssh -o BatchMode=yes -o ConnectTimeout=10 "$REMOTE" true 2>/dev/null; then
    echo "FAILED: cannot ssh to '$REMOTE'." >&2
    echo "        Set EMBER_HOST to the target host, check it is in ~/.ssh/config" >&2
    echo "        with a key, and that any tunnel to it is up on this machine." >&2
    exit 1
fi

echo "== syncing source =="
# `git archive`, not `tar` over the working tree. The old form excluded only
# target/ and .git, so ~250 MB of untracked artist assets rode to specht on
# every run — 351 MB to build a server that needs none of it. The manifests
# and crates alone are ~0.5 MB and build identically.
TARBALL="$(mktemp -t ember-src-XXXX.tar.gz)"
# Reaped on every path: a failed scp used to leave half a megabyte of source
# tarball in the temp directory on every retry.
trap 'st=$?; rm -f "$TARBALL"; exit $st' EXIT
git archive --format=tar.gz -o "$TARBALL" "$REF" Cargo.toml Cargo.lock crates/
echo "   $(du -h "$TARBALL" | cut -f1) of committed source"
scp -o BatchMode=yes "$TARBALL" "$REMOTE":ember-src.tar.gz
rm -f "$TARBALL"
ssh -o BatchMode=yes "$REMOTE" \
    'rm -rf ~/ember-src && mkdir ~/ember-src && tar xzf ~/ember-src.tar.gz -C ~/ember-src'

echo "== resolving this host's name =="
# The name is a property of the MACHINE, not of the workstation deploying to
# it, so it is generated there and kept there (docs/hosts.md §6). host-name.sh
# is piped in rather than installed: the host has no checkout of this repo.
# A local EMBER_HOST_NAME still wins, passed through this one call's
# environment, which is how a name collision gets broken by hand.
HOST_NAME="$(ssh -o BatchMode=yes "$REMOTE" \
    "EMBER_HOST_NAME='${EMBER_HOST_NAME:-}' bash -s" < "$REPO_DIR/deploy/host-name.sh")"
HOST_NAME="$(printf '%s' "$HOST_NAME" | tr -d '[:space:]')"
if ! printf '%s' "$HOST_NAME" | grep -qE '^[a-z0-9-]{3,32}$'; then
    echo "FAILED: '$REMOTE' produced no usable host name ('$HOST_NAME')." >&2
    exit 1
fi
echo "   $REMOTE publishes as '$HOST_NAME'"

echo "== building $ARENA_PKG (toolbox: ember-build) =="
# EMBER_BUILD_VERSION/EMBER_BUILD_COMMIT are read by the server crate's
# build.rs through option_env!, so the binary can say which commit it is in
# its Welcome. They must be set for the BUILD, not the launch.
ssh -o BatchMode=yes "$REMOTE" \
    "toolbox run -c ember-build bash -lc 'source ~/.cargo/env && cd ~/ember-src && EMBER_BUILD_VERSION=$VERSION EMBER_BUILD_COMMIT=$COMMIT cargo build --release -p $ARENA_PKG'"

echo "== restarting $ARENA_PKG =="
# Two ways to own the process, and they must never both be used at once. If
# `install-watchdog.sh` has enabled the systemd unit, IT owns the lifecycle:
# pkill+nohup here would race it, because systemd sees its child die and
# immediately starts a replacement, and then two processes fight over the port.
# One loses the bind, which reads exactly like "the deploy failed" while the
# old build keeps serving. So: detect, and defer.
#
# The fallback is not legacy — a freshly built host has no units yet, and this
# script has to be what brings the game up there in the first place.
MANAGED=""
USER_MANAGER=""
if ssh -o BatchMode=yes "$REMOTE" \
        'test -n "${XDG_RUNTIME_DIR:-}" && test -S "$XDG_RUNTIME_DIR/bus" && systemctl --user is-system-running >/dev/null 2>&1'; then
    USER_MANAGER=1
else
    echo "   no running systemd user manager; using a direct launch"
fi
if [ -n "$USER_MANAGER" ] && ssh -o BatchMode=yes "$REMOTE" \
        'systemctl --user is-enabled ember-pong.service' >/dev/null 2>&1; then
    # `is-enabled` only reads a symlink. It does NOT prove the manager can be
    # COMMANDED, and on specht it cannot: SELinux is Enforcing and permits the
    # read verbs (is-enabled, show, list-units, is-system-running) while
    # denying start/restart/stop/is-active from a non-interactive ssh session.
    # Measured directly — enable succeeds, start returns "Access denied".
    # Predicting from is-enabled therefore takes the systemd path and then
    # fails on the restart, which is exactly what happened on the first real
    # attempt. Attempt it and fall back on refusal: the restart's own exit
    # status is the only honest signal, and the health probe below is what
    # verifies the result either way.
    echo "   ember-pong.service is enabled; trying systemd"
    if ssh -o BatchMode=yes "$REMOTE" \
            'systemctl --user restart ember-pong.service' >/dev/null 2>&1; then
        MANAGED=1
        echo "   systemd accepted the restart and owns the process"
    else
        echo "   systemd refused it; falling back to a direct launch"
    fi
fi

if [ -z "$MANAGED" ]; then
    echo "   launching directly"
    # Kill and launch are separate ssh calls on purpose: in a combined call the
    # launch text matches the pkill pattern and kills its own shell.
    # Only our own account's processes. `pkill -u ender` run from any other
    # account matched nothing and was swallowed by the trailing `true`.
    # Kill either artifact name so the first renamed deploy also replaces the old process.
    ssh -o BatchMode=yes "$REMOTE" \
        'pkill -u "$(id -un)" -f "pong-serve[r]" 2>/dev/null; pkill -u "$(id -un)" -f "arena-serve[r]" 2>/dev/null; true'
    sleep 1

    echo "== checking nobody else holds $PORT =="
    # Only meaningful in this branch: under systemd the port is legitimately
    # held by our own unit across the restart.
    HOLDER="$(ssh -o BatchMode=yes "$REMOTE" \
        "ss -ltnp 'sport = :$PORT' 2>/dev/null | tail -n +2" || true)"
    if [ -n "$HOLDER" ]; then
        echo "FAILED: something is already listening on $PORT:" >&2
        echo "  $HOLDER" >&2
        echo "        A row with no process name belongs to another account and" >&2
        echo "        cannot be stopped from here. Deploy from the machine whose" >&2
        echo "        account owns it." >&2
        exit 1
    fi
    # EMBER_HOST_NAME, never a `--name` flag. A host may be running an older
    # commit whose binary has never heard of the flag, and an unknown flag is
    # a crash loop where an unknown environment variable is simply ignored.
    ssh -o BatchMode=yes -f "$REMOTE" \
        "EMBER_HOST_NAME=$HOST_NAME RUST_LOG=info nohup ~/ember-src/target/release/$ARENA_PKG --bind $BIND >> ~/pong-server.log 2>&1 &"
    sleep 2
    if ! ssh -o BatchMode=yes "$REMOTE" "pgrep -u \"\$(id -un)\" -f \"$ARENA_PGREP\" >/dev/null"; then
        echo "FAILED: $ARENA_PKG is not running. Last log lines:" >&2
        ssh -o BatchMode=yes "$REMOTE" 'tail -20 ~/pong-server.log' >&2 || true
        exit 1
    fi
fi
ssh -o BatchMode=yes "$REMOTE" 'tail -1 ~/pong-server.log'

echo "== local health check (before exposing it) =="
# Prove the hub loop is alive on the loopback side first, so a failure at the
# public check below is unambiguously the tunnel rather than the server. Worth
# the extra minute on a host we have never deployed to before.
if ! ssh -o BatchMode=yes "$REMOTE" \
    "toolbox run -c ember-build bash -lc 'source ~/.cargo/env && cd ~/ember-src && cargo run --release -q -p $ARENA_PKG --example wsbot -- ws://$BIND create local-healthcheck - healthcheck 6'"; then
    echo "FAILED: the server is listening but wsbot could not create a lobby on it." >&2
    ssh -o BatchMode=yes "$REMOTE" 'tail -20 ~/pong-server.log' >&2 || true
    exit 1
fi

echo "== restarting tunnel (fresh domain) =="
# Pattern must not match this command's own shell: "cloudflared.log" would
# match a bare "cloudflare[d]" regex, so anchor on " tunnel" and truncate
# the log in a separate call.
# Same split as the server. A quick tunnel mints a NEW random hostname every
# time it starts, so this restart is what forces the republish below — and it
# is also why an unattended systemd restart leaves the game healthy at an
# address server.json does not name. A NAMED tunnel would remove both.
if [ -n "$MANAGED" ]; then
    # The unit truncates its own log in ExecStartPre, so the grep below still
    # finds only the hostname from this run.
    ssh -o BatchMode=yes "$REMOTE" 'systemctl --user restart ember-pong-tunnel.service'
else
    # Our own account, and anchored on THIS port. A bare "cloudflare[d] tunnel"
    # pattern also matches Fire Racer's tunnel on 7781, so redeploying the
    # arena would have silently taken the other game offline.
    ssh -o BatchMode=yes "$REMOTE" \
        "pkill -u \"\$(id -un)\" -f \"cloudflare[d] tunnel --url http://$BIND\" 2>/dev/null; true"
    ssh -o BatchMode=yes "$REMOTE" ': > ~/cloudflared.log'
    ssh -o BatchMode=yes -f "$REMOTE" \
        "nohup ~/bin/cloudflared tunnel --url http://$BIND --no-autoupdate >> ~/cloudflared.log 2>&1 &"
fi

echo "== waiting for the tunnel domain =="
TUNNEL=""
for _ in $(seq 1 30); do
    sleep 2
    TUNNEL=$(ssh -o BatchMode=yes "$REMOTE" \
        "grep -oE 'https://[a-z0-9-]+\\.trycloudflare\\.com' ~/cloudflared.log | head -1" || true)
    [ -n "$TUNNEL" ] && break
done
if [ -z "$TUNNEL" ]; then
    echo "FAILED: no trycloudflare domain appeared; see ~/cloudflared.log on the target host" >&2
    exit 1
fi
WS_URL="wss://${TUNNEL#https://}"
echo "tunnel domain: $TUNNEL  ->  $WS_URL"

echo "== health check through the public URL =="
# NOT a bare HTTP 101. A 101 only says a connection thread completed the
# WebSocket handshake; arena-server was observed on the target host with its listener up
# and its hub loop dead, handing out 101s and closing immediately — and this
# check printed ONLINE over it. wsbot speaks the protocol: it creates a lobby
# and counts state updates, so it can only pass if the hub is actually
# simulating. Exit 0 = the online loop works.
# Let the hostname exist before anything asks for it. The first DNS query for
# a brand-new *.trycloudflare.com name can land before Cloudflare has published
# the record, and a resolver that caches that NXDOMAIN keeps returning it long
# after the record appears. Every later attempt then fails against a poisoned
# cache while the tunnel is provably fine from anywhere else — which is exactly
# what happened on 2026-09-01: two arena deploys failed here with `os error
# 11001` (host unknown) while 1.1.1.1 resolved the name instantly and the
# tunnel answered 101. Not asking too early is the fix; retrying harder is not,
# because by then the bad answer is already cached.
echo "== letting the tunnel hostname propagate =="
sleep 15

ok=""
for _ in $(seq 1 10); do
    if cargo run --release -q -p "$LOCAL_ARENA_PKG" --example wsbot -- \
        "$WS_URL" create deploy-healthcheck - healthcheck 6; then
        ok=1
        break
    fi
    sleep 3
done
if [ -z "$ok" ]; then
    echo "FAILED: the tunnel is up but wsbot could not create a lobby through it." >&2
    echo "        Not publishing server.json — the page keeps its previous value," >&2
    echo "        which is better than pointing every player at a server we have" >&2
    echo "        not proved is alive." >&2
    exit 1
fi

echo "== publishing this host's entry to the address book =="
# Only after the health check passed. This used to run BEFORE it, so a
# failed deploy still pointed every player at the new domain.
#
# The protocol number comes from the REF being deployed, not from the working
# tree: the entry has to say what the binary on that host actually speaks, and
# those differ the moment EMBER_REF names an older commit.
PROTO="$(git show "$REF:crates/$ARENA_CRATE/src/proto.rs" \
    | grep -oE 'PROTO_VERSION: u16 = [0-9]+' | grep -oE '[0-9]+$')"
[ -n "$PROTO" ] || { echo "FAILED: no PROTO_VERSION in $REF:crates/$ARENA_CRATE/src/proto.rs" >&2; exit 1; }

# publish-host.sh upserts THIS host's entry and recomputes the legacy `ws`
# from the whole list. The inline python this replaced assigned `ws` directly,
# which is only correct while there is exactly one host: the second machine to
# deploy took the first one's address out of the book, and a host deployed
# from an older commit pointed every frozen page at a protocol they could not
# join.
#
# `--repo`, not a gh-pages worktree of this checkout. Two failures came out of
# that worktree and both were permanent. It checked out the workstation's LOCAL
# gh-pages, which nothing ever fetches — `git fetch` moves origin/gh-pages and
# not the branch — so as soon as any other writer published (a second
# workstation, a host running `host.sh` with EMBER_PUBLISH=upstream) the push
# was rejected as a non-fast-forward. And the removal at the end of the block
# was not a trap, so that rejection also left the worktree registered with
# gh-pages checked out in a temp directory, which made EVERY later deploy of
# either game and the pages deploy itself die at their own `worktree add` until
# a human ran `git worktree remove`. Both times the tunnel had already been
# restarted, so the book was left naming a domain that no longer existed.
# publish-host.sh fetches the branch one commit deep into its own temp
# repository under its own EXIT trap, and retries by refetching if the branch
# moves under it; nothing it does can touch this checkout's worktree registry.
bash "$REPO_DIR/deploy/publish-host.sh" \
    --repo "$(git -C "$REPO_DIR" remote get-url origin)" --branch gh-pages \
    --name "$HOST_NAME" \
    --game arena --url "$WS_URL" --proto "$PROTO" \
    --version "$VERSION" --commit "$COMMIT" \
    --by "$(id -un)@$REMOTE"

echo "== ONLINE: $HOST_NAME -> $WS_URL (the page picks it from server.json) =="
