# GPU heap laboratory

`ember-lab-heap` is the WebGL2 descriptor-heap and fragment-compute experiment used to compare GPU-resident 120-cell lattice paths while holding their logical work and presentation fixed.

It provides reusable heap allocation, paged spans, generated kernel lowering, completion polling, and GPU execution to the Julibrot kernels and presenter as well as its own browser evidence page.

The lab distinguishes logical bytes, physical reservation, address and draw walls, runtime capability refusals, and measured completion instead of treating a selected lattice rung as delivered work.

The architecture, fixed workload, modes, ABI, and evidence protocol are defined in [`../../../docs/gpu-heap-lattice.md`](../../../docs/gpu-heap-lattice.md); the applicable device floor remains [`../../../docs/minimum-requirements.md`](../../../docs/minimum-requirements.md).
