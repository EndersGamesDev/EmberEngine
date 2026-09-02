# Watchdogs

Two of them, deliberately split, because they fail for different reasons and only one of them can hold a GitHub credential.

## 1. On the game host — `install-watchdog.sh`

Installs `systemd --user` units for both servers and both tunnels, then `loginctl enable-linger` so the user manager starts at boot and survives logout. Without linger the units die with your last ssh session and never come back on reboot, which is the single most common way this setup silently breaks.

It fixes: the box reboots, or a server panics, and nothing restarts it. Before this, both games ran under bare `nohup` launched from an ssh session — a reboot took both offline until a human noticed and redeployed by hand.

It cannot fix: the published address. A Cloudflare **quick** tunnel mints a new random `*.trycloudflare.com` hostname every time it starts, so after an unattended restart the servers are perfectly healthy at an address `server.json` does not name. Every player still sees a dead game.

## 2. Off the host — `watchdog.sh`

Runs on the workstation, where the git credentials already are. Nothing has to put a GitHub token on the game host.

It probes the address the **pages are currently publishing**, not the server directly. That is deliberate: it is the thing a player actually depends on, and it goes red if the server, the tunnel, or `server.json` is wrong — including the address-drift case above. When it goes red the watchdog redeploys, which republishes the new hostname.

It also watches `origin/main`. When a commit lands, the running servers are stale, so it redeploys them.

It refuses to act on a dirty tree or a branch it cannot fast-forward, since either means a human is mid-change and redeploying would ship something nobody tested.

```
bash deploy/watchdog.sh            # loop, WATCHDOG_INTERVAL=300 by default
bash deploy/watchdog.sh --once     # one pass, for Task Scheduler or cron
```

## Watching more than one host

`EMBER_HOSTS` is a space-separated list of ssh names; it defaults to `EMBER_HOST`, which defaults to `specht`, so a single-host setup needs no change. Each host is handled on its own: its name is resolved on the machine with `host-name.sh`, its entry is found by that name in `hosts[]`, and only the addresses that entry actually carries are probed — the top-level `ws` and `fire_ws` are never used here, because they name whichever host is preferred and probing them would test one machine once per host in the list.

A host that needs work is redeployed with `EMBER_HOST=<its ssh name>`, and only the games whose address stopped answering. State is one file per host (`.watchdog-state-<ssh name>`), which is what stops one machine that keeps failing from holding every other machine at an old commit. The never-over-players rule is asked of the host being redeployed, so a full lobby on one box no longer defers a repair on another.

The rest of the model — what an entry contains, how a page picks between hosts, and how someone runs their own — is `docs/hosts.md`.

## How the deploys and the units coexist

There is no conflict to avoid: as of `c71118a` both deploy scripts **detect** which world they are in.

`systemctl --user is-enabled ember-fire.service` (or `ember-pong`) succeeds, so systemd owns the process: the deploy restarts through `systemctl --user restart`, checks liveness with `is-active`, and restarts the tunnel the same way. It fails, so nothing is installed: the original `pkill` + `nohup` path runs exactly as before.

This replaces the either/or an earlier version of this file recommended. Hard-switching the restart step to `systemctl` would have been wrong, because it creates a cycle on a fresh host: the units cannot start until the binaries exist, the binaries only exist once the deploy has run, and the deploy would have required the units. Detection removes any ordering requirement between installing units and deploying — which is exactly the situation a migration to a new box lands in.

One consequence worth knowing: the "is anyone else holding this port" guard runs **only** in the unmanaged branch. Under systemd the port is legitimately held by our own unit across a restart, so the guard would fire on every managed deploy as a false alarm. Under `nohup` it still does its job.

## A stable hostname would delete most of this

Every problem above that involves republishing exists only because quick tunnels have no stable name. A Cloudflare **named** tunnel keeps its hostname across restarts, which would make a reboot fully self-healing: systemd brings the server and tunnel back at the same address, `server.json` is still correct, and no republish is needed at all.

It needs a Cloudflare account, a domain in it, and `cloudflared tunnel login` — a browser consent flow, so it has to be done by a human once. After that the credentials file lives at `~/.cloudflared/<UUID>.json` on the host and the tunnel is `cloudflared tunnel run <name>`.
