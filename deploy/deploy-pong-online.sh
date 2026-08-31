#!/usr/bin/env bash
# Bring pong online multiplayer up end to end:
#   1. build + (re)start pong-server on specht (127.0.0.1:7778)
#   2. (re)start a Cloudflare quick tunnel in front of it — this mints a
#      fresh https://…trycloudflare.com domain on EVERY restart
#   3. publish the new domain to server.json on GitHub Pages so the web
#      page finds the current server
#   4. health-check through the public URL
#
# Run from Windows (git-bash): bash deploy/deploy-pong-online.sh
# Requires the specht wireproxy tunnel running locally (ssh specht works).
#
# ACCOUNTS. specht carries more than one, and the arena has historically run
# as `ender`, deployed from a particular workstation. This script only ever
# stops processes belonging to whoever `ssh specht` lands as, and refuses to
# continue if the port is held by someone else. Run it from the machine whose
# account owns the running server, or it will tell you it cannot — which is
# the point: the previous version killed nothing, said nothing (the call was
# wrapped in `|| true`), then failed to bind, and reported a successful deploy
# over a server that had never started.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
# NOTE: SELinux on specht denies many ports (7778, 8890, 8891 tested); 7780
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
scp -o BatchMode=yes "$TARBALL" specht:ember-src.tar.gz
rm -f "$TARBALL"
ssh -o BatchMode=yes specht \
    'rm -rf ~/ember-src && mkdir ~/ember-src && tar xzf ~/ember-src.tar.gz -C ~/ember-src'

echo "== building pong-server (toolbox: ember-build) =="
ssh -o BatchMode=yes specht \
    'toolbox run -c ember-build bash -lc "source ~/.cargo/env && cd ~/ember-src && cargo build --release -p pong-server"'

echo "== restarting pong-server =="
# Kill and launch are separate ssh calls on purpose: in a combined call the
# launch text matches the pkill pattern and kills its own shell.
# Only our own account's processes. `pkill -u ender` run from any other
# account matched nothing and was swallowed by the trailing `true`.
ssh -o BatchMode=yes specht 'pkill -u "$(id -un)" -f "pong-serve[r]" 2>/dev/null; true'
sleep 1

echo "== checking nobody else holds $PORT =="
HOLDER="$(ssh -o BatchMode=yes specht \
    "ss -ltnp 'sport = :$PORT' 2>/dev/null | tail -n +2" || true)"
if [ -n "$HOLDER" ]; then
    echo "FAILED: something is already listening on $PORT:" >&2
    echo "  $HOLDER" >&2
    echo "        A row with no process name belongs to another account and" >&2
    echo "        cannot be stopped from here. Deploy from the machine whose" >&2
    echo "        account owns it." >&2
    exit 1
fi
ssh -o BatchMode=yes -f specht \
    "RUST_LOG=info nohup ~/ember-src/target/release/pong-server --bind $BIND >> ~/pong-server.log 2>&1 &"
sleep 2
ssh -o BatchMode=yes specht 'pgrep -af "pong-serve[r]" && tail -1 ~/pong-server.log'

echo "== restarting tunnel (fresh domain) =="
# Pattern must not match this command's own shell: "cloudflared.log" would
# match a bare "cloudflare[d]" regex, so anchor on " tunnel" and truncate
# the log in a separate call.
# Our own account, and anchored on THIS port. A bare "cloudflare[d] tunnel"
# pattern also matches Fire Racer's tunnel on 7781, so redeploying the arena
# would have silently taken the other game offline.
ssh -o BatchMode=yes specht \
    "pkill -u \"\$(id -un)\" -f \"cloudflare[d] tunnel --url http://$BIND\" 2>/dev/null; true"
ssh -o BatchMode=yes specht ': > ~/cloudflared.log'
ssh -o BatchMode=yes -f specht \
    "nohup ~/bin/cloudflared tunnel --url http://$BIND --no-autoupdate >> ~/cloudflared.log 2>&1 &"

echo "== waiting for the tunnel domain =="
HOST=""
for _ in $(seq 1 30); do
    sleep 2
    HOST=$(ssh -o BatchMode=yes specht \
        "grep -oE 'https://[a-z0-9-]+\\.trycloudflare\\.com' ~/cloudflared.log | head -1" || true)
    [ -n "$HOST" ] && break
done
if [ -z "$HOST" ]; then
    echo "FAILED: no trycloudflare domain appeared; see ~/cloudflared.log on specht" >&2
    exit 1
fi
WS_URL="wss://${HOST#https://}"
echo "tunnel domain: $HOST  ->  $WS_URL"

echo "== publishing server.json to GitHub Pages =="
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

echo "== health check through the public URL =="
# NOT a bare HTTP 101. A 101 only says a connection thread completed the
# WebSocket handshake; pong-server was observed on specht with its listener up
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
if [ -n "$ok" ]; then
    echo "== ONLINE: $WS_URL (page picks it up from server.json) =="
else
    echo "WARNING: health check did not reach 101 yet (tunnel may still be propagating)" >&2
fi
