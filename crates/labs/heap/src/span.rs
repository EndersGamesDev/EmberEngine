//! Constant-class logical spans, directory packing, dispatch headers, and wall arithmetic.

use bytemuck::{Pod, Zeroable};
use serde::Serialize;
use thiserror::Error;

use crate::{Handle, HeapAllocator, HeapError, HeapKind};

const RECORD_WORDS: usize = 4;

/// One shader-visible span record; the fourth word is reserved and must remain zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct PackedSpan {
    words: [u32; RECORD_WORDS],
}

impl PackedSpan {
    /// Packs the quotient/remainder addressing constants for one span.
    #[must_use]
    pub const fn new(page_records: u32, page_count: u32, first_directory_slot: u32) -> Self {
        Self {
            words: [page_records, page_count, first_directory_slot, 0],
        }
    }

    /// Returns the shader-visible words.
    #[must_use]
    pub const fn words(self) -> [u32; RECORD_WORDS] {
        self.words
    }
}

/// A logical RGBA32F allocation composed of equal-class physical pages.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DataSpan {
    /// Number of addressable records, excluding final-page padding.
    pub logical_len: u32,
    /// Records reserved by every page.
    pub page_records: u32,
    /// Number of physical page handles.
    pub page_count: u32,
    /// First slot in the directory's handle area.
    pub first_directory_slot: u32,
    /// Span-record index used by a generated accessor.
    pub directory_index: u32,
    handles: Vec<Handle>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SpanIdentity {
    logical_len: u32,
    page_records: u32,
    page_count: u32,
    first_directory_slot: u32,
    directory_index: u32,
    handles: Vec<Handle>,
}

/// Exact result of a non-mutating single-span allocation trial.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct SpanPlan {
    /// Physical pages a real allocation would consume.
    pub pages: u32,
    /// Square buddy side selected for every page.
    pub page_class: u16,
    /// RGBA32F bytes reserved including final-page padding.
    pub reserved_bytes: u64,
    /// Contiguous page-handle directory slots consumed.
    pub directory_slots: u32,
}

impl DataSpan {
    /// Ordered page handles, validated by generation on every debug CPU access.
    #[must_use]
    pub fn handles(&self) -> &[Handle] {
        &self.handles
    }

    /// Resolves one logical index into a page handle and page-local record.
    ///
    /// # Errors
    ///
    /// Returns a typed bounds, directory-corruption, or stale-handle error.
    pub fn resolve_record(
        &self,
        allocator: &HeapAllocator,
        index: u32,
    ) -> Result<(Handle, u32), SpanError> {
        if index >= self.logical_len {
            return Err(SpanError::IndexOutOfBounds {
                index,
                logical_len: self.logical_len,
            });
        }
        let page = index / self.page_records;
        let local = index % self.page_records;
        let handle = *self
            .handles
            .get(page as usize)
            .ok_or(SpanError::DirectoryCorrupt)?;
        if cfg!(debug_assertions) {
            allocator.resolve(handle)?;
        }
        Ok((handle, local))
    }

    /// Reserved records including the last page's padding.
    #[must_use]
    pub const fn reserved_records(&self) -> u64 {
        self.page_records as u64 * self.page_count as u64
    }

    /// Unused records in the last page.
    #[must_use]
    pub const fn padding_records(&self) -> u64 {
        self.reserved_records() - self.logical_len as u64
    }

    pub(crate) fn identity(&self) -> SpanIdentity {
        SpanIdentity {
            logical_len: self.logical_len,
            page_records: self.page_records,
            page_count: self.page_count,
            first_directory_slot: self.first_directory_slot,
            directory_index: self.directory_index,
            handles: self.handles.clone(),
        }
    }

