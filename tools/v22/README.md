# Arena v22 verification

This directory served Arena v22's environment and weather release with a headless-browser smoke harness for the real web client, WebGL2 renderer, and private loopback Arena server.

`browser-environment-smoke.cjs` loads both shipping shooter maps without keyboard or pointer input and records isolated captures and results under `target/environment-captures`; it complements the engine GPU gate by pinning browser-visible sky, sunlight, cloud, wetness, particle, and map behavior.

The renderer contract, known limits, output location, and exact companion verification command live in `docs/environment-weather.md`.
