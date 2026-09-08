# Julibrot presenter device module

`device.rs` defines the concrete `Presenter` and partitions its implementation into resource creation, submission, polling, readback, census, shading, redraw, and warp modules below this directory.

It owns scene and depth targets, index buffers, value textures, bind layouts, HOT rings, completion fences, exposure state, and the fixed-capacity facts returned to the application.

The public `FrameReadback` route is re-exported through `gpu.rs`; no other presentation module reaches around this device owner to issue wgpu work.

The single-owner and fixed-event-order requirements are documented in [`../../../../../../docs/julibrot/present.md`](../../../../../../docs/julibrot/present.md).
