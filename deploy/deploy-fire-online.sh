#!/usr/bin/env bash
# Bring Fire Racer online multiplayer up end to end:
#   1. build + (re)start fire-server on the target host
#   2. (re)start a Cloudflare quick tunnel in front of it — this mints a
#      fresh https://…trycloudflare.com domain on EVERY restart
#   3. publish the new domain to server.json on GitHub Pages as "fire_ws"
#   4. health-check by SPEAKING THE PROTOCOL through the public URL
#
# Run from Windows (git-bash): bash deploy/deploy-fire-online.sh
# Requires ssh to the target host to work non-interactively from here.
#
# Deliberately a separate script, port, tunnel and server.json key from the
# arena's. The two games speak different protocols with independent version
# numbers, so redeploying one must never be able to knock the other offline.
#
# ACCOUNTS. The host may carry more than one: the arena historically ran as
# `ender`, deployed from a different workstation, holding 127.0.0.1:7780. This
# script runs as whoever the ssh lands as and touches nothing belonging to
# anyone else — see the pkill and the port guard below.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
# The target host. specht is being decommissioned; export EMBER_HOST to point
# this at its replacement without editing the script:
#
#     EMBER_HOST=newbox bash deploy/deploy-fire-online.sh
#
# It must be a name `ssh` already resolves and can log into without a prompt
# (BatchMode is on everywhere below), i.e. an entry in ~/.ssh/config with a key.
REMOTE="${EMBER_HOST:-specht}"

PORT=7781
BIND="127.0.0.1:$PORT"
REMOTE_DIR="ember-src-fire"
# Matching on our own absolute binary path means the pattern cannot hit
# another account's server even if one is running the same program.
BIN="$REMOTE_DIR/target/release/fire-server"

cd "$REPO_DIR"

# Which commit this host runs (docs/hosts.md §7). A host may stay on an older
# one on purpose; the entry it publishes carries the version, so the pages can
# still prefer the newest host that speaks their protocol.
#
#     EMBER_REF=v2 bash deploy/deploy-fire-online.sh
REF="${EMBER_REF:-HEAD}"

# The tarball below is built with `git archive`, so ONLY COMMITTED WORK
# DEPLOYS. Refuse to run against a dirty tree rather than quietly shipping
# something other than what is in front of you — but only when the ref IS this
# tree's HEAD. Pinning a host to an older commit has nothing to do with what
# is being edited here.
if [ "$REF" = "HEAD" ]; then
    if ! git diff --quiet || ! git diff --cached --quiet; then
        echo "FAILED: working tree is dirty. This deploys the committed tree" >&2
        echo "        (git archive HEAD), so commit or stash first:" >&2
        git status --short >&2
        exit 1
    fi
fi
git rev-parse --verify -q "$REF^{commit}" >/dev/null \
    || { echo "FAILED: EMBER_REF='$REF' names no commit here." >&2; exit 1; }

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
# `git archive` rather than `tar` over the working tree. The old form excluded
# only target/ and .git, so ~250 MB of untracked artist assets rode to specht
# on every run — 351 MB to build a server that needs none of it. Limiting the
# pathspec to the manifests and crates gives 0.49 MB and an identical build.
# assets/models/fire is included so the `fire` client crate would also build
# there; it is 431 KB and saves a confusing failure later.
TARBALL="$(mktemp -t ember-fire-src-XXXX.tar.gz)"
git archive --format=tar.gz -o "$TARBALL" "$REF" \
    Cargo.toml Cargo.lock crates/ assets/models/fire/
echo "   $(du -h "$TARBALL" | cut -f1) of committed source"
scp -o BatchMode=yes "$TARBALL" "$REMOTE":ember-fire-src.tar.gz
rm -f "$TARBALL"
ssh -o BatchMode=yes "$REMOTE" \
    "rm -rf ~/$REMOTE_DIR && mkdir ~/$REMOTE_DIR && tar xzf ~/ember-fire-src.tar.gz -C ~/$REMOTE_DIR"

echo "== resolving this host's name =="
# Generated on the MACHINE and kept there (docs/hosts.md §6), so both games
# deployed to the same box land on the same entry rather than two. host-name.sh
# is piped in because the host has no checkout; a local EMBER_HOST_NAME still
# wins, through this one call's environment.
HOST_NAME="$(ssh -o BatchMode=yes "$REMOTE" \
    "EMBER_HOST_NAME='${EMBER_HOST_NAME:-}' bash -s" < "$REPO_DIR/deploy/host-name.sh")"
HOST_NAME="$(printf '%s' "$HOST_NAME" | tr -d '[:space:]')"
if ! printf '%s' "$HOST_NAME" | grep -qE '^[a-z0-9-]{3,32}$'; then
    echo "FAILED: '$REMOTE' produced no usable host name ('$HOST_NAME')." >&2
    exit 1
