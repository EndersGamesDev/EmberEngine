#!/usr/bin/env bash
# Bring pong online multiplayer up end to end:
#   1. build + (re)start pong-server on the target host (127.0.0.1:7778)
#   2. (re)start a Cloudflare quick tunnel in front of it — this mints a
#      fresh https://…trycloudflare.com domain on EVERY restart
#   3. publish the new domain to server.json on GitHub Pages so the web
#      page finds the current server
#   4. health-check through the public URL
#
# Run from Windows (git-bash): bash deploy/deploy-pong-online.sh
# Requires the wireproxy tunnel to $EMBER_HOST running locally (ssh specht works).
#
# ACCOUNTS. the host may carry more than one, and the arena has historically run
# as `ender`, deployed from a particular workstation. This script only ever
# stops processes belonging to whoever `ssh specht` lands as, and refuses to
# continue if the port is held by someone else. Run it from the machine whose
# account owns the running server, or it will tell you it cannot — which is
# the point: the previous version killed nothing, said nothing (the call was
# wrapped in `|| true`), then failed to bind, and reported a successful deploy
# over a server that had never started.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
# The target host. specht is being decommissioned; export EMBER_HOST to point
# this at its replacement without editing the script:
#
#     EMBER_HOST=newbox bash deploy/deploy-pong-online.sh
#
# It must be a name `ssh` already resolves and can log into without a prompt
# (BatchMode is on everywhere below), i.e. an entry in ~/.ssh/config with a key.
REMOTE="${EMBER_HOST:-specht}"

# NOTE: SELinux on the target host denies many ports (7778, 8890, 8891 tested); 7780
# is verified bindable for this account.
PORT=7780
BIND="127.0.0.1:$PORT"
cd "$REPO_DIR"

# `git archive` below deploys the COMMITTED tree, so refuse a dirty one rather
# than shipping something other than what is in front of you.
if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "FAILED: working tree is dirty; this deploys the committed tree." >&2
    git status --short >&2
    exit 1
fi
echo "== deploying $(git rev-parse --short HEAD) =="

echo "== syncing source =="
# `git archive`, not `tar` over the working tree. The old form excluded only
# target/ and .git, so ~250 MB of untracked artist assets rode to specht on
# every run — 351 MB to build a server that needs none of it. The manifests
# and crates alone are ~0.5 MB and build identically.
TARBALL="$(mktemp -t ember-src-XXXX.tar.gz)"
git archive --format=tar.gz -o "$TARBALL" HEAD Cargo.toml Cargo.lock crates/
echo "   $(du -h "$TARBALL" | cut -f1) of committed source"
scp -o BatchMode=yes "$TARBALL" "$REMOTE":ember-src.tar.gz
rm -f "$TARBALL"
ssh -o BatchMode=yes "$REMOTE" \
    'rm -rf ~/ember-src && mkdir ~/ember-src && tar xzf ~/ember-src.tar.gz -C ~/ember-src'

echo "== building pong-server (toolbox: ember-build) =="
ssh -o BatchMode=yes "$REMOTE" \
    'toolbox run -c ember-build bash -lc "source ~/.cargo/env && cd ~/ember-src && cargo build --release -p pong-server"'

echo "== restarting pong-server =="
# Kill and launch are separate ssh calls on purpose: in a combined call the
# launch text matches the pkill pattern and kills its own shell.
# Only our own account's processes. `pkill -u ender` run from any other
# account matched nothing and was swallowed by the trailing `true`.
ssh -o BatchMode=yes "$REMOTE" 'pkill -u "$(id -un)" -f "pong-serve[r]" 2>/dev/null; true'
sleep 1

echo "== checking nobody else holds $PORT =="
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
ssh -o BatchMode=yes -f "$REMOTE" \
    "RUST_LOG=info nohup ~/ember-src/target/release/pong-server --bind $BIND >> ~/pong-server.log 2>&1 &"
sleep 2
if ! ssh -o BatchMode=yes "$REMOTE" 'pgrep -u "$(id -un)" -f "pong-serve[r]" >/dev/null'; then
    echo "FAILED: pong-server is not running. Last log lines:" >&2
    ssh -o BatchMode=yes "$REMOTE" 'tail -20 ~/pong-server.log' >&2 || true
    exit 1
