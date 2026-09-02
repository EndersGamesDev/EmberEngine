//! Handle, descriptor, and deterministic buddy-allocation contract.

// Descriptor words and bounded heap dimensions intentionally narrow at their fixed-width ABI.
#![allow(clippy::cast_lossless, clippy::cast_possible_truncation)]

use std::collections::BTreeSet;

use bytemuck::{Pod, Zeroable};
use serde::Serialize;
use thiserror::Error;

const INDEX_BITS: u32 = 20;
const GENERATION_BITS: u32 = 12;
const INDEX_MASK: u32 = (1 << INDEX_BITS) - 1;
const GENERATION_MASK: u16 = (1 << GENERATION_BITS) - 1;

/// A physical heap selected by a descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[repr(u8)]
pub enum HeapKind {
    /// RGBA32F, nearest-sampled structured data.
    Data = 0,
    /// RGBA8, linearly sampled image data.
    Image = 1,
}

impl HeapKind {
    const fn index(self) -> usize {
        self as usize
    }
}

/// A stable heap handle with a 20-bit descriptor index and 12-bit generation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
#[repr(transparent)]
pub struct Handle(u32);

impl Handle {
    /// Builds a handle from its independently validated fields.
    ///
    /// # Errors
    ///
    /// Returns [`HeapError::InvalidHandleFields`] when the index does not fit 20 bits or the
    /// generation is zero or does not fit 12 bits.
    pub const fn encode(index: u32, generation: u16) -> Result<Self, HeapError> {
        if index > INDEX_MASK || generation == 0 || generation > GENERATION_MASK {
            return Err(HeapError::InvalidHandleFields { index, generation });
        }
        Ok(Self((u32::from(generation) << INDEX_BITS) | index))
    }

    /// Returns the descriptor index field.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.0 & INDEX_MASK
    }

    /// Returns the nonzero generation field.
    #[must_use]
    pub const fn generation(self) -> u16 {
        (self.0 >> INDEX_BITS) as u16
    }

    /// Returns the wire-sized integer representation.
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// Logical fields encoded in one 16-byte descriptor-table record.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct Descriptor {
    /// Array layer.
    pub layer: u16,
    /// Region origin x coordinate.
    pub x: u16,
    /// Region origin y coordinate.
    pub y: u16,
    /// Logical region width.
    pub width: u16,
    /// Logical region height.
    pub height: u16,
    /// Physical heap class.
    pub kind: HeapKind,
}

/// Four-word descriptor-table representation with a 16-byte uniform stride.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct PackedDescriptor {
    words: [u32; 4],
}

impl PackedDescriptor {
    /// Canonical missing-resource record used by descriptor slot zero and free slots.
    pub const MISSING: Self = Self { words: [0; 4] };

    /// Packs one live descriptor.
    ///
    /// # Errors
    ///
    /// Returns [`HeapError::ZeroExtent`] when either logical extent is zero.
    pub const fn pack(descriptor: Descriptor) -> Result<Self, HeapError> {
        if descriptor.width == 0 || descriptor.height == 0 {
            return Err(HeapError::ZeroExtent);
        }
        Ok(Self {
            words: [
                u32::from(descriptor.layer) | (u32::from(descriptor.x) << 16),
                u32::from(descriptor.y) | (u32::from(descriptor.width) << 16),
                u32::from(descriptor.height),
                descriptor.kind as u32,
            ],
        })
    }