    pub(crate) fn prefix(&self, active_len: u32) -> Result<Self, SpanError> {
        if active_len == 0 {
            return Err(SpanError::ZeroLength);
        }
        if active_len > self.logical_len {
            return Err(SpanError::IndexOutOfBounds {
                index: active_len - 1,
                logical_len: self.logical_len,
            });
        }
        let page_count = active_len.div_ceil(self.page_records);
        let handle_count =
            usize::try_from(page_count).map_err(|_| SpanError::ArithmeticOverflow)?;
        let handles = self
            .handles
            .get(..handle_count)
            .ok_or(SpanError::DirectoryCorrupt)?
            .to_vec();
        Ok(Self {
            logical_len: active_len,
            page_records: self.page_records,
            page_count,
            first_directory_slot: self.first_directory_slot,
            directory_index: self.directory_index,
            handles,
        })
    }
}

/// Fixed-capacity UBO directory with 16-byte span records followed by packed handles.
#[derive(Clone, Debug)]
pub struct SpanDirectory {
    span_capacity: u32,
    handle_capacity: u32,
    records: Vec<Option<PackedSpan>>,
    handles: Vec<Option<Handle>>,
}

impl SpanDirectory {
    /// Splits a live uniform-binding limit into span metadata and page handles.
    ///
    /// # Errors
    ///
    /// Returns an error when the binding cannot hold the requested fixed span-record capacity.
    pub fn from_binding_limit(binding_bytes: u32, span_capacity: u32) -> Result<Self, SpanError> {
        let record_bytes = span_capacity
            .checked_mul(16)
            .ok_or(SpanError::ArithmeticOverflow)?;
        let handle_capacity = binding_bytes
            .checked_sub(record_bytes)
            .ok_or(SpanError::DirectoryTooSmall)?
            / 4;
        if span_capacity == 0 || handle_capacity == 0 {
            return Err(SpanError::DirectoryTooSmall);
        }
        Ok(Self {
            span_capacity,
            handle_capacity,
            records: vec![None; span_capacity as usize],
            handles: vec![None; handle_capacity as usize],
        })
    }

    /// Maximum number of logical spans.
    #[must_use]
    pub const fn span_capacity(&self) -> u32 {
        self.span_capacity
    }

    /// Maximum number of physical-page handles.
    #[must_use]
    pub const fn handle_capacity(&self) -> u32 {
        self.handle_capacity
    }

    fn first_record(&self) -> Result<usize, SpanError> {
        self.records
            .iter()
            .position(Option::is_none)
            .ok_or(SpanError::SpanRecordsFull)
    }

    fn first_handle_run(&self, count: usize) -> Result<usize, SpanError> {
        self.handles
            .windows(count)
            .position(|window| window.iter().all(Option::is_none))
            .ok_or(SpanError::PageDirectoryFull)
    }

    fn insert(&mut self, page_records: u32, pages: &[Handle]) -> Result<(u32, u32), SpanError> {
        let record = self.first_record()?;
        let first = self.first_handle_run(pages.len())?;
        let page_count = u32::try_from(pages.len()).map_err(|_| SpanError::ArithmeticOverflow)?;
        let record_index = u32::try_from(record).map_err(|_| SpanError::ArithmeticOverflow)?;
        let first_directory_slot =
            u32::try_from(first).map_err(|_| SpanError::ArithmeticOverflow)?;
        for (slot, handle) in self.handles[first..first + pages.len()]
            .iter_mut()
            .zip(pages)
        {
            *slot = Some(*handle);
        }
        self.records[record] = Some(PackedSpan::new(
            page_records,
            page_count,
            first_directory_slot,
        ));
        Ok((record_index, first_directory_slot))
    }

    fn remove(&mut self, span: &DataSpan) -> Result<(), SpanError> {
        let record = self
            .records
            .get_mut(span.directory_index as usize)
            .ok_or(SpanError::DirectoryCorrupt)?;
        if record.take().is_none() {
            return Err(SpanError::DirectoryCorrupt);
        }
        let first = span.first_directory_slot as usize;
        let end = first
            .checked_add(span.page_count as usize)
            .ok_or(SpanError::ArithmeticOverflow)?;
        for slot in self
            .handles
            .get_mut(first..end)
            .ok_or(SpanError::DirectoryCorrupt)?
        {
            *slot = None;
        }
        Ok(())
    }

