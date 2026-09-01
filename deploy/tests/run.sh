#!/usr/bin/env bash
# Run every deploy test, each timed, and report a total.
#
#   bash deploy/tests/run.sh              # all of them
#   bash deploy/tests/run.sh syntax       # just the named ones
#
# Suites:
#   syntax        bash -n, usage lines, line endings — instant
#   publish-host  the address book's only writer, against temp files — seconds
#   ssh-deploys   both workstation deploys against PATH shims — seconds
#   host-loopback host.sh up/status/update/down for real on loopback; builds
#                 both servers, so minutes on a cold target directory
#
# Nothing here contacts a host, a tunnel or a network.
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
SUITES="${*:-syntax publish-host ssh-deploys host-loopback}"

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
