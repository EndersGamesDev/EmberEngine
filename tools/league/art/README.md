# League art preparation

This directory contains the mesh inspector and deterministic bakers that turn reviewed champion meshes and ground pictures into compact League GLBs, preview images, and manifests.

`bake_champion.py` emits each champion model with a JSON pivot-and-extents sidecar consumed by `crates/league/src/scene/art.rs`; `bake_surface.py` and the two environment passes embed tiling pictures for the v2 and v4 scenes, while `inspect_mesh.py` supplies orthographic review views before orientation is accepted.

Raw generator output and durable job records remain under `target` rather than becoming shipping assets, and generated manifests carry hashes and review provenance forward; coordinate, source-retention, and sidecar conventions are governed by `docs/asset-pipeline.md`, with release-specific choices recorded in the Ultimate League plans.
