# Repository tools

This directory collects offline asset preparation, release-specific validation, publishing automation, and the Rust helper crates used to maintain Ember; the top-level Python programs inspect or convert source meshes and assemble arena level, operator, hand, and viewmodel artifacts.

Game crates and the web release trees consume the generated GLB, JSON sidecar, texture, and publication outputs, while each named or versioned subdirectory records the narrower workflow that produced or proved a particular result.

Asset provenance, source retention, coordinate conversion, and sidecar requirements live in `docs/asset-pipeline.md`; command-line helper behavior lives in `tools/cli-common/README.md`, and release-specific constraints stay with the corresponding version directory.
