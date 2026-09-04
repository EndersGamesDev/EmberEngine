#!/usr/bin/env bash
# Run every deploy test, each timed, and report a total.
#
#   bash deploy/tests/run.sh              # all of them
#   bash deploy/tests/run.sh syntax       # just the named ones
#
# Suites:
#   syntax        bash -n, usage lines, line endings — instant
#   pages         pages assembly and prebuilt bundles against PATH shims — seconds
#   publish-host  the address book's only writer, against temp files — seconds
#   ssh-deploys   both workstation deploys against PATH shims — seconds
#   watchdog      what the off-host watchdog decides, against PATH shims and a
#                 real git origin — seconds
#   host-pids     host.sh's process control, against real `sleep` processes —
#                 instant
#   host-loopback host.sh up/status/update/down for real on loopback; builds
#                 both servers, so minutes on a cold target directory
#
# Nothing here contacts a host, a tunnel or a network.
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
SUITES="${*:-syntax pages publish-host ssh-deploys watchdog host-pids host-loopback}"

T0="$(date +%s)"
failed=""
for s in $SUITES; do
    f="$HERE/test-$s.sh"
    if [ ! -f "$f" ]; then
        echo "RUN: no such suite: $s"
        failed="$failed $s"
        continue
    fi
    echo
    echo "######## $s ########"
    t="$(date +%s)"
    bash "$f"
    rc=$?
    echo "######## $s: exit=$rc in $(( $(date +%s) - t ))s ########"
    [ "$rc" -eq 0 ] || failed="$failed $s"
done

echo
echo "RUN TOTAL $(( $(date +%s) - T0 ))s failed=[${failed# }]"
[ -z "$failed" ]
