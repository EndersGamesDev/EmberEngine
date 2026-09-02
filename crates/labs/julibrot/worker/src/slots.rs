//! Four-slot ownership state machine independent of transport lowering.

use crate::{ChannelError, ErrorCode, MessageKind, Pool};

/// Pool and pool-local slot identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SlotId {
    /// Request or orbit pool.
    pub(crate) pool: Pool,
    /// Pool-local index, zero or one.
    pub(crate) slot: u32,
}

impl SlotId {
    /// Validates a pool-local slot.
    pub(crate) const fn new(pool: Pool, slot: u32) -> Result<Self, ChannelError> {
        if slot > 1 {
            return Err(ChannelError::new(ErrorCode::BadTrailer, slot, 0, 0));
        }
        Ok(Self { pool, slot })
    }

    const fn index(self) -> usize {
        let pool_base = match self.pool {
            Pool::Request => 0,
            Pool::Orbit => 2,
        };
        pool_base + self.slot as usize
    }
}

/// Exactly one current owner or directed transit state for a slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlotOwner {
    /// Main can read or write the attached buffer.
    Main,
    /// A main-to-producer transfer detached main.
    ToProducer,
    /// Producer can read or write the attached buffer.
    Producer,
    /// A producer-to-main transfer detached producer.
    ToMain,
}

/// Transport-independent ownership of two request and two orbit slots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FourSlotModel {
    owners: [SlotOwner; 4],
}

impl Default for FourSlotModel {
    fn default() -> Self {
        Self::new()
    }
}

impl FourSlotModel {
    /// Starts with request slots on main and orbit slots on producer.
    pub(crate) const fn new() -> Self {
        Self {
            owners: [
                SlotOwner::Main,
                SlotOwner::Main,
                SlotOwner::Producer,
                SlotOwner::Producer,
            ],
        }
    }

    /// Reports the unique current owner or transit direction.
    pub(crate) const fn owner(self, id: SlotId) -> SlotOwner {
        self.owners[id.index()]
    }

    /// Starts the transfer permitted by `kind`, detaching its sender.
    pub(crate) fn begin(&mut self, id: SlotId, kind: MessageKind) -> Result<(), ChannelError> {
        let (expected_pool, from, to) = transfer_rule(kind)?;
        if id.pool != expected_pool || self.owner(id) != from {
            return Err(ChannelError::new(ErrorCode::BufferStarved, id.slot, 0, 0));
        }
        self.owners[id.index()] = to;
        Ok(())
    }

    /// Completes one browser delivery and attaches the receiver.
    pub(crate) const fn deliver(&mut self, id: SlotId) -> Result<(), ChannelError> {
        let delivered = match self.owner(id) {
            SlotOwner::ToProducer => SlotOwner::Producer,
            SlotOwner::ToMain => SlotOwner::Main,
            SlotOwner::Main | SlotOwner::Producer => {
                return Err(ChannelError::new(ErrorCode::BufferStarved, id.slot, 0, 0));
            }
        };
        self.owners[id.index()] = delivered;
        Ok(())
    }

    /// Returns whether all slots are attached to their startup owners.
    pub(crate) const fn is_reconciled(self) -> bool {
        matches!(self.owners[0], SlotOwner::Main)
            && matches!(self.owners[1], SlotOwner::Main)
            && matches!(self.owners[2], SlotOwner::Producer)
            && matches!(self.owners[3], SlotOwner::Producer)
    }
}

const fn transfer_rule(kind: MessageKind) -> Result<(Pool, SlotOwner, SlotOwner), ChannelError> {
    match kind {
        MessageKind::OrbitRequest | MessageKind::Shutdown => Ok((
            Pool::Request,
            SlotOwner::Main,
            SlotOwner::ToProducer,
        )),
        MessageKind::RequestReturn | MessageKind::ShutdownAck => Ok((
            Pool::Request,
            SlotOwner::Producer,
            SlotOwner::ToMain,
        )),
        MessageKind::OrbitResponse | MessageKind::OrbitCancelled => Ok((
            Pool::Orbit,
            SlotOwner::Producer,
            SlotOwner::ToMain,
        )),
        MessageKind::CreditApplied | MessageKind::CreditStale => Ok((
            Pool::Orbit,
            SlotOwner::Main,
            SlotOwner::ToProducer,
        )),
        MessageKind::ChannelError => Err(ChannelError::new(
            ErrorCode::BadKind,
            kind as u32,
            0,
            0,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{FourSlotModel, SlotId, SlotOwner};
    use crate::{MessageKind, Pool};

    #[test]
    fn both_slots_in_both_pools_complete_full_round_trips() {
        for slot in 0..=1 {
            let request = SlotId::new(Pool::Request, slot).unwrap();
            let orbit = SlotId::new(Pool::Orbit, slot).unwrap();
            let mut model = FourSlotModel::new();

            model.begin(request, MessageKind::OrbitRequest).unwrap();
            assert_eq!(model.owner(request), SlotOwner::ToProducer);
            model.deliver(request).unwrap();
            model.begin(request, MessageKind::RequestReturn).unwrap();
            model.deliver(request).unwrap();

            model.begin(orbit, MessageKind::OrbitResponse).unwrap();
            assert_eq!(model.owner(orbit), SlotOwner::ToMain);
            model.deliver(orbit).unwrap();
            model.begin(orbit, MessageKind::CreditApplied).unwrap();
            model.deliver(orbit).unwrap();
            assert!(model.is_reconciled());
        }
    }

    #[test]
    fn cancellation_stale_credit_and_shutdown_preserve_ownership() {
        let request = SlotId::new(Pool::Request, 0).unwrap();
        let orbit = SlotId::new(Pool::Orbit, 1).unwrap();
        let mut model = FourSlotModel::new();
        model.begin(request, MessageKind::Shutdown).unwrap();
        model.deliver(request).unwrap();
        model.begin(request, MessageKind::ShutdownAck).unwrap();
        model.deliver(request).unwrap();
        model.begin(orbit, MessageKind::OrbitCancelled).unwrap();
        model.deliver(orbit).unwrap();
        model.begin(orbit, MessageKind::CreditStale).unwrap();
        model.deliver(orbit).unwrap();
        assert!(model.is_reconciled());
    }

    #[test]
    fn detached_or_wrong_pool_sender_is_rejected() {
        let request = SlotId::new(Pool::Request, 0).unwrap();
        let mut model = FourSlotModel::new();
        model.begin(request, MessageKind::OrbitRequest).unwrap();
        assert!(model.begin(request, MessageKind::OrbitRequest).is_err());
        assert!(model.begin(request, MessageKind::OrbitResponse).is_err());
        assert!(model.deliver(request).is_ok());
        assert!(model.deliver(request).is_err());
    }
}