    fn contains(&self, identity: &SpanIdentity) -> bool {
        let expected = PackedSpan::new(
            identity.page_records,
            identity.page_count,
            identity.first_directory_slot,
        );
        let Some(record) = self.records.get(identity.directory_index as usize) else {
            return false;
        };
        if *record != Some(expected) {
            return false;
        }
        let first = identity.first_directory_slot as usize;
        let Some(end) = first.checked_add(identity.handles.len()) else {
            return false;
        };
        self.handles.get(first..end).is_some_and(|handles| {
            handles
                .iter()
                .zip(&identity.handles)
                .all(|(stored, expected)| *stored == Some(*expected))
        })
    }

    /// Packs the whole binding with span records first and raw handle words second.
    #[must_use]
    pub fn packed_words(&self) -> Vec<u32> {
        let mut words = Vec::with_capacity(self.records.len() * RECORD_WORDS + self.handles.len());
        for record in &self.records {
            words.extend(record.unwrap_or_else(PackedSpan::zeroed).words());
        }
        words.extend(
            self.handles
                .iter()
                .map(|handle| handle.map_or(0, Handle::raw)),
        );
        words
    }
}

/// Heap plus span directory, cloned transactionally for dry runs and paired allocation.
#[derive(Clone)]
pub struct SpanArena {
    heap: HeapAllocator,
    directory: SpanDirectory,
}

impl SpanArena {
    /// Creates the physical allocator and its runtime-sized directory.
    ///
    /// # Errors
    ///
    /// Returns the underlying heap or directory configuration failure.
    pub fn new(
        side: u16,
        layers: u16,
        descriptor_capacity: u32,
        directory_binding_bytes: u32,
        span_capacity: u32,
    ) -> Result<Self, SpanError> {
        Ok(Self {
            heap: HeapAllocator::new(side, layers, descriptor_capacity)?,
            directory: SpanDirectory::from_binding_limit(directory_binding_bytes, span_capacity)?,
        })
    }

    /// Read-only descriptor allocator access for packing and debug validation.
    #[must_use]
    pub const fn heap(&self) -> &HeapAllocator {
        &self.heap
    }

    /// Read-only span directory access for UBO packing.
    #[must_use]
    pub const fn directory(&self) -> &SpanDirectory {
        &self.directory
    }

    fn allocate_one(&mut self, logical_len: u32, page_side: u16) -> Result<DataSpan, SpanError> {
        if logical_len == 0 {
            return Err(SpanError::ZeroLength);
        }
        if page_side == 0 || !page_side.is_power_of_two() || page_side > self.heap.side() {
            return Err(SpanError::InvalidPageClass(page_side));
        }
        let page_records = u32::from(page_side) * u32::from(page_side);
        let page_count = logical_len.div_ceil(page_records);
        let mut handles = Vec::with_capacity(page_count as usize);
        for _ in 0..page_count {
            match self.heap.allocate(HeapKind::Data, page_side, page_side) {
                Ok(handle) => handles.push(handle),
                Err(error) => {
                    for handle in handles {
                        self.heap.free(handle)?;
                    }
                    return Err(error.into());
                }
            }
        }
        let (directory_index, first_directory_slot) =
            match self.directory.insert(page_records, &handles) {
                Ok(placed) => placed,
                Err(error) => {
                    for handle in handles {
                        self.heap.free(handle)?;
                    }
                    return Err(error);
                }
            };
        Ok(DataSpan {
            logical_len,
            page_records,
            page_count,
            first_directory_slot,
            directory_index,
            handles,
        })
    }

