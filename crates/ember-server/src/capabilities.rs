//! Host implementations of the narrow `ember-legacy` capability traits.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use ember_legacy::{
    BroadcastHandle, CloseReason, CloseRequest, GameKey, LegacyCapabilities, LegacyClock,
    LegacyRandom, LegacyTransport, MetricObservation, MonotonicTimestamp, PeerId, RandomDrawKey,
    ScheduleError, ScheduleHandle, SchedulingRequest, SessionId, TransportError, UnicastHandle,
};
use sha2::{Digest, Sha256};

const MAX_PENDING_SCHEDULES: usize = 1_024;
const RANDOM_DOMAIN: &[u8] = b"ember-legacy-random-v1\0";

/// One process-wide monotonic epoch shared by every hosted version.
#[derive(Clone, Copy)]
pub(crate) struct HostEpoch {
    started: Instant,
}

impl HostEpoch {
    pub(crate) fn new() -> Self {
        Self {
            started: Instant::now(),
        }
    }

    pub(crate) fn now(self) -> MonotonicTimestamp {
        let micros = u64::try_from(self.started.elapsed().as_micros()).unwrap_or(u64::MAX);
        MonotonicTimestamp::from_micros(micros)
    }
}

pub(crate) struct HostClock {
    epoch: HostEpoch,
    shutting_down: Arc<AtomicBool>,
    next_handle: AtomicU64,
    schedules: Mutex<BTreeMap<u64, MonotonicTimestamp>>,
}

impl HostClock {
    pub(crate) fn new(epoch: HostEpoch, shutting_down: Arc<AtomicBool>) -> Self {
        Self {
            epoch,
            shutting_down,
            next_handle: AtomicU64::new(1),
            schedules: Mutex::new(BTreeMap::new()),
        }
    }

    pub(crate) fn has_due_schedule(&self, now: MonotonicTimestamp) -> bool {
        let Ok(mut schedules) = self.schedules.lock() else {
            return true;
        };
        let due_handles: Vec<_> = schedules
            .iter()
            .filter(|(_, timestamp)| **timestamp <= now)
            .map(|(handle, _)| *handle)
            .collect();
        let has_due = !due_handles.is_empty();
        for handle in due_handles {
            schedules.remove(&handle);
        }
        has_due
    }
}

impl LegacyClock for HostClock {
    fn now(&self) -> MonotonicTimestamp {
        self.epoch.now()
    }

    fn request_schedule(
        &self,
        request: SchedulingRequest,
    ) -> Result<ScheduleHandle, ScheduleError> {
        if self.shutting_down.load(Ordering::Acquire) {
            return Err(ScheduleError::ShuttingDown);
        }
        let timestamp = match request {
            SchedulingRequest::At(timestamp) => timestamp,
            SchedulingRequest::After(duration) => self.now().saturating_add(duration),
        };
        let mut schedules = self
            .schedules
            .lock()
            .map_err(|_| ScheduleError::ShuttingDown)?;
        if schedules.len() >= MAX_PENDING_SCHEDULES {
            return Err(ScheduleError::CapacityReached);
        }
        let handle = self.next_handle.fetch_add(1, Ordering::Relaxed);
        schedules.insert(handle, timestamp);
        Ok(ScheduleHandle::from_host_value(handle))
    }

    fn cancel_schedule(&self, handle: ScheduleHandle) -> Result<(), ScheduleError> {
        let mut schedules = self
            .schedules
            .lock()
            .map_err(|_| ScheduleError::ShuttingDown)?;
        if schedules.remove(&handle.host_value()).is_some() {
            Ok(())
        } else {
            Err(ScheduleError::UnknownHandle)
        }
    }
}

pub(crate) struct HostRandom;

impl LegacyRandom for HostRandom {
    fn draw_u64(&self, key: &RandomDrawKey) -> u64 {
        let mut bytes = [0_u8; 8];
        self.fill_bytes(key, &mut bytes);
        u64::from_le_bytes(bytes)
    }

