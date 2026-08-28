#!/usr/bin/env bash
# Deploy ember-server to specht. Run from Windows (git-bash):
#   bash deploy/deploy-specht.sh
#
# Requires: the specht wireproxy tunnel running locally (ssh specht must work).
# The server binds the WireGuard tunnel IP only — it is reachable by WG peers
# (10.72.0.0/24), never the public internet.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BIND="10.72.0.1:7777"
TARBALL="$(mktemp -t ember-src-XXXX.tar.gz)"

echo "== packaging =="
tar --exclude='ember/target' --exclude='ember/.git' -czf "$TARBALL" \
    -C "$(dirname "$REPO_DIR")" "$(basename "$REPO_DIR")"

echo "== uploading =="
scp -o BatchMode=yes "$TARBALL" specht:ember-src.tar.gz
rm -f "$TARBALL"
ssh -o BatchMode=yes specht \
    'rm -rf ~/ember-src && mkdir ~/ember-src && tar xzf ~/ember-src.tar.gz -C ~/ember-src --strip-components=1'

echo "== building (toolbox: ember-build) =="
ssh -o BatchMode=yes specht \
    'toolbox run -c ember-build bash -lc "source ~/.cargo/env && cd ~/ember-src && cargo build --release -p ember-server"'

echo "== restarting =="
# NOTE: kill and launch are separate ssh calls on purpose. In a combined call
# the launch command line contains the text the pkill pattern matches, so
# pkill kills its own shell before the launch runs.
ssh -o BatchMode=yes specht 'pkill -u ender -f "ember-serve[r]" 2>/dev/null; true'
ssh -o BatchMode=yes -f specht \
    "RUST_LOG=info nohup ~/ember-src/target/release/ember-server --bind $BIND >> ~/ember-server.log 2>&1 &"
sleep 2

echo "== verifying =="
ssh -o BatchMode=yes specht 'pgrep -af "ember-serve[r]" && ss -tln | grep 7777 && tail -2 ~/ember-server.log'
echo "== deployed =="
