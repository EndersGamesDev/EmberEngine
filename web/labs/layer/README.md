# Fragment-compute lattice lab

This directory contains the standalone WebGL2 page that keeps a fixed prism and edge topology GPU-resident, expands a mixed-radix five-dimensional lattice in generated fragments and projects it into the displayed scene.

GPU-architecture reviewers depend on the page as a visible experiment for layer-generated work and compact per-frame uploads.

The related design argument lives in [`docs/gpu-heap-lattice.md`](../../../docs/gpu-heap-lattice.md), with the supported device floor in [`docs/minimum-requirements.md`](../../../docs/minimum-requirements.md).