    fn fill_bytes(&self, key: &RandomDrawKey, output: &mut [u8]) {
        for (block_index, block) in output.chunks_mut(32).enumerate() {
            let mut digest = Sha256::new();
            digest.update(RANDOM_DOMAIN);
            update_length_prefixed(&mut digest, key.game_key.game_id.as_bytes());
            digest.update(key.game_key.game_version.to_le_bytes());
            digest.update(key.lobby_seed.0);
            update_length_prefixed(&mut digest, key.stream_key.0.as_bytes());
            digest.update(key.event_index.to_le_bytes());
            digest.update(
                u64::try_from(block_index)
                    .unwrap_or(u64::MAX)
                    .to_le_bytes(),
            );
            let result = digest.finalize();
            block.copy_from_slice(&result[..block.len()]);
        }
    }
}

fn update_length_prefixed(digest: &mut Sha256, bytes: &[u8]) {
    digest.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    digest.update(bytes);
}

pub(crate) struct HostTransport {
    game_key: GameKey,
    session_id: SessionId,
    shutting_down: Arc<AtomicBool>,
    peers: Mutex<BTreeSet<PeerId>>,
    close_requests: Mutex<Vec<CloseRequest>>,
}

impl HostTransport {
    pub(crate) fn new(
        game_key: GameKey,
        session_id: SessionId,
        shutting_down: Arc<AtomicBool>,
    ) -> Self {
        Self {
            game_key,
            session_id,
            shutting_down,
            peers: Mutex::new(BTreeSet::new()),
            close_requests: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn add_peer(&self, peer_id: PeerId) {
        if let Ok(mut peers) = self.peers.lock() {
            peers.insert(peer_id);
        }
    }

    pub(crate) fn remove_peer(&self, peer_id: PeerId) {
        if let Ok(mut peers) = self.peers.lock() {
            peers.remove(&peer_id);
        }
    }

    pub(crate) fn take_close_requests(&self) -> Vec<CloseRequest> {
        let Ok(mut requests) = self.close_requests.lock() else {
            return Vec::new();
        };
        std::mem::take(&mut *requests)
    }
}

impl LegacyTransport for HostTransport {
    fn unicast(&self, peer_id: PeerId) -> Result<UnicastHandle, TransportError> {
        if self.shutting_down.load(Ordering::Acquire) {
            return Err(TransportError::ShuttingDown);
        }
        let peers = self
            .peers
            .lock()
            .map_err(|_| TransportError::ShuttingDown)?;
        if !peers.contains(&peer_id) {
            return Err(TransportError::UnknownPeer);
        }
        // The frozen capability has no host constructor for this opaque handle.
        Err(TransportError::QueueFull)
    }

    fn broadcast(&self, session_id: SessionId) -> Result<BroadcastHandle, TransportError> {
        if self.shutting_down.load(Ordering::Acquire) {
            return Err(TransportError::ShuttingDown);
        }
        if session_id != self.session_id {
            return Err(TransportError::UnknownSession);
        }
        // The frozen capability has no host constructor for this opaque handle.
        Err(TransportError::QueueFull)
    }

    fn close_peer(&self, peer_id: PeerId, reason: CloseReason) -> Result<(), TransportError> {
        let peers = self
            .peers
            .lock()
            .map_err(|_| TransportError::ShuttingDown)?;
        if !peers.contains(&peer_id) {
            return Err(TransportError::UnknownPeer);
        }
        drop(peers);
        self.close_requests
            .lock()
            .map_err(|_| TransportError::ShuttingDown)?
            .push(CloseRequest { peer_id, reason });
        Ok(())
    }

    fn record_metric(&self, observation: MetricObservation) {
        tracing::info!(
            game = %self.game_key.game_id,
            version = self.game_key.game_version,
            metric = %observation.name,
            value = observation.value,
            "version metric"
        );
    }
}

pub(crate) struct SessionCapabilities {
    pub(crate) capabilities: LegacyCapabilities,
    pub(crate) clock: Arc<HostClock>,
    pub(crate) transport: Arc<HostTransport>,
}

impl SessionCapabilities {
    pub(crate) fn new(
        game_key: GameKey,
        session_id: SessionId,
        epoch: HostEpoch,
        shutting_down: Arc<AtomicBool>,
    ) -> Self {
        let clock = Arc::new(HostClock::new(epoch, Arc::clone(&shutting_down)));
        let transport = Arc::new(HostTransport::new(game_key, session_id, shutting_down));
        Self {
            capabilities: LegacyCapabilities {
                clock: Arc::clone(&clock),
                random: Arc::new(HostRandom),
                transport: Arc::clone(&transport),
                assets: None,
            },
            clock,
            transport,
        }
    }
}