    /// Allocates one span transactionally.
    ///
    /// # Errors
    ///
    /// Returns a typed capacity, directory, class, or arithmetic failure without mutation.
    pub fn allocate_span(
        &mut self,
        logical_len: u32,
        page_side: u16,
    ) -> Result<DataSpan, SpanError> {
        let mut trial = self.clone();
        let span = trial.allocate_one(logical_len, page_side)?;
        *self = trial;
        Ok(span)
    }

    /// Trials one exact allocation against a clone, leaving this arena untouched.
    ///
    /// # Errors
    ///
    /// Returns the same typed capacity, fragmentation, directory, class, or arithmetic failure
    /// that a real allocation against the current arena would return.
    pub fn plan_span(&self, logical_len: u32, page_side: u16) -> Result<SpanPlan, SpanError> {
        let mut trial = self.clone();
        let span = trial.allocate_one(logical_len, page_side)?;
        Ok(SpanPlan {
            pages: span.page_count,
            page_class: page_side,
            reserved_bytes: span.reserved_records() * 16,
            directory_slots: span.page_count,
        })
    }

    /// Allocates two equal-class spans atomically; failure leaves this arena byte-for-byte logical.
    ///
    /// # Errors
    ///
    /// Returns a typed capacity, directory, class, or arithmetic failure.
    pub fn allocate_pair(
        &mut self,
        logical_len: u32,
        page_side: u16,
    ) -> Result<[DataSpan; 2], SpanError> {
        let mut trial = self.clone();
        let first = trial.allocate_one(logical_len, page_side)?;
        let second = trial.allocate_one(logical_len, page_side)?;
        *self = trial;
        Ok([first, second])
    }

    /// Finds the greatest whole-copy paired allocation without mutating this arena.
    #[must_use]
    pub fn plan_paired_copies(
        &self,
        requested_copies: u64,
        records_per_copy: u32,
        page_side: u16,
    ) -> u64 {
        if records_per_copy == 0 {
            return 0;
        }
        let address_copies = u64::from(u32::MAX) / u64::from(records_per_copy);
        let mut low = 0_u64;
        let mut high = requested_copies.min(address_copies);
        while low < high {
            let middle = low + (high - low).div_ceil(2);
            let logical = middle * u64::from(records_per_copy);
            let fits = u32::try_from(logical).is_ok_and(|length| {
                let mut trial = self.clone();
                trial.allocate_pair(length, page_side).is_ok()
            });
            if fits {
                low = middle;
            } else {
                high = middle - 1;
            }
        }
        low
    }

    /// Frees both directory and physical pages of one span.
    ///
    /// # Errors
    ///
    /// Returns a typed directory or generation error for a non-live span.
    pub fn free(&mut self, span: DataSpan) -> Result<(), SpanError> {
        self.directory.remove(&span)?;
        for handle in span.handles {
            self.heap.free(handle)?;
        }
        Ok(())
    }

    pub(crate) fn validate_header_owner(&self, identity: &SpanIdentity) -> Result<(), SpanError> {
        if !self.directory.contains(identity) {
            return Err(SpanError::DirectoryCorrupt);
        }
        for handle in &identity.handles {
            self.heap.resolve(*handle)?;
        }
        Ok(())
    }
}

/// One static, dynamically selected per-page dispatch uniform.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct DispatchHeader {
    /// First logical output record written by this pass.
    pub global_base: u32,
    /// Valid records in this page, excluding padding.
    pub valid_length: u32,
    padding: [u32; 2],
}

/// Pre-uploaded headers and dynamic offsets for one step plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticHeaders {
    /// Uniform-buffer bytes, including alignment gaps.
    pub bytes: Vec<u8>,
    /// Dynamic offset selecting each header.
    pub offsets: Vec<u32>,
    /// Runtime-aligned stride in bytes.
    pub stride: u32,
    owner: SpanIdentity,
}

