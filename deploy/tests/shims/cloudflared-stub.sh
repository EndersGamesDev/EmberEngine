#!/usr/bin/env bash
# A stand-in for `cloudflared tunnel --url http://127.0.0.1:<port>`.
#
# It prints an address for the port it was given and then sits there, which is
# all host.sh needs from a tunnel: one line on stdout carrying the public
# address, and a process that stays alive so the pid file means something.
# Pointing EMBER_TUNNEL_BIN at this is what lets the whole of `host.sh up` —
# build, start, probe through the "public" URL, publish — run on loopback with
# no Cloudflare account and no network.
#
# The address is a plain ws:// on the loopback port, which host.sh accepts
# verbatim precisely because EMBER_TUNNEL_BIN is not the default. A real
# cloudflared prints an https://…trycloudflare.com line instead.
set -euo pipefail

URL=""
while [ $# -gt 0 ]; do
    case "$1" in
        --url) URL="${2:-}"; shift 2 ;;
        *) shift ;;
    esac
done
[ -n "$URL" ] || { echo "cloudflared-stub: no --url" >&2; exit 2; }

PORT="${URL##*:}"
echo "cloudflared-stub: serving http://127.0.0.1:$PORT"
echo "ws://127.0.0.1:$PORT"

# exec, so the pid host.sh recorded IS the process that has to die on `down`.
# A `sleep` left as a child would outlive the kill and make the test's
# "no processes left" check a lie.
exec sleep 3600
