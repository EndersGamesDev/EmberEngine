#!/bin/bash
# Build kings-server and its probe example inside the claude-sdk WSL distro.
#
#   bash deploy/wsl/kings-build.sh <repo /mnt/c path> <build version> <build commit>
#
# Invoked by deploy/deploy-kings-online.sh as
#   MSYS_NO_PATHCONV=1 WSL_UTF8=1 wsl -d claude-sdk -- bash "/mnt/c/<repo>/deploy/wsl/kings-build.sh" ...
# Every command that runs inside the distro lives in a file like this one:
# a multi-command string through Git Bash -> wsl.exe -> `bash -lc` has hung
# on this machine, and a file has no quoting to get wrong.
#
# The version and commit are computed on the HOST from git and passed in, so
# distro git never has to read the /mnt/c checkout (it would refuse without
# safe.directory). They reach the binary through option_env! in the server's
# build.rs, which re-runs when either changes; the probe later checks the
# running server reports the same commit.
#
# Target dir is the distro-local shared warm dir, at idle priority, like every
# other build on this machine.
set -u
source "$HOME/.cargo/env"
export CARGO_TARGET_DIR="$HOME/targets/ember"

REPO="${1:?usage: kings-build.sh <repo /mnt/c path> <version> <commit>}"
VERSION="${2:?usage: kings-build.sh <repo /mnt/c path> <version> <commit>}"
COMMIT="${3:?usage: kings-build.sh <repo /mnt/c path> <version> <commit>}"

cd "$REPO" || { echo "kings-build: no such repo dir inside the distro: $REPO" >&2; exit 2; }

t0=$(date +%s)
EMBER_BUILD_VERSION="$VERSION" EMBER_BUILD_COMMIT="$COMMIT" \
    chrt --idle 0 ionice -c3 cargo build --release -p kings-server --bin kings-server --example probe
rc=$?
echo "kings-build: cargo build --release -p kings-server --bin kings-server --example probe -> exit $rc, wall $(( $(date +%s) - t0 ))s (stamp $VERSION $COMMIT)"
if [ "$rc" = 0 ]; then
    for bin in "$CARGO_TARGET_DIR/release/kings-server" "$CARGO_TARGET_DIR/release/examples/probe"; do
        if [ ! -x "$bin" ]; then
            echo "kings-build: cargo reported success but $bin is missing or not executable" >&2
            exit 1
        fi
    done
fi
exit "$rc"