impl StaticHeaders {
    /// Builds one header per span page at the device's dynamic-uniform alignment.
    ///
    /// # Errors
    ///
    /// Returns an error for zero alignment or byte-size overflow.
    pub fn for_span(span: &DataSpan, alignment: u32) -> Result<Self, SpanError> {
        if alignment == 0 {
            return Err(SpanError::ZeroAlignment);
        }
        let stride = 16_u32.div_ceil(alignment) * alignment;
        let byte_len = stride
            .checked_mul(span.page_count)
            .ok_or(SpanError::ArithmeticOverflow)?;
        let mut bytes = vec![0; byte_len as usize];
        let mut offsets = Vec::with_capacity(span.page_count as usize);
        for page in 0..span.page_count {
            let global_base = page * span.page_records;
            let valid_length = (span.logical_len - global_base).min(span.page_records);
            let header = DispatchHeader {
                global_base,
                valid_length,
                padding: [0; 2],
            };
            let offset = page * stride;
            bytes[offset as usize..offset as usize + 16]
                .copy_from_slice(bytemuck::bytes_of(&header));
            offsets.push(offset);
        }
        Ok(Self {
            bytes,
            offsets,
            stride,
            owner: span.identity(),
        })
    }

    /// Builds immutable headers for a dense prefix of a larger allocation.
    ///
    /// # Errors
    ///
    /// Returns an error when the prefix is empty, exceeds the span, or its byte layout overflows.
    pub fn for_prefix(span: &DataSpan, active_len: u32, alignment: u32) -> Result<Self, SpanError> {
        let mut headers = Self::for_span(&span.prefix(active_len)?, alignment)?;
        headers.owner = span.identity();
        Ok(headers)
    }

    pub(crate) const fn owner(&self) -> &SpanIdentity {
        &self.owner
    }
}

/// One independently computed capacity or policy term.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WallTerm {
    /// Stable page label.
    pub name: &'static str,
    /// Whole copies admitted by this term.
    pub copies: u64,
}

/// Requested-versus-delivered minimum with its first limiting term.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DeliveryPlan {
    /// Requested whole copies.
    pub requested_copies: u64,
    /// Delivered whole copies.
    pub delivered_copies: u64,
    /// Stable name and value of every runtime or type term.
    pub terms: Vec<WallTerm>,
    /// First term equal to the delivered minimum.
    pub limiting_term: &'static str,
}

impl DeliveryPlan {
    /// Computes the displayed delivery minimum without changing the requested control.
    #[must_use]
    pub fn new(requested_copies: u64, terms: Vec<WallTerm>) -> Self {
        let (limiting_term, delivered_copies) = terms
            .iter()
            .map(|term| (term.name, term.copies))
            .chain(std::iter::once(("requested", requested_copies)))
            .min_by_key(|(_, copies)| *copies)
            .unwrap_or(("requested", requested_copies));
        Self {
            requested_copies,
            delivered_copies,
            terms,
            limiting_term,
        }
    }
}

/// Typed span-planning or addressing failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SpanError {
    /// Underlying descriptor or buddy allocation failure.
    #[error(transparent)]
    Heap(#[from] HeapError),
    /// A span cannot have an empty logical domain.
    #[error("span logical length must be nonzero")]
    ZeroLength,
    /// Page side must be a supported buddy class.
    #[error("page side {0} must be a nonzero power of two within the heap")]
    InvalidPageClass(u16),
    /// Fixed span-record space is exhausted.
    #[error("span-directory record capacity is exhausted")]
    SpanRecordsFull,
    /// No contiguous ordered run remains in the page-handle directory.
    #[error("span-directory page-handle capacity or contiguity is exhausted")]
    PageDirectoryFull,
    /// The UBO limit cannot hold both required directory regions.
    #[error("span-directory uniform binding is too small")]
    DirectoryTooSmall,
    /// An index is outside the logical span rather than its padded reservation.
    #[error("span index {index} is outside logical length {logical_len}")]
    IndexOutOfBounds {
        /// Requested logical index.
        index: u32,
        /// Span's logical record count.
        logical_len: u32,
    },
    /// CPU directory state disagrees with its typed span.
    #[error("span directory is inconsistent with its typed allocation")]
    DirectoryCorrupt,
    /// Dynamic uniform offsets require a nonzero device alignment.
    #[error("dynamic uniform alignment must be nonzero")]
    ZeroAlignment,
    /// Fixed-width byte or record arithmetic overflowed.
    #[error("span arithmetic overflow")]
    ArithmeticOverflow,
}