fi
ssh -o BatchMode=yes "$REMOTE" 'tail -1 ~/pong-server.log'

echo "== local health check (before exposing it) =="
# Prove the hub loop is alive on the loopback side first, so a failure at the
# public check below is unambiguously the tunnel rather than the server. Worth
# the extra minute on a host we have never deployed to before.
if ! ssh -o BatchMode=yes "$REMOTE" \
    "toolbox run -c ember-build bash -lc 'source ~/.cargo/env && cd ~/ember-src && cargo run --release -q -p pong-server --example wsbot -- ws://$BIND create local-healthcheck - healthcheck 6'"; then
    echo "FAILED: the server is listening but wsbot could not create a lobby on it." >&2
    ssh -o BatchMode=yes "$REMOTE" 'tail -20 ~/pong-server.log' >&2 || true
    exit 1
fi

echo "== restarting tunnel (fresh domain) =="
# Pattern must not match this command's own shell: "cloudflared.log" would
# match a bare "cloudflare[d]" regex, so anchor on " tunnel" and truncate
# the log in a separate call.
# Our own account, and anchored on THIS port. A bare "cloudflare[d] tunnel"
# pattern also matches Fire Racer's tunnel on 7781, so redeploying the arena
# would have silently taken the other game offline.
ssh -o BatchMode=yes "$REMOTE" \
    "pkill -u \"\$(id -un)\" -f \"cloudflare[d] tunnel --url http://$BIND\" 2>/dev/null; true"
ssh -o BatchMode=yes "$REMOTE" ': > ~/cloudflared.log'
ssh -o BatchMode=yes -f "$REMOTE" \
    "nohup ~/bin/cloudflared tunnel --url http://$BIND --no-autoupdate >> ~/cloudflared.log 2>&1 &"

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
# WebSocket handshake; pong-server was observed on the target host with its listener up
# and its hub loop dead, handing out 101s and closing immediately — and this
# check printed ONLINE over it. wsbot speaks the protocol: it creates a lobby
# and counts state updates, so it can only pass if the hub is actually
# simulating. Exit 0 = the online loop works.
ok=""
for _ in $(seq 1 5); do
    sleep 2
    if cargo run --release -q -p pong-server --example wsbot -- \
        "$WS_URL" create deploy-healthcheck - healthcheck 6; then
        ok=1
        break
    fi
done
if [ -z "$ok" ]; then
    echo "FAILED: the tunnel is up but wsbot could not create a lobby through it." >&2
    echo "        Not publishing server.json — the page keeps its previous value," >&2
    echo "        which is better than pointing every player at a server we have" >&2
    echo "        not proved is alive." >&2
    exit 1
fi

echo "== publishing server.json to GitHub Pages =="
# Only after the health check passed. This used to run BEFORE it, so a
# failed deploy still pointed every player at the new domain.
PAGES_DIR="$(mktemp -d -t ember-pages-XXXX)"
git -C "$REPO_DIR" worktree add "$PAGES_DIR" gh-pages
# Merge, never overwrite: server.json also carries "proto", written by
# deploy-pages.sh at ship time. Clobbering it loses the record of which
# protocol the live bundle speaks, so the next pages deploy has no baseline
# to compare against and warns instead of catching a bump.
python - "$PAGES_DIR/server.json" "$WS_URL" <<'EOF'
import json, os, sys, time
p, ws = sys.argv[1], sys.argv[2]
d = {}
if os.path.exists(p):
    try:
        d = json.load(open(p))
    except Exception:
        d = {}
d["ws"] = ws
d["v"] = str(int(time.time()))
json.dump(d, open(p, "w"))
EOF
(
    cd "$PAGES_DIR"
    git add server.json
    if git diff --cached --quiet; then
        echo "server.json unchanged"
    else
        git commit -q -m "Point server.json at $WS_URL

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
        git push -q origin gh-pages
    fi
)
git -C "$REPO_DIR" worktree remove --force "$PAGES_DIR"

echo "== ONLINE: $WS_URL (page picks it up from server.json) =="
