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
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
# NOTE: SELinux on specht denies many ports (7778, 8890, 8891 tested); 7780
# is verified bindable for this account.
BIND="127.0.0.1:7780"

echo "== syncing source =="
TARBALL="$(mktemp -t ember-src-XXXX.tar.gz)"
tar --exclude='ember/target' --exclude='ember/.git' -czf "$TARBALL" \
    -C "$(dirname "$REPO_DIR")" "$(basename "$REPO_DIR")"
scp -o BatchMode=yes "$TARBALL" specht:ember-src.tar.gz
rm -f "$TARBALL"
ssh -o BatchMode=yes specht \
    'rm -rf ~/ember-src && mkdir ~/ember-src && tar xzf ~/ember-src.tar.gz -C ~/ember-src --strip-components=1'

echo "== building pong-server (toolbox: ember-build) =="
ssh -o BatchMode=yes specht \
    'toolbox run -c ember-build bash -lc "source ~/.cargo/env && cd ~/ember-src && cargo build --release -p pong-server"'

echo "== restarting pong-server =="
# Kill and launch are separate ssh calls on purpose: in a combined call the
# launch text matches the pkill pattern and kills its own shell.
ssh -o BatchMode=yes specht 'pkill -u ender -f "pong-serve[r]" 2>/dev/null; true'
ssh -o BatchMode=yes -f specht \
    "RUST_LOG=info nohup ~/ember-src/target/release/pong-server --bind $BIND >> ~/pong-server.log 2>&1 &"
sleep 2
ssh -o BatchMode=yes specht 'pgrep -af "pong-serve[r]" && tail -1 ~/pong-server.log'

echo "== restarting tunnel (fresh domain) =="
# Pattern must not match this command's own shell: "cloudflared.log" would
# match a bare "cloudflare[d]" regex, so anchor on " tunnel" and truncate
# the log in a separate call.
ssh -o BatchMode=yes specht 'pkill -u ender -f "cloudflare[d] tunnel" 2>/dev/null; true'
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
ok=""
for _ in $(seq 1 15); do
    sleep 2
    code=$(curl -s -o /dev/null -w '%{http_code}' \
        -H 'Connection: Upgrade' -H 'Upgrade: websocket' \
        -H 'Sec-WebSocket-Version: 13' -H 'Sec-WebSocket-Key: SGVsbG8sIHdvcmxkIQ==' \
        "$HOST" || true)
    if [ "$code" = "101" ]; then ok=1; break; fi
done
if [ -n "$ok" ]; then
    echo "== ONLINE: $WS_URL (page picks it up from server.json) =="
else
    echo "WARNING: health check did not reach 101 yet (tunnel may still be propagating)" >&2
fi
