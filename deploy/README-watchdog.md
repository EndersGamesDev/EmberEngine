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

## The conflict you must not create

Once the systemd units own the processes, **do not also run the deploy scripts' own launch path**. Both bind the same ports. The deploy scripts `pkill` their own binary and relaunch it with `nohup`; systemd sees its child die, and restarts it — now two processes race for 7780/7781 and one of them loses the bind, which looks exactly like "the deploy failed" while the old build keeps serving.

Pick one of these before enabling the units:

- **Simplest:** leave the units disabled and keep deploying by hand. You get no reboot durability, which is what we had before.
- **Correct:** change the deploy scripts' restart step to `systemctl --user restart ember-pong` (and `ember-fire`) instead of `pkill` + `nohup`, and let the units own the lifecycle. The tunnel restart, and therefore the republish, still has to happen in the deploy.

The second is the right end state. It is not done yet — the units are written and installed but left **stopped**, so nothing conflicts today.

## A stable hostname would delete most of this

Every problem above that involves republishing exists only because quick tunnels have no stable name. A Cloudflare **named** tunnel keeps its hostname across restarts, which would make a reboot fully self-healing: systemd brings the server and tunnel back at the same address, `server.json` is still correct, and no republish is needed at all.

It needs a Cloudflare account, a domain in it, and `cloudflared tunnel login` — a browser consent flow, so it has to be done by a human once. After that the credentials file lives at `~/.cloudflared/<UUID>.json` on the host and the tunnel is `cloudflared tunnel run <name>`.
