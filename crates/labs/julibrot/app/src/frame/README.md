# Julibrot frame scheduling

This directory splits progressive frame orchestration into `schedule.rs`, a platform-neutral state machine, and `loop.rs`, the cross-crate integration that applies it to references, kernels, presentation, and browser work.

The public `RefinementSchedule` and `SceneMode` decide Preview, Interactive, and Final progression, manual versus automatic updates, refusal classification, retained-scene holds, and when an accepted warp can skip draft work.

The loop consumes worker arrivals and GPU completion events in a fixed cooperative turn so a displayed image, outstanding request, and next scheduled level cannot disagree about generation or pose.

The required refresh ordering and bounded-work argument live in [`../../../../../../docs/julibrot/app.md`](../../../../../../docs/julibrot/app.md), with presentation event semantics in [`../../../../../../docs/julibrot/present.md`](../../../../../../docs/julibrot/present.md).
