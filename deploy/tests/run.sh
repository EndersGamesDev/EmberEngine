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
#   ship-host     ship-host.sh against PATH shims for ssh, scp and curl: the
#                 published-version read, the builder's command line, the copy
#                 list and check's exit codes — seconds
#   watchdog      what the off-host watchdog decides, against PATH shims and a
#                 real git origin — seconds
#   host-pids     host.sh's process control, against real `sleep` processes —
#                 instant
#   host-kings    host.sh's whole-game wiring against fake build products
#   host-prebuilt host.sh and bootstrap-host.sh in EMBER_PREBUILT mode, with
#                 git and cargo present as shims that must never be called
#   host-timer    generated unit lifecycle and isolated tick lock inheritance
#   republish-host workstation fetch/merge and unchanged no-push behaviour
#   host-loopback host.sh up/status/update/down for real on loopback; builds
#                 all four servers, so minutes on a cold target directory
#   changelog     CHANGELOG.md against web/games.json, the object store and
#                 the tags — instant; tag checks skip on a tagless checkout
#   workflows     workflow YAML and the branch/tag contract — instant
#   rulesets      repository-ruleset JSON payloads and policy — instant
#
# Nothing here contacts a host, a tunnel or a network.
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
SUITES="${*:-syntax changelog workflows rulesets pages publish-host ssh-deploys ship-host watchdog host-pids host-kings host-prebuilt host-timer republish-host host-loopback}"

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
