#!/usr/bin/env bash
# Install the ON-HOST watchdog: systemd --user units so both game servers and
# both tunnels come back by themselves after a reboot or a crash.
#
#   EMBER_HOST=specht bash deploy/install-watchdog.sh
#
# Why user units and not system units: the deploys already run entirely as an
# unprivileged account, the binaries live under $HOME, and nothing here needs
# root. `loginctl enable-linger` is the part people forget — without it a user
# manager is torn down when the last ssh session closes, so the units would
# stop the moment you log out and would never start at boot.
#
# This handles the SERVERS coming back. It cannot handle the published address
# coming back: a Cloudflare QUICK tunnel mints a new random hostname on every
# restart, so after an unattended reboot the servers are healthy at an address
# nobody knows. That half is deploy/watchdog.sh, which runs where the git
# credentials already are. The two are deliberately separate.
set -euo pipefail

REMOTE="${EMBER_HOST:-specht}"
SSH=(ssh -o BatchMode=yes "$REMOTE")
REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"

echo "== resolving this host's name =="
# The units must start the servers with the SAME name the deploys publish
# under, or a reboot would bring the games back under a second entry and the
# book would carry a host that no longer exists. Resolved on the machine
# itself (docs/hosts.md §6), which is where the name is kept.
HOST_NAME="$("${SSH[@]}" "EMBER_HOST_NAME='${EMBER_HOST_NAME:-}' bash -s" \
    < "$REPO_DIR/deploy/host-name.sh" | tr -d '[:space:]')"
if ! printf '%s' "$HOST_NAME" | grep -qE '^[a-z0-9-]{3,32}$'; then
    echo "FAILED: '$REMOTE' produced no usable host name ('$HOST_NAME')." >&2
    exit 1
fi
echo "   $REMOTE runs as '$HOST_NAME'"

echo "== installing user units on $REMOTE =="
# The name is passed in rather than resolved on the far side, so the units and
# the deploys cannot disagree about it even if ~/.ember/host-name is lost
# later. Everything else in the heredoc is quoted and expands on the host.
"${SSH[@]}" "EMBER_HOST_NAME='$HOST_NAME' bash -s" <<'REMOTE_SCRIPT'
set -euo pipefail
mkdir -p ~/.config/systemd/user

emit_server() {
    # $1 = short name (pong|fire)  $2 = source dir  $3 = binary  $4 = bind args
    cat > ~/.config/systemd/user/ember-"$1".service <<UNIT
[Unit]
Description=ember $1 game server
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
Environment=RUST_LOG=info
Environment=EMBER_HOST_NAME=$EMBER_HOST_NAME
ExecStart=%h/$2/target/release/$3 $4
Restart=always
RestartSec=3
StandardOutput=append:%h/$1-server.log
StandardError=append:%h/$1-server.log

[Install]
WantedBy=default.target
UNIT
}

emit_tunnel() {
    # $1 = short name  $2 = port
    # The log is truncated on every start so whoever reads the minted hostname
    # out of it cannot pick up the previous run's dead domain.
    cat > ~/.config/systemd/user/ember-"$1"-tunnel.service <<UNIT
[Unit]
Description=ember $1 cloudflare tunnel
After=ember-$1.service
Requires=ember-$1.service

[Service]
Type=simple
ExecStartPre=/usr/bin/truncate -s 0 %h/cloudflared-$1.log
ExecStart=%h/bin/cloudflared tunnel --url http://127.0.0.1:$2 --no-autoupdate
Restart=always
RestartSec=5
StandardOutput=append:%h/cloudflared-$1.log
StandardError=append:%h/cloudflared-$1.log

[Install]
WantedBy=default.target
UNIT
}

emit_server pong ember-src      pong-server "--bind 127.0.0.1:7780"
emit_server fire ember-src-fire fire-server "127.0.0.1:7781"
emit_tunnel pong 7780
emit_tunnel fire 7781

systemctl --user daemon-reload
for u in ember-pong ember-pong-tunnel ember-fire ember-fire-tunnel; do
    systemctl --user enable "$u".service >/dev/null
done
echo "units installed and enabled"
REMOTE_SCRIPT

echo "== enabling linger (units must survive logout and start at boot) =="
"${SSH[@]}" 'loginctl enable-linger "$(id -un)" && loginctl show-user "$(id -un)" -p Linger'

echo
echo "Installed but NOT started — the binaries must exist first."
echo "Run the two deploys, then:"
echo "  ssh $REMOTE 'systemctl --user start ember-pong ember-pong-tunnel ember-fire ember-fire-tunnel'"
echo
echo "NOTE: once these units own the processes, the deploy scripts' own"
echo "pkill+nohup launch and systemd will fight over the same ports."
echo "See deploy/README-watchdog.md before enabling both."