    /// Decodes a live descriptor and validates every reserved bit.
    ///
    /// # Errors
    ///
    /// Returns a typed error for the missing record, nonzero reserved bits, an unknown heap kind,
    /// or a zero extent.
    pub const fn unpack(self) -> Result<Descriptor, HeapError> {
        if self.words[0] == 0 && self.words[1] == 0 && self.words[2] == 0 && self.words[3] == 0 {
            return Err(HeapError::MissingDescriptor);
        }
        if self.words[2] & 0xffff_0000 != 0 || self.words[3] & !1 != 0 {
            return Err(HeapError::ReservedDescriptorBits);
        }
        let kind = if self.words[3] == 0 {
            HeapKind::Data
        } else if self.words[3] == 1 {
            HeapKind::Image
        } else {
            return Err(HeapError::UnknownHeapKind(self.words[3]));
        };
        let descriptor = Descriptor {
            layer: self.words[0] as u16,
            x: (self.words[0] >> 16) as u16,
            y: self.words[1] as u16,
            width: (self.words[1] >> 16) as u16,
            height: self.words[2] as u16,
            kind,
        };
        if descriptor.width == 0 || descriptor.height == 0 {
            return Err(HeapError::ZeroExtent);
        }
        Ok(descriptor)
    }

    /// Returns the record's four shader-visible words.
    #[must_use]
    pub const fn words(self) -> [u32; 4] {
        self.words
    }
}

