# Assets

This directory holds checked-in concept references, level data, runtime-ready glTF models and 8-bit textures; large artist source files remain outside Git.

Game clients either embed these files into their wasm bundle or load them in native-only paths, so file size, format and path are runtime contracts rather than incidental artwork details.

Read [`docs/asset-pipeline.md`](../docs/asset-pipeline.md) before changing an asset and obey the renderer constraints in [`CLAUDE.md`](../CLAUDE.md); each child README identifies its producer and consumer.
