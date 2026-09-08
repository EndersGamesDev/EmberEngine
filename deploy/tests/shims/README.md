# Deployment test shims

This directory contains deterministic stand-ins for Cargo, Git, SSH, SCP, curl, wasm-bindgen, cloudflared, probes, servers and sleep used by the shell suites one level up.

Deployment tests depend on each shim's logged argument and controlled-output contract to exercise scripts without compilers, networks or live processes.

Keep shims minimal and test-only, preserve the environment variables documented by their calling suite, and follow [`deploy/tests/README.md`](../README.md) when changing a simulated boundary.