fi
echo "   $REMOTE publishes as '$HOST_NAME'"

echo "== building fire-server (toolbox: ember-build) =="
# Stamped at BUILD time: the server crate's build.rs reads these through
# option_env! so the binary can report its own build in its Welcome.
ssh -o BatchMode=yes "$REMOTE" \
    "toolbox run -c ember-build bash -lc 'source ~/.cargo/env && cd ~/$REMOTE_DIR && EMBER_BUILD_VERSION=$VERSION EMBER_BUILD_COMMIT=$COMMIT cargo build --release -p fire-server'"

echo "== checking nobody is mid-race =="
# A redeploy kicks everyone off. That is not hypothetical: a watchdog test on
# 2026-09-01 restarted the arena 72 seconds after two people had joined a
# lobby, and they lost the game they were playing. The scripts had no idea
# anyone was there.
#
# Three outcomes, and the distinction matters:
#   0  healthy and empty        -> go
#   2  healthy and OCCUPIED     -> refuse, unless forced
#   1  unreachable/unhealthy    -> GO. A dead server has no players to
#                                  disturb, and that is exactly when a
#                                  redeploy is most needed. Refusing here
#                                  would turn an outage into a deadlock.
set +e
ssh -o BatchMode=yes "$REMOTE" \
    "toolbox run -c ember-build bash -lc 'source ~/.cargo/env && cd ~/$REMOTE_DIR && cargo run --release -q -p fire-server --example probe -- ws://$BIND --require-empty'"
OCCUPANCY=$?
set -e
if [ "$OCCUPANCY" = "2" ]; then
    if [ -n "${EMBER_FORCE:-}" ]; then
        echo "   players are in game; EMBER_FORCE is set, continuing anyway"
    else
        echo "FAILED: people are playing on the current server right now." >&2
        echo "        Redeploying would disconnect them mid-race." >&2
        echo "        Wait for the lobby to empty, or override deliberately:" >&2
        echo "            EMBER_FORCE=1 bash deploy/deploy-fire-online.sh" >&2
        exit 1
    fi
fi

echo "== restarting fire-server =="
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
if ssh -o BatchMode=yes "$REMOTE" \
        'systemctl --user is-enabled ember-fire.service' >/dev/null 2>&1; then
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
    echo "   ember-fire.service is enabled; trying systemd"
    if ssh -o BatchMode=yes "$REMOTE" \
            'systemctl --user restart ember-fire.service' >/dev/null 2>&1; then
        MANAGED=1
        echo "   systemd accepted the restart and owns the process"
    else
        echo "   systemd refused it; falling back to a direct launch"
    fi
fi

if [ -z "$MANAGED" ]; then
    echo "   launching directly"
    # Only our own account's process, and only one matching our own install
    # path. The old shared form was `pkill -u <hardcoded>`: run from the wrong
    # account it matched nothing, said nothing (the call is wrapped in
    # `|| true`), and the launch then failed to bind a port the previous
    # process still held — a silent outage that looked like a good deploy.
    ssh -o BatchMode=yes "$REMOTE" \
        "pkill -u \"\$(id -un)\" -f \"\$HOME/$REMOTE_DIR/target/release/fire-serve[r]\" 2>/dev/null; true"
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
        echo "        cannot be stopped from here." >&2
        exit 1
    fi

    # EMBER_HOST_NAME, never a `--name` flag: a host pinned to an older commit
    # runs a binary that has never heard of the flag, and an unknown flag is a
    # crash loop where an unknown environment variable is ignored.
    ssh -o BatchMode=yes -f "$REMOTE" \
        "EMBER_HOST_NAME=$HOST_NAME RUST_LOG=info nohup ~/$REMOTE_DIR/target/release/fire-server $BIND >> ~/fire-server.log 2>&1 &"
    sleep 2
    if ! ssh -o BatchMode=yes "$REMOTE" 'pgrep -u "$(id -un)" -f "fire-serve[r]" >/dev/null'; then
        echo "FAILED: fire-server is not running. Last log lines:" >&2
        ssh -o BatchMode=yes "$REMOTE" 'tail -20 ~/fire-server.log' >&2 || true
        exit 1
    fi
fi
ssh -o BatchMode=yes "$REMOTE" 'tail -1 ~/fire-server.log'

echo "== local health check (before exposing it) =="
# Prove the hub loop is alive on the loopback side first, so a failure here is
# unambiguously the server rather than the tunnel.
if ! ssh -o BatchMode=yes "$REMOTE" \
    "toolbox run -c ember-build bash -lc 'source ~/.cargo/env && cd ~/$REMOTE_DIR && cargo run --release -q -p fire-server --example probe -- ws://$BIND'"; then
    echo "FAILED: the server is listening but did not answer Hello." >&2
    ssh -o BatchMode=yes "$REMOTE" 'tail -20 ~/fire-server.log' >&2 || true
    exit 1
