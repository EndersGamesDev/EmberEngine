use std::sync::Arc;

use ember_julibrot_present::{
    FrameReceipt, FrameState, HotSlot, PresentConfig, PresentError, PresentEvent, PresentEvents,
    PresentFacts, PresentHot, PresentMain, Presenter, Warp, WarpPlan, WarpValidation,
};
use ember_lab_heap::HeapPresentResources;

#[allow(clippy::type_complexity)]
type NewPresenter = fn(
    Arc<wgpu::Device>,
    Arc<wgpu::Queue>,
    HeapPresentResources,
    PresentConfig,
) -> Result<Presenter, PresentError>;

#[test]
fn app_facing_callable_surface_has_the_pinned_signatures() {
    let new: NewPresenter = Presenter::new;
    let set_main: fn(&mut Presenter, PresentMain) = Presenter::set_main;
    let write_hot: fn(&mut Presenter, HotSlot, PresentHot, WarpValidation, bool) =
        Presenter::write_hot;
    let submit_scene: fn(&mut Presenter, HotSlot, f64) -> Result<u64, PresentError> =
        Presenter::submit_scene;
    let frame: for<'a> fn(
        &mut Presenter,
        FrameState<'a>,
        HotSlot,
    ) -> Result<FrameReceipt, PresentError> = Presenter::frame;
    let poll: fn(&mut Presenter, f64) -> Vec<PresentEvent> = Presenter::poll;
    let poll_fixed: fn(&mut Presenter, f64) -> PresentEvents = Presenter::poll_fixed;
    let facts: fn(&Presenter) -> PresentFacts = Presenter::facts;
    let warp: fn(
        &ember_julibrot_present::SceneFrame,
        &ember_julibrot_present::Pose,
        &ember_julibrot_present::Pose,
        ember_julibrot_math::PrecisionMode,
        WarpValidation,
    ) -> WarpPlan = Warp::reproject;
    std::hint::black_box(new);
    std::hint::black_box(set_main);
    std::hint::black_box(write_hot);
    std::hint::black_box(submit_scene);
    std::hint::black_box(frame);
    std::hint::black_box(poll);
    std::hint::black_box(poll_fixed);
    std::hint::black_box(facts);
    std::hint::black_box(warp);
}

#[test]
fn pending_surface_contract_has_one_warp_completion_identity() {
    let measurement = ember_julibrot_present::SubmissionMeasurement {
        kind: ember_julibrot_present::SubmissionKind::Warp,
        id: 41,
        source_scene_id: Some(9),
        sample_class: ember_julibrot_present::SampleClass::Measured,
        precision_mode: "PictureFast",
        wall_ms: 2.5,
        fence_wait_ms: 1.5,
        polls: 3,
    };
    let event = PresentEvent::WarpCompleted { measurement };
    let PresentEvent::WarpCompleted { measurement } = event else {
        panic!("constructed event changed kind");
    };
    assert_eq!(measurement.id, 41);
    assert_eq!(
        measurement.kind,
        ember_julibrot_present::SubmissionKind::Warp
    );
}