#[cfg(test)]
mod tests {
    use crate::HeapKind;

    use super::{DeliveryPlan, SpanArena, SpanError, SpanPlan, StaticHeaders, WallTerm};

    fn snapshot(arena: &SpanArena) -> (Vec<crate::PackedDescriptor>, Vec<u32>, usize, usize) {
        (
            arena.heap().packed_table(),
            arena.directory().packed_words(),
            arena.heap().free_descriptor_count(),
            arena.heap().free_block_count(HeapKind::Data),
        )
    }

    fn allocated_plan(span: &super::DataSpan, page_class: u16) -> SpanPlan {
        SpanPlan {
            pages: span.page_count,
            page_class,
            reserved_bytes: span.reserved_records() * 16,
            directory_slots: span.page_count,
        }
    }

    #[test]
    fn constant_class_span_crosses_layers_by_quotient_and_remainder() {
        let mut arena =
            SpanArena::new(8, 3, 32, 16 * 16 + 4 * 16, 16).expect("arena configuration fits");
        let [first, second] = arena
            .allocate_pair(70, 4)
            .expect("ten class-four pages fit across three layers");
        assert_eq!((first.page_records, first.page_count), (16, 5));
        assert_eq!(first.padding_records(), 10);
        let (handle, local) = first
            .resolve_record(arena.heap(), 69)
            .expect("last logical record resolves");
        assert_eq!(local, 5);
        assert_eq!(arena.heap().resolve(handle).expect("page is live").layer, 1);
        assert_eq!(
            first.resolve_record(arena.heap(), 70),
            Err(SpanError::IndexOutOfBounds {
                index: 70,
                logical_len: 70,
            })
        );
        arena.free(first).expect("first span reclaims pages");
        arena.free(second).expect("second span reclaims pages");
    }

    #[test]
    fn paired_dry_run_is_atomic_and_directory_reclaims_runs() {
        let mut arena =
            SpanArena::new(8, 1, 16, 16 * 4 + 4 * 8, 4).expect("arena configuration fits");
        assert_eq!(arena.plan_paired_copies(99, 8, 4), 4);
        let before = arena.directory().packed_words();
        assert!(arena.allocate_pair(40, 4).is_err());
        assert_eq!(arena.directory().packed_words(), before);
        let pair = arena
            .allocate_pair(32, 4)
            .expect("four pages fit atomically");
        for span in pair {
            arena.free(span).expect("directory run is reclaimed");
        }
        assert_eq!(arena.directory().packed_words(), before);
    }

    #[test]
    fn single_span_trial_matches_real_success_and_never_mutates() {
        let mut arena =
            SpanArena::new(16, 2, 32, 16 * 8 + 4 * 16, 8).expect("arena configuration fits");
        let before = snapshot(&arena);
        let trial = arena.plan_span(700, 16).expect("three pages fit");
        assert_eq!(snapshot(&arena), before);
        let allocated = arena
            .allocate_span(700, 16)
            .expect("the real allocation follows the trial");
        assert_eq!(trial, allocated_plan(&allocated, 16));
    }

