# Deploy the arena and Fire Racer on sokol

## Why the live services move

The machines that ran the live services and baked their builds were decommissioned, and their returned names no longer identify those machines. The arena and Fire Racer therefore move to `sokol`, a rented pod that must be treated as replaceable: the repository, the address book and the account's SSH key are sufficient to rebuild it, and nothing stored only on the pod is precious.

## Deployment invariants

Hosts are named only by SSH alias in the repository. Network addresses, key paths, port forwards and ephemeral tunnel URLs stay outside tracked files; a host may deliberately run an older commit, and every published entry must describe the ref actually built.

## Defaults and current-host documentation

The workstation deploys, unit installer and off-host watchdog default to `sokol`, and the current-host documentation names its role while preserving old machine names only where they are historical measurements.

## Lifecycle on a pod without systemd

Sokol has systemd binaries but no running user manager. The unit installer must reject that state before writing anything, the workstation deploys must fall through to their direct launch, and `host.sh` remains the on-pod lifecycle because its `nohup` processes and PID-plus-start-time identity do not depend on systemd.

## Reproducible bootstrap

Bootstrap is a separate idempotent `deploy/bootstrap-host.sh`, rather than a `host.sh` verb, because preparing machine dependencies and operating an already prepared service have different failure and repetition boundaries. The script can be rerun safely before any lifecycle command, reports each missing prerequisite directly, installs the architecture-checked release-pinned cloudflared only after a pinned SHA-256 match, and never replaces an existing `host.env`.

## Rebuilding a replacement pod

The design-of-record sequence is estate-provided account and key, repository clone into the `host.sh` layout, bootstrap, then `host.sh up`; current one-off source and publish choices remain environment overrides so the checked-in defaults stay reusable.

## Cache and identity

The checkout, build products, logs, PID files and changing tunnel URL under `~/ember-host` are cache. `~/.ember/host-name` is identity but is deterministically regenerated if lost, while the default `~/.ember/host.env` is recreated by bootstrap and deliberate overrides belong in the invocation or provisioning source.

## Live deployment proof

The lane report, not a tracked file, carries the live tunnel URLs and generated host entry. It also records clone, bootstrap, build, server start, tunnel start and probe walls; both loopback ports and public protocol paths must answer, `status` must match reality, and `update` must be a no-op before the services are handed over running.

## Verification

Every touched shell script receives a sokol-side syntax check, shellcheck runs when installed, and the linter tests plus the complete workspace build run through the server's report wrapper against the shared gate target.

## Follow-ups

A pod-local restart-on-death supervisor and an off-host watchdog that does not depend on the workstation remain explicit backlog work.
