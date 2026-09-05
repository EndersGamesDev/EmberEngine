use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

use ember_julibrot_kernels::RefinementLevel;
use ember_julibrot_math::{
    ObjectAngles, Plane, Pose, PoseMap, PrecisionMode, ViewControls, screen_to_plane,
};
use ember_julibrot_present::{
    PaletteId, PresentEvent, PresentEvents, SampleClass, SceneFrame, SubmissionKind,
    SubmissionMeasurement, Warp, WarpValidation,
};

struct CountingAllocator;

thread_local! {
    static COUNT_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
}

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        COUNT_ALLOCATIONS.with(|enabled| {
            if enabled.get() {
                ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            }
        });
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        COUNT_ALLOCATIONS.with(|enabled| {
            if enabled.get() {
                ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            }
        });
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        COUNT_ALLOCATIONS.with(|enabled| {
            if enabled.get() {
                ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            }
        });
        unsafe { System.realloc(pointer, layout, new_size) }
    }
}

fn measured_allocations<T>(action: impl FnOnce() -> T) -> (T, usize) {
    COUNT_ALLOCATIONS.with(|enabled| enabled.set(false));
    ALLOCATIONS.store(0, Ordering::Relaxed);
    COUNT_ALLOCATIONS.with(|enabled| enabled.set(true));
    let output = action();
    COUNT_ALLOCATIONS.with(|enabled| enabled.set(false));
    (output, ALLOCATIONS.load(Ordering::Relaxed))
}

fn pose(displacement: [f64; 2]) -> Pose {
    let object = ObjectAngles::JULIA;
    let view = ViewControls::NEUTRAL;
    let extent = [960, 540];
    let map = screen_to_plane(
        &object,
        &view,
        40.0,
        extent[0],
        extent[1],
        f64::from(extent[0]) / f64::from(extent[1]),
    )
    .expect("the neutral fixture maps");
    Pose {
        epoch: 1,
        orbit_generation: 1,
        plane: Plane {
            basis_u: [1.0, 0.0, 0.0, 0.0],
            basis_v: [0.0, 1.0, 0.0, 0.0],
        },
        object,
        plane_origin: [0.0; 4],
        zoom_log2: 40.0,
        view,
        grid_width: extent[0],
        grid_height: extent[1],
        map: PoseMap::Mapped(map),
        centre_from_reference_px: displacement,
    }
}

const fn frame(pose: Pose) -> SceneFrame {
    SceneFrame {
        scene_id: 7,
        pose,
        palette: PaletteId::Classic,
        iteration_cap: 256,
        level: RefinementLevel::Final,
        extent: [pose.grid_width, pose.grid_height],
        texture_index: 1,
        centre_revision: 1,
        plane_origin_f64: pose.plane_origin,
        precision_mode: PrecisionMode::PictureFast.as_str(),
        measurement: SubmissionMeasurement {
            kind: SubmissionKind::Scene,
            id: 7,
            source_scene_id: None,
            sample_class: SampleClass::Measured,
            precision_mode: PrecisionMode::PictureFast.as_str(),
            wall_ms: 1.0,
            fence_wait_ms: 0.5,
            polls: 1,
        },
    }
}

#[test]
fn planning_the_full_error_corpus_allocates_nothing() {
    let from = pose([0.0; 2]);
    let to = pose([5.0, -3.0]);
    let retained = frame(from);
    let plan = || {
        Warp::reproject(
            &retained,
            &from,
            &to,
            PrecisionMode::PictureFast,
            WarpValidation::Measure,
        )
    };
    std::hint::black_box(plan());
    let (planned, allocations) = measured_allocations(|| std::hint::black_box(plan()));
    assert!(planned.approx_max_error_px.is_some());
    assert_eq!(allocations, 0, "reprojection planning allocated");
}

#[test]
fn fixed_poll_result_iteration_allocates_nothing() {
    let measurement = SubmissionMeasurement {
        kind: SubmissionKind::Warp,
        id: 9,
        source_scene_id: Some(7),
        sample_class: SampleClass::Measured,
        precision_mode: PrecisionMode::PictureFast.as_str(),
        wall_ms: 1.0,
        fence_wait_ms: 0.5,
        polls: 1,
    };
    let events = PresentEvents::new(
        Some(PresentEvent::WarpCompleted { measurement }),
        Some(PresentEvent::FenceRefused {
            kind: SubmissionKind::Scene,
            id: 10,
            reason: ember_julibrot_present::FenceRefusal::Cancelled,
            polls: 1,
            wall_ms: 2.0,
            precision_mode: PrecisionMode::PictureFast.as_str(),
        }),
    );
    let (count, allocations) = measured_allocations(|| events.into_iter().count());
    assert_eq!(count, 2);
    assert_eq!(allocations, 0, "fixed poll result iteration allocated");
}