fi

echo "== restarting tunnel (fresh domain) =="
# Same split as the server. A quick tunnel mints a NEW random hostname every
# time it starts, so this restart is what forces the republish below — and it
# is also why an unattended systemd restart leaves the game healthy at an
# address server.json does not name. A NAMED tunnel would remove both.
if [ -n "$MANAGED" ]; then
    # The unit truncates its own log in ExecStartPre, so the grep below still
    # finds only the hostname from this run.
    ssh -o BatchMode=yes "$REMOTE" 'systemctl --user restart ember-fire-tunnel.service'
else
    # Anchored on this port so it can never match the arena's tunnel.
    ssh -o BatchMode=yes "$REMOTE" \
        "pkill -u \"\$(id -un)\" -f \"cloudflare[d] tunnel --url http://$BIND\" 2>/dev/null; true"
    ssh -o BatchMode=yes "$REMOTE" ': > ~/cloudflared-fire.log'
    ssh -o BatchMode=yes -f "$REMOTE" \
        "nohup ~/bin/cloudflared tunnel --url http://$BIND --no-autoupdate >> ~/cloudflared-fire.log 2>&1 &"
fi

echo "== waiting for the tunnel domain =="
TUNNEL=""
for _ in $(seq 1 30); do
    sleep 2
    TUNNEL=$(ssh -o BatchMode=yes "$REMOTE" \
        "grep -oE 'https://[a-z0-9-]+\\.trycloudflare\\.com' ~/cloudflared-fire.log | head -1" || true)
    [ -n "$TUNNEL" ] && break
done
if [ -z "$TUNNEL" ]; then
    echo "FAILED: no trycloudflare domain appeared; see ~/cloudflared-fire.log on the target host" >&2
    exit 1
fi
WS_URL="wss://${TUNNEL#https://}"
echo "tunnel domain: $TUNNEL  ->  $WS_URL"

echo "== health check THROUGH the public URL =="
# The old shared form accepted an HTTP 101 as proof of life. A 101 only says a
# connection thread completed the WebSocket handshake; arena-server was observed
# on the target host with its listener up and its hub loop dead, handing out 101s and
# closing immediately — and that check would have printed ONLINE. The probe
# sends Hello and requires Welcome, which only the hub thread can produce.
# Do not ask DNS for the hostname before Cloudflare has published it. A query
# that lands too early gets NXDOMAIN, and a resolver that caches that keeps
# serving it long after the record appears, so every later attempt fails
# against a poisoned cache while the tunnel is fine from anywhere else. Ten
# retries spaced 3 s was enough by luck until 2026-09-01, when it was not.
echo "== letting the tunnel hostname propagate =="
sleep 15

ok=""
for _ in $(seq 1 10); do
    if cargo run --release -q -p fire-server --example probe -- "$WS_URL"; then
        ok=1
        break
    fi
    sleep 3
done
if [ -z "$ok" ]; then
    echo "FAILED: the tunnel is up but the server never answered Hello through it." >&2
    echo "        Not publishing server.json — the page keeps its previous value." >&2
    exit 1
fi

echo "== publishing this host's entry to the address book =="
# Only after the probe passed: publishing first would point every player at a
# server we had not yet proved was alive.
#
# Fire's protocol number comes from the REF being deployed. Fire carries its
# own version in its own crate on purpose, so bumping one game's protocol
# never gates the other's join.
FIRE_PROTO="$(git show "$REF:crates/fire-core/src/proto.rs" \
    | grep -oE 'PROTO_VERSION: u16 = [0-9]+' | grep -oE '[0-9]+$')"
[ -n "$FIRE_PROTO" ] || { echo "FAILED: no PROTO_VERSION in $REF:crates/fire-core/src/proto.rs" >&2; exit 1; }

# Merge, never overwrite. publish-host.sh writes only this host's entry and
# only fire's two keys on it, so the arena running on the SAME box keeps its
# address — the inline python this replaced could not express that, and a
# second host deploying fire took the first one's entry out of the book.
#
# `--repo`, not a gh-pages worktree of this checkout: see the long note in
# deploy-pong-online.sh. The short version is that the worktree checked out a
# local gh-pages nothing ever fetches, so another writer's publish made the
# push fail; and the failure left the worktree registered, which wedged every
# later deploy of either game and the pages deploy too.
bash "$REPO_DIR/deploy/publish-host.sh" \
    --repo "$(git -C "$REPO_DIR" remote get-url origin)" --branch gh-pages \
    --name "$HOST_NAME" \
    --game fire --url "$WS_URL" --proto "$FIRE_PROTO" \
    --version "$VERSION" --commit "$COMMIT" \
    --by "$(id -un)@$REMOTE"

echo "== ONLINE: $HOST_NAME -> $WS_URL =="
echo "   the fire page picks it up from server.json on its next load"
