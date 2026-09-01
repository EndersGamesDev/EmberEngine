use std::collections::VecDeque;

use crate::{CorrectionMode, RemoteEntityHooks};

/// One timestamped authoritative remote-entity snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteSnapshot<S> {
    /// Opaque monotonically ordered game timestamp.
    pub server_timestamp: u64,
    /// Game-owned entity state.
    pub snapshot: S,
}

/// Result of inserting a remote snapshot into an ordered bounded buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotPush {
    /// A newer snapshot was appended.
    Appended,
    /// A duplicate timestamp replaced the newest snapshot.
    Replaced,
    /// An older snapshot was ignored to preserve monotonic order.
    Stale,
}

/// Capacity-bounded monotonic buffer for one remote entity.
#[derive(Clone, Debug)]
pub struct RemoteSnapshotBuffer<S> {
    capacity: usize,
    snapshots: VecDeque<RemoteSnapshot<S>>,
}

impl<S> RemoteSnapshotBuffer<S> {
    /// Constructs an empty buffer; a zero request is normalized to one.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            snapshots: VecDeque::with_capacity(capacity.max(1)),
        }
    }

    /// Inserts one snapshot without permitting time to move backward.
    pub fn push(&mut self, server_timestamp: u64, snapshot: S) -> SnapshotPush {
        if let Some(latest) = self.snapshots.back_mut() {
            if server_timestamp < latest.server_timestamp {
                return SnapshotPush::Stale;
            }
            if server_timestamp == latest.server_timestamp {
                latest.snapshot = snapshot;
                return SnapshotPush::Replaced;
            }
        }
        self.snapshots.push_back(RemoteSnapshot {
            server_timestamp,
            snapshot,
        });
        while self.snapshots.len() > self.capacity {
            self.snapshots.pop_front();
        }
        SnapshotPush::Appended
    }

    /// Returns the newest authoritative sample.
    #[must_use]
    pub fn latest(&self) -> Option<&RemoteSnapshot<S>> {
        self.snapshots.back()
    }

    /// Clears every sample without changing capacity.
    pub fn clear(&mut self) {
        self.snapshots.clear();
    }

    /// Resolves an interpolation or dead-reckoning cursor through game hooks.
    #[must_use]
    pub fn sample_at<H>(&self, hooks: &H, server_timestamp: u64) -> Option<H::RenderState>
    where
        H: RemoteEntityHooks<Snapshot = S>,
    {
        let first = self.snapshots.front()?;
        if server_timestamp <= first.server_timestamp {
            return Some(hooks.interpolate_remote(&first.snapshot, &first.snapshot, 1, 1));
        }
        for (from, to) in self.snapshots.iter().zip(self.snapshots.iter().skip(1)) {
            if server_timestamp <= to.server_timestamp {
                if hooks.snap_or_smooth_remote(&from.snapshot, &to.snapshot)
                    == CorrectionMode::Snap
                {
                    return Some(hooks.interpolate_remote(&to.snapshot, &to.snapshot, 1, 1));
                }
                return Some(hooks.interpolate_remote(
                    &from.snapshot,
                    &to.snapshot,
                    server_timestamp.saturating_sub(from.server_timestamp),
                    to.server_timestamp.saturating_sub(from.server_timestamp).max(1),
                ));
            }
        }
        let latest = self.snapshots.back()?;
        Some(hooks.dead_reckon_remote(
            &latest.snapshot,
            server_timestamp.saturating_sub(latest.server_timestamp),
        ))
    }

    /// Returns the number of retained snapshots.
    #[must_use]
    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    /// Returns whether the buffer has no authoritative sample.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeRemote;

    impl RemoteEntityHooks for FakeRemote {
        type Snapshot = i64;
        type RenderState = i64;

        fn interpolate_remote(
            &self,
            from: &Self::Snapshot,
            to: &Self::Snapshot,
            numerator: u64,
            denominator: u64,
        ) -> Self::RenderState {
            let span = to - from;
            *from + span * i64::try_from(numerator).unwrap_or(i64::MAX)
                / i64::try_from(denominator).unwrap_or(i64::MAX)
        }

        fn dead_reckon_remote(
            &self,
            latest: &Self::Snapshot,
            elapsed: u64,
        ) -> Self::RenderState {
            *latest + i64::try_from(elapsed).unwrap_or(i64::MAX)
        }

        fn snap_or_smooth_remote(
            &self,
            from: &Self::Snapshot,
            to: &Self::Snapshot,
        ) -> CorrectionMode {
            if to - from > 50 {
                CorrectionMode::Snap
            } else {
                CorrectionMode::Smooth
            }
        }
    }

    #[test]
    fn buffer_is_bounded_and_ignores_stale_time() {
        let mut buffer = RemoteSnapshotBuffer::new(2);
        assert_eq!(buffer.push(10, 10), SnapshotPush::Appended);
        assert_eq!(buffer.push(20, 20), SnapshotPush::Appended);
        assert_eq!(buffer.push(15, 15), SnapshotPush::Stale);
        assert_eq!(buffer.push(30, 30), SnapshotPush::Appended);
        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.latest().map(|sample| sample.snapshot), Some(30));
    }

    #[test]
    fn hook_controls_interpolation_dead_reckoning_and_snap() {
        let mut buffer = RemoteSnapshotBuffer::new(3);
        buffer.push(10, 10);
        buffer.push(20, 20);
        assert_eq!(buffer.sample_at(&FakeRemote, 15), Some(15));
        assert_eq!(buffer.sample_at(&FakeRemote, 25), Some(25));
        buffer.push(30, 100);
        assert_eq!(buffer.sample_at(&FakeRemote, 25), Some(100));
    }
}
