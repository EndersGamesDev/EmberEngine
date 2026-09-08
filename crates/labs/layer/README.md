# Fragment-compute layer laboratory

`ember-lab-layer` is the standalone WebGL2 fragment-compute dialect and GPU-resident 120-cell prism demonstration used as the equal-work comparator for the descriptor-heap lab.

It exposes reusable kernel registration and dispatch plus exact prism geometry, while its wasm demo renders the fixed lattice through square output textures without heap handle indirection.

Julibrot depends on the later heap implementation, but the heap comparison imports this lab's kernel body and draw workload so allocation architecture changes can be measured without changing submitted geometry.

The fixed lattice, Mode C comparison, and browser evidence rules are recorded in [`../../../docs/gpu-heap-lattice.md`](../../../docs/gpu-heap-lattice.md).
