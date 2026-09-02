use std::sync::Arc;

use ember_julibrot_present::{
    FrameReceipt, FrameState, HotSlot, PresentConfig, PresentError, PresentEvent, PresentFacts,
    PresentHot, PresentMain, Presenter, Warp, WarpPlan,
};
use ember_lab_heap::HeapPresentResources;

#[test]
fn app_facing_callable_surface_has_the_pinned_signatures() {
    let _new: fn(
        Arc<wgpu::Device>,
        Arc<wgpu::Queue>,
        HeapPresentResources,
        PresentConfig,
    ) -> Result<Presenter, PresentError> = Presenter::new;
    let _set_main: fn(&mut Presenter, PresentMain) = Presenter::set_main;
    let _write_hot: fn(&mut Presenter, HotSlot, PresentHot) = Presenter::write_hot;
    let _submit_scene: fn(&mut Presenter, HotSlot, f64) -> Result<u64, PresentError> =
        Presenter::submit_scene;
    let _frame: for<'a> fn(
        &mut Presenter,
        FrameState<'a>,
        HotSlot,
    ) -> Result<FrameReceipt, PresentError> = Presenter::frame;
    let _poll: fn(&mut Presenter, f64) -> Vec<PresentEvent> = Presenter::poll;
    let _facts: fn(&Presenter) -> PresentFacts = Presenter::facts;
    let _warp: fn(
        &ember_julibrot_present::SceneFrame,
        &ember_julibrot_present::Pose,
        &ember_julibrot_present::Pose,
    ) -> WarpPlan = Warp::reproject;
}

#[test]
fn pending_surface_contract_has_one_warp_completion_identity() {
    let measurement = ember_julibrot_present::SubmissionMeasurement {
        kind: ember_julibrot_present::SubmissionKind::Warp,
        id: 41,
        source_scene_id: Some(9),
        sample_class: ember_julibrot_present::SampleClass::Measured,
        wall_ms: 2.5,
        fence_wait_ms: 1.5,
        polls: 3,
    };
    let event = PresentEvent::WarpCompleted { measurement };
    let PresentEvent::WarpCompleted { measurement } = event else {
        panic!("constructed event changed kind");
    };
    assert_eq!(measurement.id, 41);
    assert_eq!(measurement.kind, ember_julibrot_present::SubmissionKind::Warp);
}
