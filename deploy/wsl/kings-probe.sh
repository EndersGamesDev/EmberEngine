#!/bin/bash
# Speak the Four Kings protocol at a server, from inside the claude-sdk WSL
# distro, using the probe example built by kings-build.sh.
#
#   bash deploy/wsl/kings-probe.sh <ws url> [--expect-commit <sha>]
#
# The probe (crates/kings-server/examples/probe.rs) sends Hello and requires
# Welcome with the build's PROTO_VERSION, then creates and leaves a lobby;
# with --expect-commit it also requires Welcome.commit to equal the stamp of
# the binary just built, which catches a missed pkill leaving an older server
# on the port. Exit 0 or 1 with the reason on stderr. Run once on loopback
# (a failure there is the server) and then through the public wss URL (a
# failure there is the tunnel).
#
# The binary is run directly rather than through `cargo run` so that a probe
# never waits on the shared target dir's build lock.
set -u
source "$HOME/.cargo/env"
export CARGO_TARGET_DIR="$HOME/targets/ember"

PROBE="$CARGO_TARGET_DIR/release/examples/probe"
if [ ! -x "$PROBE" ]; then
    echo "kings-probe: $PROBE is missing; kings-build.sh builds it (cargo build --release -p kings-server --example probe)" >&2
    exit 1
fi
[ $# -ge 1 ] || { echo "usage: kings-probe.sh <ws url> [--expect-commit <sha>]" >&2; exit 2; }

t0=$(date +%s)
"$PROBE" "$@"
rc=$?
echo "kings-probe: probe $* -> exit $rc, wall $(( $(date +%s) - t0 ))s"
exit "$rc"
