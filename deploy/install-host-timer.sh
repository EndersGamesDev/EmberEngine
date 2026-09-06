#!/usr/bin/env bash
# install-host-timer.sh - make a Linux host keep its ember servers current
# and alive by itself: a systemd timer runs deploy/host-tick.sh every two
# minutes (docs/hosts.md §8-9).
#
#   bash deploy/install-host-timer.sh              # on the host, as the host user
#   ssh <host> 'bash -s' < deploy/install-host-timer.sh
#
# Needs: a checkout at $EMBER_HOME/src (deploy/host.sh up makes one) and
# sudo for the two unit files. System units rather than `systemd --user`
# ones so nothing depends on linger or a user bus; they still run as this
# user. Idempotent: re-run after changing ~/.ember/host.env or EMBER_HOME.
#
# What the tick does with EMBER_REF=ci-passed: the host follows the branch
# pointer .github/workflows/ci.yml moves when a main commit's tests pass, and
# never main itself - a red run keeps the last good build running.
set -euo pipefail

USER_NAME="$(id -un)"
CONF="${EMBER_CONF_DIR:-$HOME/.ember}/host.env"
# shellcheck source=/dev/null
[ -f "$CONF" ] && . "$CONF"
EMBER_HOME="${EMBER_HOME:-$HOME/ember-host}"
TICK="$EMBER_HOME/src/deploy/host-tick.sh"
if [ ! -f "$TICK" ]; then
    echo "install-host-timer: $TICK not found; run deploy/host.sh up first (it clones the source)" >&2
    exit 1
fi

sudo tee /etc/systemd/system/ember-host.service >/dev/null <<UNIT
# Installed by deploy/install-host-timer.sh of EmberEngine.
[Unit]
Description=ember host - one tick: update when the ref moved, repair when something died
Documentation=https://github.com/EndersGamesDev/EmberEngine
After=network-online.target

[Service]
Type=oneshot
User=$USER_NAME
Group=$USER_NAME
WorkingDirectory=$EMBER_HOME
ExecStart=/usr/bin/bash $TICK
UNIT

sudo tee /etc/systemd/system/ember-host.timer >/dev/null <<UNIT
# Installed by deploy/install-host-timer.sh of EmberEngine.
[Unit]
Description=ember host - tick every two minutes

[Timer]
OnBootSec=2min
OnUnitActiveSec=2min
AccuracySec=10s

[Install]
WantedBy=timers.target
UNIT

sudo systemctl daemon-reload
sudo systemctl enable --now ember-host.timer
echo "ember-host.timer enabled: $TICK every 2 minutes as $USER_NAME (log: $EMBER_HOME/log/tick.log)"
systemctl list-timers ember-host.timer --no-pager | head -2
