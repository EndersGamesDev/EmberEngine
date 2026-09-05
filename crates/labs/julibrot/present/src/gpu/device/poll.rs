use crate::fence::FenceDecision;
use crate::state::SceneCompletion;
use crate::{PresentEvent, PresentFacts, PresentStatus, RefinementLevel, SubmissionKind};

use super::census::{census_if_ready, mapped_census, observe_fence, take_glitch_readback_result};
use super::{Presenter, SceneCensus};

impl Presenter {
    /// Observes every pending fence once without waiting and returns terminal events.
    #[must_use]
    pub fn poll(&mut self, now_ms: f64) -> Vec<PresentEvent> {
        if self.scene_fence.is_some() || self.warp_fence.is_some() {
            self.device.poll(wgpu::Maintain::Poll);
        }
        let mut events = Vec::with_capacity(2);
        if let Some(event) = self.poll_scene(now_ms) {
            events.push(event);
        }
        if let Some(event) = self.poll_warp(now_ms) {
            events.push(event);
        }
        events
    }

    /// Returns the latest immutable facts without polling or submitting.
    #[must_use]
    pub fn facts(&self) -> PresentFacts {
        self.facts.clone()
    }

    fn poll_scene(&mut self, now_ms: f64) -> Option<PresentEvent> {
        let pending = self.scene_fence.as_mut()?;
        let decision = observe_fence(pending, now_ms);
        match decision {
            FenceDecision::Pending => None,
            FenceDecision::Complete(measurement) => {
                let readback_result = take_glitch_readback_result(pending);
                let census =
                    census_if_ready(readback_result, || mapped_census(&self.gpu.glitch_readback));
                if pending.glitch_readback.is_some() {
                    self.gpu.glitch_readback.buffer.unmap();
                }
                self.gpu.scene_fence.unmap();
                self.scene_fence = None;
                self.scene_samples.completed();
                self.facts.in_flight_scene_id = None;
                self.facts.last_scene = Some(measurement);
                let preserve_accepted_best = self
                    .latest_hot_slot
                    .and_then(|slot| {
                        let index = slot.index() as usize;
                        self.hot_warp_source[index]
                            .frame(self.ledger.retained(), self.ledger.held())
                    })
                    .is_some();
                match self
                    .ledger
                    .complete_preserving_accepted_best(measurement, preserve_accepted_best)?
                {
                    SceneCompletion::Promoted(frame) => {
                        self.publish_promoted(&frame, census.map(SceneCensus::glitch_pixel_count));
                        Some(PresentEvent::SceneCompleted {
                            frame,
                            reference_sample: census.and_then(SceneCensus::reference_sample),
                        })
                    }
                    SceneCompletion::KeptBest(frame) => Some(PresentEvent::SceneCompleted {
                        frame,
                        reference_sample: census.and_then(SceneCensus::reference_sample),
                    }),
                    SceneCompletion::Dropped {
                        pending,
                        reason,
                        measurement,
                    } => Some(PresentEvent::SceneDropped {
                        scene_id: pending.scene_id,
                        orbit_generation: pending.pose.orbit_generation,
                        reason,
                        measurement,
                    }),
                }
            }
            FenceDecision::Refused {
                reason,
                polls,
                wall_ms,
                precision_mode,
            } => {
                let id = pending.ledger.id();
                if pending.glitch_readback.is_some() {
                    self.gpu.glitch_readback.buffer.unmap();
                }
                self.gpu.scene_fence.unmap();
                self.scene_fence = None;
                self.ledger.cancel_pending();
                self.facts.in_flight_scene_id = None;
                Some(PresentEvent::FenceRefused {
                    kind: SubmissionKind::Scene,
                    id,
                    reason,
                    polls,
                    wall_ms,
                    precision_mode,
                })
            }
        }
    }

    fn poll_warp(&mut self, now_ms: f64) -> Option<PresentEvent> {
        let pending = self.warp_fence.as_mut()?;
        let decision = observe_fence(pending, now_ms);
        match decision {
            FenceDecision::Pending => None,
            FenceDecision::Complete(measurement) => {
                self.gpu.warp_fence.unmap();
                self.warp_fence = None;
                self.warp_samples.completed();
                self.facts.last_warp = Some(measurement);
                if let Some(source) = measurement.source_scene_id {
                    if self.active_warp_scene == Some(source) {
                        self.active_warp_count = self.active_warp_count.saturating_add(1);
                    } else if self.replaced_warp_scene == Some(source) {
                        self.facts.reprojected_per_scene = Some(
                            self.facts
                                .reprojected_per_scene
                                .unwrap_or(0)
                                .saturating_add(1),
                        );
                    }
                }
                Some(PresentEvent::WarpCompleted { measurement })
            }
            FenceDecision::Refused {
                reason,
                polls,
                wall_ms,
                precision_mode,
            } => {
                let id = pending.ledger.id();
                self.gpu.warp_fence.unmap();
                self.warp_fence = None;
                Some(PresentEvent::FenceRefused {
                    kind: SubmissionKind::Warp,
                    id,
                    reason,
                    polls,
                    wall_ms,
                    precision_mode,
                })
            }
        }
    }

    fn publish_promoted(&mut self, frame: &crate::SceneFrame, glitch_pixel_count: Option<u32>) {
        let fills_current = self
            .hot
            .iter()
            .flatten()
            .max_by_key(|pose| pose.epoch)
            .is_some_and(|pose| *pose == frame.pose);
        if fills_current {
            self.exposure.scene_completed();
            self.facts.scene_fill_due = false;
            self.facts.warp_exposed = false;
            self.facts.warp_exposed_fraction = Some(0.0);
        }
        self.replaced_warp_scene = self.active_warp_scene;
        self.facts.reprojected_per_scene = self.active_warp_scene.map(|_| self.active_warp_count);
        self.active_warp_scene = Some(frame.scene_id);
        self.active_warp_count = 0;
        self.facts.completed_scene_id = Some(frame.scene_id);
        self.facts.source_generation = Some(frame.pose.orbit_generation);
        self.facts.held_frame_partition = None;
        self.facts.held_since_scene_id = None;
        self.facts.precision_mode = frame.precision_mode;
        self.facts.delivered_width = frame.extent[0];
        self.facts.delivered_height = frame.extent[1];
        self.facts.delivered_level = Some(frame.level);
        self.facts.iteration_cap = Some(frame.iteration_cap);
        self.facts.glitch_pixel_count = if frame.level == RefinementLevel::Final {
            glitch_pixel_count
        } else {
            None
        };
        self.facts.palette = frame.palette;
        self.facts.view = frame.pose.view;
        self.facts.status = PresentStatus::ShowingCompletedScene;
    }
}
