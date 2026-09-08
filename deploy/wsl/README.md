# Kings WSL host helpers

This directory contains build, launch, tunnel, probe and control helpers for the Four Kings server hosted inside the `claude-sdk` WSL distribution.

`deploy-kings-online.sh` and the Windows-side PowerShell launch path depend on these scripts to manage Kings without systemd and to prove both local and tunneled protocol responses.

Operational ownership remains with [`deploy-kings-online.sh`](../deploy-kings-online.sh), while host publication and verification rules live in [`docs/hosts.md`](../../docs/hosts.md) and [`CLAUDE.md`](../../CLAUDE.md).