    #[test]
    fn single_span_trial_matches_fragmentation_and_directory_run_failures() {
        let mut fragmented =
            SpanArena::new(8, 1, 64, 16 * 32 + 4 * 32, 32).expect("arena configuration fits");
        let blocks = (0..16)
            .map(|_| {
                fragmented
                    .allocate_span(4, 2)
                    .expect("class-two block fits")
            })
            .collect::<Vec<_>>();
        for index in (0..blocks.len()).step_by(2) {
            fragmented
                .free(blocks[index].clone())
                .expect("alternating block frees");
        }
        let before = snapshot(&fragmented);
        let trial = fragmented.plan_span(16, 4);
        assert_eq!(snapshot(&fragmented), before);
        let mut real = fragmented.clone();
        assert_eq!(
            trial,
            real.allocate_span(16, 4)
                .map(|span| allocated_plan(&span, 4))
        );

        let mut directory =
            SpanArena::new(8, 1, 64, 16 * 8 + 4 * 6, 8).expect("arena configuration fits");
        let first = directory.allocate_span(8, 2).expect("first run fits");
        let _middle = directory.allocate_span(8, 2).expect("middle run fits");
        let last = directory.allocate_span(8, 2).expect("last run fits");
        directory.free(first).expect("first run frees");
        directory.free(last).expect("last run frees");
        let before = snapshot(&directory);
        let trial = directory.plan_span(12, 2);
        assert_eq!(trial, Err(SpanError::PageDirectoryFull));
        assert_eq!(snapshot(&directory), before);
        let mut real = directory.clone();
        assert_eq!(
            trial,
            real.allocate_span(12, 2)
                .map(|span| allocated_plan(&span, 2))
        );
    }

    #[test]
    fn directory_packing_and_static_headers_match_the_ubo_contract() {
        let mut arena =
            SpanArena::new(16, 4, 16, 16 * 4 + 4 * 8, 4).expect("arena configuration fits");
        let [span, other] = arena
            .allocate_pair(300, 16)
            .expect("two pages per span fit");
        let words = arena.directory().packed_words();
        let record = span.directory_index as usize * 4;
        assert_eq!(
            &words[record..record + 4],
            &[256, 2, span.first_directory_slot, 0]
        );
        let headers = StaticHeaders::for_span(&span, 256).expect("alignment is valid");
        assert_eq!(headers.stride, 256);
        assert_eq!(headers.offsets, [0, 256]);
        assert_eq!(headers.bytes.len(), 512);
        assert_eq!(
            bytemuck::from_bytes::<[u32; 4]>(&headers.bytes[256..272]),
            &[256, 44, 0, 0]
        );
        arena.free(span).expect("first frees");
        arena.free(other).expect("second frees");
    }

    #[test]
    fn dense_prefix_headers_are_immutable_and_stop_at_the_active_tail() {
        let mut arena =
            SpanArena::new(16, 4, 16, 16 * 4 + 4 * 8, 4).expect("arena configuration fits");
        let span = arena
            .allocate_span(700, 16)
            .expect("three pages fit in the arena");
        let before = span.clone();
        let headers = StaticHeaders::for_prefix(&span, 300, 256).expect("prefix is valid");
        assert_eq!(headers.offsets, [0, 256]);
        assert_eq!(headers.bytes.len(), 512);
        assert_eq!(
            bytemuck::from_bytes::<[u32; 4]>(&headers.bytes[256..272]),
            &[256, 44, 0, 0]
        );
        assert_eq!(span, before);
        assert_eq!(
            StaticHeaders::for_prefix(&span, 0, 256),
            Err(SpanError::ZeroLength)
        );
        assert_eq!(
            StaticHeaders::for_prefix(&span, 701, 256),
            Err(SpanError::IndexOutOfBounds {
                index: 700,
                logical_len: 700,
            })
        );
    }

    #[test]
    fn wall_minimum_preserves_the_request_and_names_the_limiter() {
        let plan = DeliveryPlan::new(
            6_125,
            vec![
                WallTerm {
                    name: "paired heap",
                    copies: 4_000,
                },
                WallTerm {
                    name: "u32 address",
                    copies: 1_431_655,
                },
                WallTerm {
                    name: "draw policy",
                    copies: 715_827,
                },
            ],
        );
        assert_eq!(plan.requested_copies, 6_125);
        assert_eq!(plan.delivered_copies, 4_000);
        assert_eq!(plan.limiting_term, "paired heap");
    }
}
