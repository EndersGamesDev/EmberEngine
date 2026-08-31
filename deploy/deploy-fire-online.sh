#!/usr/bin/env bash
# Bring Fire Racer online multiplayer up end to end:
#   1. build + (re)start fire-server on the target host
#   2. (re)start a Cloudflare quick tunnel in front of it — this mints a
#      fresh https://…trycloudflare.com domain on EVERY restart
#   3. publish the new domain to server.json on GitHub Pages as "fire_ws"
#   4. health-check by SPEAKING THE PROTOCOL through the public URL
#
# Run from Windows (git-bash): bash deploy/deploy-fire-online.sh
# Requires the wireproxy tunnel to $EMBER_HOST running locally (ssh specht works).
#
# Deliberately a separate script, port, tunnel and server.json key from the
# arena's. The two games speak different protocols with independent version
# numbers, so redeploying one must never be able to knock the other offline.
#
# ACCOUNTS. the host may carry more than one: the live arena runs as `ender`,
# deployed from a different workstation, and holds 127.0.0.1:7780. This script
# runs as whoever `ssh specht` lands as (today `end`) and touches nothing
# belonging to anyone else — see the pkill and the port guard below.
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

# The tarball below is built with `git archive`, so ONLY COMMITTED WORK
# DEPLOYS. Refuse to run against a dirty tree rather than quietly shipping
# something other than what is in front of you.
if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "FAILED: working tree is dirty. This deploys the committed tree" >&2
    echo "        (git archive HEAD), so commit or stash first:" >&2
    git status --short >&2
    exit 1
fi
echo "== deploying $(git rev-parse --short HEAD) =="

echo "== syncing source =="
# `git archive` rather than `tar` over the working tree. The old form excluded
# only target/ and .git, so ~250 MB of untracked artist assets rode to specht
# on every run — 351 MB to build a server that needs none of it. Limiting the
# pathspec to the manifests and crates gives 0.49 MB and an identical build.
# assets/models/fire is included so the `fire` client crate would also build
# there; it is 431 KB and saves a confusing failure later.
TARBALL="$(mktemp -t ember-fire-src-XXXX.tar.gz)"
git archive --format=tar.gz -o "$TARBALL" HEAD \
    Cargo.toml Cargo.lock crates/ assets/models/fire/
echo "   $(du -h "$TARBALL" | cut -f1) of committed source"
scp -o BatchMode=yes "$TARBALL" "$REMOTE":ember-fire-src.tar.gz
rm -f "$TARBALL"
ssh -o BatchMode=yes "$REMOTE" \
    "rm -rf ~/$REMOTE_DIR && mkdir ~/$REMOTE_DIR && tar xzf ~/ember-fire-src.tar.gz -C ~/$REMOTE_DIR"

echo "== building fire-server (toolbox: ember-build) =="
ssh -o BatchMode=yes "$REMOTE" \
    "toolbox run -c ember-build bash -lc 'source ~/.cargo/env && cd ~/$REMOTE_DIR && cargo build --release -p fire-server'"

echo "== stopping our own fire-server, if any =="
# Only our own account's process, and only one matching our own install path.
# The old shared form was `pkill -u <hardcoded>`: run from the wrong account
# it matched nothing, said nothing (the call is wrapped in `|| true`), and the
# launch then failed to bind a port the previous process still held — a silent
# outage that looked like a successful deploy.
ssh -o BatchMode=yes "$REMOTE" \
    "pkill -u \"\$(id -un)\" -f \"\$HOME/$REMOTE_DIR/target/release/fire-serve[r]\" 2>/dev/null; true"
sleep 1

echo "== checking nobody else holds $PORT =="
# If the port is held by another account there is nothing this script can do
# about it, and the launch below would fail to bind. Say so plainly instead of
# reporting success over a server that never started.
HOLDER="$(ssh -o BatchMode=yes "$REMOTE" \
    "ss -ltnp 'sport = :$PORT' 2>/dev/null | tail -n +2" || true)"
if [ -n "$HOLDER" ]; then
    echo "FAILED: something is already listening on $PORT:" >&2
    echo "  $HOLDER" >&2
    echo "        If it shows no process name it belongs to another account" >&2
    echo "        (the arena runs as 'ender' on 7780, deployed elsewhere)." >&2
    echo "        Pick a free port or stop it from the machine that owns it." >&2
    exit 1
fi

echo "== starting fire-server =="
ssh -o BatchMode=yes -f "$REMOTE" \
    "RUST_LOG=info nohup ~/$REMOTE_DIR/target/release/fire-server $BIND >> ~/fire-server.log 2>&1 &"
sleep 2
if ! ssh -o BatchMode=yes "$REMOTE" 'pgrep -u "$(id -un)" -f "fire-serve[r]" >/dev/null'; then
    echo "FAILED: fire-server is not running. Last log lines:" >&2
    ssh -o BatchMode=yes "$REMOTE" 'tail -20 ~/fire-server.log' >&2 || true
    exit 1
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
# Anchored on this port so it can never match the arena's tunnel.
ssh -o BatchMode=yes "$REMOTE" \
    "pkill -u \"\$(id -un)\" -f \"cloudflare[d] tunnel --url http://$BIND\" 2>/dev/null; true"
ssh -o BatchMode=yes "$REMOTE" ': > ~/cloudflared-fire.log'
ssh -o BatchMode=yes -f "$REMOTE" \
    "nohup ~/bin/cloudflared tunnel --url http://$BIND --no-autoupdate >> ~/cloudflared-fire.log 2>&1 &"

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
# connection thread completed the WebSocket handshake; pong-server was observed
# on the target host with its listener up and its hub loop dead, handing out 101s and
# closing immediately — and that check would have printed ONLINE. The probe
# sends Hello and requires Welcome, which only the hub thread can produce.
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

echo "== publishing server.json to GitHub Pages =="
# Only after the probe passed: publishing first would point every player at a
# server we had not yet proved was alive.
#
# Merge, never overwrite: server.json carries the arena's "ws" and "proto"
# alongside fire's keys. Clobbering it would take the arena offline.
PAGES_DIR="$(mktemp -d -t ember-pages-XXXX)"
git -C "$REPO_DIR" worktree add -q "$PAGES_DIR" gh-pages
python - "$PAGES_DIR/server.json" "$WS_URL" <<'EOF'
import json, os, sys, time
p, ws = sys.argv[1], sys.argv[2]
d = {}
if os.path.exists(p):
    try:
        d = json.load(open(p))
    except Exception:
        d = {}
d["fire_ws"] = ws
d["v"] = str(int(time.time()))
json.dump(d, open(p, "w"))
EOF
(
    cd "$PAGES_DIR"
    git add server.json
    if git diff --cached --quiet; then
        echo "server.json unchanged"
    else
        git commit -q -m "Point fire_ws at $WS_URL

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
        git push -q origin gh-pages
    fi
)
git -C "$REPO_DIR" worktree remove --force "$PAGES_DIR"

echo "== ONLINE: $WS_URL =="
echo "   the fire page picks it up from server.json on its next load"