/// A typed heap-allocation or stale-access failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HeapError {
    /// Handle fields exceed their fixed bit allocation.
    #[error("invalid handle fields: index {index}, generation {generation}")]
    InvalidHandleFields {
        /// Requested descriptor index.
        index: u32,
        /// Requested generation.
        generation: u16,
    },
    /// A descriptor extent is zero.
    #[error("descriptor width and height must both be nonzero")]
    ZeroExtent,
    /// The canonical missing record was decoded as live.
    #[error("descriptor is the canonical missing-resource record")]
    MissingDescriptor,
    /// Reserved descriptor bits were nonzero.
    #[error("descriptor contains nonzero reserved bits")]
    ReservedDescriptorBits,
    /// The heap-kind word was not recognized.
    #[error("descriptor heap kind {0} is unknown")]
    UnknownHeapKind(u32),
    /// The configured heap geometry is not representable by this allocator.
    #[error("heap side {side} must be a nonzero power of two and layers must be nonzero")]
    InvalidHeapGeometry {
        /// Requested square side.
        side: u16,
    },
    /// The descriptor table cannot represent the requested capacity.
    #[error("descriptor capacity {0} must be in 2..=1048576")]
    InvalidDescriptorCapacity(u32),
    /// The requested region exceeds one physical layer.
    #[error("requested {width}x{height} region exceeds heap side {side}")]
    RegionTooLarge {
        /// Requested logical width.
        width: u16,
        /// Requested logical height.
        height: u16,
        /// Physical layer side.
        side: u16,
    },
    /// No descriptor index remains available.
    #[error("descriptor table is full")]
    DescriptorTableFull,
    /// Buddy fragmentation or total capacity prevents placement.
    #[error("{kind:?} heap has no {size}x{size} buddy block")]
    PhysicalHeapFull {
        /// Selected physical heap.
        kind: HeapKind,
        /// Rounded square class.
        size: u16,
    },
    /// The handle index is outside this allocator's descriptor table.
    #[error("handle {handle:#010x} addresses descriptor {index}, outside capacity {capacity}")]
    HandleOutOfBounds {
        /// Raw handle.
        handle: u32,
        /// Decoded index.
        index: u32,
        /// Configured table capacity.
        capacity: u32,
    },
    /// The descriptor slot is free or permanently retired.
    #[error("handle {handle:#010x} addresses descriptor {index}, which is not live")]
    DescriptorNotLive {
        /// Raw handle.
        handle: u32,
        /// Decoded index.
        index: u32,
    },
    /// Debug generation validation caught use-after-free.
    #[error("stale handle {handle:#010x}; descriptor generation is {current_generation}")]
    StaleHandle {
        /// Raw stale handle.
        handle: u32,
        /// Current descriptor generation.
        current_generation: u16,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Block {
    layer: u16,
    x: u16,
    y: u16,
    size: u16,
}

struct BuddyLayer {
    side: u16,
    free_by_order: Vec<BTreeSet<(u16, u16)>>,
}

impl BuddyLayer {
    fn new(side: u16) -> Self {
        let mut free_by_order = vec![BTreeSet::new(); side.trailing_zeros() as usize + 1];
        free_by_order[side.trailing_zeros() as usize].insert((0, 0));
        Self {
            side,
            free_by_order,
        }
    }

    fn allocate(&mut self, layer: u16, size: u16) -> Option<Block> {
        let target_order = size.trailing_zeros() as usize;
        let source_order = (target_order..self.free_by_order.len())
            .find(|order| !self.free_by_order[*order].is_empty())?;
        let &(x, y) = self.free_by_order[source_order].first()?;
        self.free_by_order[source_order].remove(&(x, y));
        let mut block_size = 1_u16 << source_order;
        while block_size > size {
            block_size /= 2;
            self.free_by_order[block_size.trailing_zeros() as usize].extend([
                (x + block_size, y),
                (x, y + block_size),
                (x + block_size, y + block_size),
            ]);
        }
        Some(Block { layer, x, y, size })
    }

    fn free(&mut self, block: Block) {
        let mut x = block.x;
        let mut y = block.y;
        let mut size = block.size;
        loop {
            let order = size.trailing_zeros() as usize;
            self.free_by_order[order].insert((x, y));
            if size == self.side {
                return;
            }
            let parent_size = size * 2;
            let parent_x = x / parent_size * parent_size;
            let parent_y = y / parent_size * parent_size;
            let siblings = [
                (parent_x, parent_y),
                (parent_x + size, parent_y),
                (parent_x, parent_y + size),
                (parent_x + size, parent_y + size),
            ];
            if !siblings
                .iter()
                .all(|sibling| self.free_by_order[order].contains(sibling))
            {
                return;
            }
            for sibling in siblings {
                self.free_by_order[order].remove(&sibling);
            }
            x = parent_x;
            y = parent_y;
            size = parent_size;
        }
    }

    fn free_blocks(&self) -> usize {
        self.free_by_order.iter().map(BTreeSet::len).sum()
    }
}

#[derive(Clone, Copy)]
struct Slot {
    generation: u16,
    descriptor: PackedDescriptor,
    block: Option<Block>,
    retired: bool,
}

/// Shared descriptor free list with independent DATA and IMAGE buddy layers.
pub struct HeapAllocator {
    side: u16,
    layers: u16,
    heaps: [Vec<BuddyLayer>; 2],
    slots: Vec<Slot>,
    free_descriptors: Vec<u32>,
}

impl HeapAllocator {
    /// Creates a deterministic allocator; descriptor zero is permanently missing.
    ///
    /// # Errors
    ///
    /// Returns a typed configuration error for a non-power-of-two side, zero layers, or a
    /// descriptor capacity outside the 20-bit handle domain.
    pub fn new(side: u16, layers: u16, descriptor_capacity: u32) -> Result<Self, HeapError> {
        if side == 0 || !side.is_power_of_two() || layers == 0 {
            return Err(HeapError::InvalidHeapGeometry { side });
        }
        if !(2..=INDEX_MASK + 1).contains(&descriptor_capacity) {
            return Err(HeapError::InvalidDescriptorCapacity(descriptor_capacity));
        }
        let make_layers = || (0..layers).map(|_| BuddyLayer::new(side)).collect();
        let mut slots = vec![
            Slot {
                generation: 1,
                descriptor: PackedDescriptor::MISSING,
                block: None,
                retired: false,
            };
            descriptor_capacity as usize
        ];
        slots[0].retired = true;
        let free_descriptors = (1..descriptor_capacity).rev().collect();
        Ok(Self {
            side,
            layers,
            heaps: [make_layers(), make_layers()],
            slots,
            free_descriptors,
        })
    }

    /// Allocates one region, rounding it to the documented square buddy class.
    ///
    /// # Errors
    ///
    /// Returns a typed extent, descriptor-capacity, or physical-capacity error.
    pub fn allocate(
        &mut self,
        kind: HeapKind,
        width: u16,
        height: u16,
    ) -> Result<Handle, HeapError> {
        if width == 0 || height == 0 {
            return Err(HeapError::ZeroExtent);
        }
        let longest = width.max(height);
        if longest > self.side {
            return Err(HeapError::RegionTooLarge {
                width,
                height,
                side: self.side,
            });
        }
        let size = longest.next_power_of_two();
        let Some(index) = self.free_descriptors.pop() else {
            return Err(HeapError::DescriptorTableFull);
        };
        let block = self.heaps[kind.index()]
            .iter_mut()
            .enumerate()
            .find_map(|(layer, allocator)| allocator.allocate(layer as u16, size));
        let Some(block) = block else {
            self.free_descriptors.push(index);
            return Err(HeapError::PhysicalHeapFull { kind, size });
        };
        let descriptor = Descriptor {
            layer: block.layer,
            x: block.x,
            y: block.y,
            width,
            height,
            kind,
        };
        let packed = PackedDescriptor::pack(descriptor)?;
        let slot = &mut self.slots[index as usize];
        slot.descriptor = packed;
        slot.block = Some(block);
        Handle::encode(index, slot.generation)
    }

    /// Resolves a live handle to its logical descriptor.
    ///
    /// # Errors
    ///
    /// Returns a typed bounds, liveness, generation, or descriptor-decode error.
    pub fn resolve(&self, handle: Handle) -> Result<Descriptor, HeapError> {
        let slot = self.slot(handle)?;
        slot.descriptor.unpack()
    }

    /// Frees a live handle, coalesces its physical block, and advances its generation.
    ///
    /// # Errors
    ///
    /// Returns a typed bounds, liveness, or generation error when the handle is not current.
    pub fn free(&mut self, handle: Handle) -> Result<(), HeapError> {
        let index = handle.index() as usize;
        self.slot(handle)?;
        let slot = &mut self.slots[index];
        let block = slot.block.take().ok_or(HeapError::DescriptorNotLive {
            handle: handle.raw(),
            index: handle.index(),
        })?;
        let kind = slot.descriptor.unpack()?.kind;
        self.heaps[kind.index()][block.layer as usize].free(block);
        slot.descriptor = PackedDescriptor::MISSING;
        if slot.generation == GENERATION_MASK {
            slot.retired = true;
        } else {
            slot.generation += 1;
            self.free_descriptors.push(index as u32);
        }
        Ok(())
    }

    fn slot(&self, handle: Handle) -> Result<&Slot, HeapError> {
        let index = handle.index() as usize;
        let Some(slot) = self.slots.get(index) else {
            return Err(HeapError::HandleOutOfBounds {
                handle: handle.raw(),
                index: handle.index(),
                capacity: self.slots.len() as u32,
            });
        };
        if slot.block.is_none() || slot.retired {
            return Err(HeapError::DescriptorNotLive {
                handle: handle.raw(),
                index: handle.index(),
            });
        }
        if cfg!(debug_assertions) && slot.generation != handle.generation() {
            return Err(HeapError::StaleHandle {
                handle: handle.raw(),
                current_generation: slot.generation,
            });
        }
        Ok(slot)
    }

    /// Returns a full table snapshot suitable for one UBO upload.
    #[must_use]
    pub fn packed_table(&self) -> Vec<PackedDescriptor> {
        self.slots.iter().map(|slot| slot.descriptor).collect()
    }

    /// Returns the selected square side.
    #[must_use]
    pub const fn side(&self) -> u16 {
        self.side
    }

    /// Returns the selected layer count per physical heap.
    #[must_use]
    pub const fn layers(&self) -> u16 {
        self.layers
    }

    /// Returns the number of descriptor indices available for allocation.
    #[must_use]
    pub fn free_descriptor_count(&self) -> usize {
        self.free_descriptors.len()
    }

    /// Returns the current number of free buddy blocks in one heap.
    #[must_use]
    pub fn free_block_count(&self, kind: HeapKind) -> usize {
        self.heaps[kind.index()]
            .iter()
            .map(BuddyLayer::free_blocks)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use super::{Descriptor, Handle, HeapAllocator, HeapError, HeapKind, PackedDescriptor};

    #[test]
    fn handles_encode_and_decode_the_contract_split() {
        let handle = Handle::encode(0x0a_bcde, 0x0fed).expect("fields fit their contract");
        assert_eq!(handle.index(), 0x0a_bcde);
        assert_eq!(handle.generation(), 0x0fed);
        assert_eq!(handle.raw(), 0xfedabcde);
        assert!(Handle::encode(1 << 20, 1).is_err());
        assert!(Handle::encode(1, 0).is_err());
        assert!(Handle::encode(1, 1 << 12).is_err());
    }

    #[test]
    fn descriptor_packing_round_trips_every_field() {
        let descriptor = Descriptor {
            layer: 0x1234,
            x: 0x5678,
            y: 0x9abc,
            width: 0xdef0,
            height: 0x1357,
            kind: HeapKind::Image,
        };
        let packed = PackedDescriptor::pack(descriptor).expect("descriptor is live");
        assert_eq!(packed.words(), [0x5678_1234, 0xdef0_9abc, 0x0000_1357, 1]);
        assert_eq!(packed.unpack(), Ok(descriptor));
        assert_eq!(
            PackedDescriptor::MISSING.unpack(),
            Err(HeapError::MissingDescriptor)
        );
    }

    #[test]
    fn size_classes_roll_layers_and_reclaim_four_way_buddies() {
        let mut allocator = HeapAllocator::new(8, 2, 32).expect("geometry is valid");
        let mut first_layer = Vec::new();
        for _ in 0..4 {
            let handle = allocator
                .allocate(HeapKind::Data, 3, 2)
                .expect("four 4x4 classes fit layer zero");
            assert_eq!(allocator.resolve(handle).expect("handle is live").layer, 0);
            first_layer.push(handle);
        }
        let rollover = allocator
            .allocate(HeapKind::Data, 4, 4)
            .expect("fifth class rolls to layer one");
        assert_eq!(
            allocator.resolve(rollover).expect("handle is live").layer,
            1
        );
        for handle in first_layer {
            allocator.free(handle).expect("handle frees exactly once");
        }
        let reclaimed = allocator
            .allocate(HeapKind::Data, 8, 8)
            .expect("four siblings coalesce to the whole first layer");
        let descriptor = allocator
            .resolve(reclaimed)
            .expect("reclaimed handle is live");
        assert_eq!((descriptor.layer, descriptor.x, descriptor.y), (0, 0, 0));
        assert!(
            writeln!(
                std::io::stdout().lock(),
                "heap allocator evidence: side=8 layers=2 class=4 rollover_layer=1 reclaimed_class=8 reclaimed_origin=(0,0,0)"
            )
            .is_ok()
        );
    }

    #[test]
    fn free_list_reuses_index_and_debug_catches_stale_generation() {
        let mut allocator = HeapAllocator::new(8, 1, 4).expect("geometry is valid");
        let old = allocator
            .allocate(HeapKind::Image, 1, 1)
            .expect("first allocation fits");
        allocator.free(old).expect("first generation frees");
        let fresh = allocator
            .allocate(HeapKind::Image, 1, 1)
            .expect("free-list index is reused");
        assert_eq!(old.index(), fresh.index());
        assert_eq!(fresh.generation(), old.generation() + 1);
        if cfg!(debug_assertions) {
            assert!(matches!(
                allocator.resolve(old),
                Err(HeapError::StaleHandle { .. })
            ));
        }
    }

    #[test]
    fn exhausted_generation_retires_instead_of_wrapping() {
        let mut allocator = HeapAllocator::new(1, 1, 2).expect("one live slot is valid");
        for generation in 1..=4095 {
            let handle = allocator
                .allocate(HeapKind::Data, 1, 1)
                .expect("slot remains reusable before retirement");
            assert_eq!(handle.generation(), generation);
            allocator.free(handle).expect("current generation frees");
        }
        assert_eq!(allocator.free_descriptor_count(), 0);
        assert_eq!(
            allocator.allocate(HeapKind::Data, 1, 1),
            Err(HeapError::DescriptorTableFull)
        );
    }
}
