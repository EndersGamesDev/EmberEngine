# Exact camera

`ember-camera` owns exact, reversible camera state and navigation for a two-dimensional image plane embedded in an N-dimensional space.

The crate is engine-neutral and dependency-free; as a shared engine crate, it is part of the `ember` workspace series. Future consumers provide screen geometry and translate their own presentation controls at its public boundary.

The arithmetic and navigation contracts are documented in [`../../docs/camera.md`](../../docs/camera.md), while workspace versioning, folder, and contribution rules live in [`../../docs/versioning.md`](../../docs/versioning.md), [`../../docs/readmes.md`](../../docs/readmes.md), and [`../../CLAUDE.md`](../../CLAUDE.md).
