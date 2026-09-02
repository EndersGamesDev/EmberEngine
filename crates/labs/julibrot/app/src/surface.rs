//! Single-owner surface state and delayed presentation after warp completion.

use ember_lab_heap::SurfaceOwnership;

use crate::AppError;

/// One acquired image retained until its matching warp reaches a terminal event.
#[derive(Debug)]
pub struct PendingSurface<T> {
    /// Present-owned warp submission identifier.
    pub warp_id: u64,
    /// Orbit generation attributed to the surface refresh.
    pub generation: u32,
    /// Acquired surface image, consumed by present or drop.
    pub frame: T,
}

/// Terminal action produced for one pending image.
#[derive(Debug, Eq, PartialEq)]
pub enum SurfaceAction<T> {
    /// A matching completed warp may now be presented outside measurement.
    Present(T),
    /// A matching refusal or cancellation must be dropped unpresented.
    Drop(T),
    /// The event did not name the live warp.
    Ignore,
}

/// Exactly-zero-or-one pending image plus the inherited ownership token.
#[derive(Debug)]
pub struct SurfaceState<T> {
    ownership: SurfaceOwnership,
    pending: Option<PendingSurface<T>>,
}

impl<T> SurfaceState<T> {
    /// Creates an available surface with no pending image.
    #[must_use]
    pub fn new() -> Self {
        Self {
            ownership: SurfaceOwnership::default(),
            pending: None,
        }
    }

    /// Claims the sole acquisition token before calling `get_current_texture`.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::SurfaceBusy`] while another generation owns an image.
    pub fn claim(&mut self, generation: u32) -> Result<(), AppError> {
        let requested = u64::from(generation);
        self.ownership
            .try_acquire(requested)
            .map_err(|owner| AppError::SurfaceBusy { owner, requested })
    }

    /// Releases an acquisition that never became pending.
    #[must_use]
    pub fn release_unsubmitted(&mut self, generation: u32) -> bool {
        self.ownership.release(u64::from(generation))
    }

    /// Associates the acquired image with the warp returned by present.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::SurfaceBusy`] if the supplied generation does not own the token or an
    /// image is already pending.
    pub fn retain(&mut self, pending: PendingSurface<T>) -> Result<(), AppError> {
        let requested = u64::from(pending.generation);
        let owner = self.ownership.owner();
        if owner != Some(requested) || self.pending.is_some() {
            return Err(AppError::SurfaceBusy {
                owner: owner.unwrap_or(requested),
                requested,
            });
        }
        self.pending = Some(pending);
        Ok(())
    }

    /// Consumes a matching completion and releases ownership before returning the image.
    #[must_use]
    pub fn complete(&mut self, warp_id: u64) -> SurfaceAction<T> {
        self.resolve(warp_id, true)
    }

    /// Consumes a matching refusal and releases ownership before returning the image to drop.
    #[must_use]
    pub fn refuse(&mut self, warp_id: u64) -> SurfaceAction<T> {
        self.resolve(warp_id, false)
    }

    /// Drops any pending image owned by the supplied generation.
    #[must_use]
    pub fn cancel_generation(&mut self, generation: u32) -> SurfaceAction<T> {
        if self.pending.as_ref().map(|pending| pending.generation) != Some(generation) {
            return SurfaceAction::Ignore;
        }
        self.take_pending(false)
    }

    /// Returns the live warp identifier, if any.
    #[must_use]
    pub fn pending_warp_id(&self) -> Option<u64> {
        self.pending.as_ref().map(|pending| pending.warp_id)
    }

    /// Returns the current acquisition owner.
    #[must_use]
    pub const fn owner(&self) -> Option<u64> {
        self.ownership.owner()
    }

    fn resolve(&mut self, warp_id: u64, present: bool) -> SurfaceAction<T> {
        if self.pending_warp_id() != Some(warp_id) {
            return SurfaceAction::Ignore;
        }
        self.take_pending(present)
    }

    fn take_pending(&mut self, present: bool) -> SurfaceAction<T> {
        let Some(pending) = self.pending.take() else {
            return SurfaceAction::Ignore;
        };
        let released = self.ownership.release(u64::from(pending.generation));
        debug_assert!(released, "pending surface must own its generation");
        if present {
            SurfaceAction::Present(pending.frame)
        } else {
            SurfaceAction::Drop(pending.frame)
        }
    }
}

impl<T> Default for SurfaceState<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{PendingSurface, SurfaceAction, SurfaceState};

    #[test]
    fn only_the_matching_warp_releases_an_image_for_present() {
        let mut state = SurfaceState::new();
        state.claim(7).expect("surface is initially available");
        state
            .retain(PendingSurface {
                warp_id: 11,
                generation: 7,
                frame: "frame-7",
            })
            .expect("owner may retain its image");
        assert_eq!(state.complete(10), SurfaceAction::Ignore);
        assert_eq!(state.owner(), Some(7));
        assert_eq!(state.complete(11), SurfaceAction::Present("frame-7"));
        assert_eq!(state.owner(), None);
    }

    #[test]
    fn refusal_and_cancellation_drop_without_presenting() {
        let mut state = SurfaceState::new();
        state.claim(3).expect("surface is initially available");
        state
            .retain(PendingSurface {
                warp_id: 5,
                generation: 3,
                frame: 19,
            })
            .expect("owner may retain its image");
        assert_eq!(state.refuse(5), SurfaceAction::Drop(19));

        state.claim(4).expect("refusal released the token");
        state
            .retain(PendingSurface {
                warp_id: 6,
                generation: 4,
                frame: 23,
            })
            .expect("new generation may retain");
        assert_eq!(state.cancel_generation(3), SurfaceAction::Ignore);
        assert_eq!(state.cancel_generation(4), SurfaceAction::Drop(23));
        assert_eq!(state.owner(), None);
    }

    #[test]
    fn acquisition_is_single_owner_even_before_warp_submission() {
        let mut state = SurfaceState::<()>::new();
        state.claim(1).expect("first claim succeeds");
        assert!(state.claim(2).is_err());
        assert!(state.release_unsubmitted(1));
        state.claim(2).expect("released claim may be reused");
    }
}
