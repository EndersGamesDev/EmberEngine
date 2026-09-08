# Web site

This directory is the source tree for ember's GitHub Pages site: the launcher, developer landing page, shared host selection, game catalogue, versioned game pages and browser labs.

Players depend on `index.html` and `games.json` for the active-lobby showcase, browser-local generated handles, version picker and one wasm bundle for the selected game or lab; lobby listing happens before any game bundle loads. Each live page depends on `deploy/deploy-pages.sh` to assemble its matching generated bindings and wasm binary.

The publication contract lives in [`deploy/deploy-pages.sh`](../deploy/deploy-pages.sh), host discovery lives in [`docs/hosts.md`](../docs/hosts.md), and version selection, frozen pages and release tags are governed by [`docs/versioning.md`](../docs/versioning.md).
