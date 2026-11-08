//! Deterministic fixed-capacity atomic arena allocator.
//!
//! Provides a bump-style allocator backed by a fixed-size buffer and an
//! atomic cursor. Individual deallocations are no-ops; memory is reclaimed
//! only via an explicit `reset`.

use std::alloc::{GlobalAlloc, Layout};
use std::cell::UnsafeCell;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Fixed-capacity deterministic arena allocator.
///
/// `CAPACITY` is the total number of bytes the arena can hold.
/// Allocation is performed by atomically advancing a cursor and returning
/// a pointer into the internal buffer. Alignment is respected for every
/// request.
///
/// This type is intentionally simple:
/// - no free-list
/// - no individual reclamation
/// - reset is the only way to free memory
///
/// Safe for concurrent allocation (via `AtomicUsize` CAS), but the
/// provided benchmarks run single-threaded.
pub struct DeterministicAllocator<const CAPACITY: usize> {
    /// Backing storage. `UnsafeCell` allows interior mutability without
    /// requiring `&mut self` for allocation.
    storage: UnsafeCell<[u8; CAPACITY]>,
    /// Current offset from the start of the buffer.
    cursor: AtomicUsize,
}

// Safety: the allocator only exposes raw pointers obtained from its own
// fixed buffer. Concurrent access is synchronized by the atomic cursor.
unsafe impl<const CAPACITY: usize> Sync for DeterministicAllocator<CAPACITY> {}
unsafe impl<const CAPACITY: usize> Send for DeterministicAllocator<CAPACITY> {}

impl<const CAPACITY: usize> DeterministicAllocator<CAPACITY> {
    /// Creates a new empty arena.
    ///
    /// The backing buffer is zero-initialized.
    pub const fn new() -> Self {
        Self {
            storage: UnsafeCell::new([0u8; CAPACITY]),
            cursor: AtomicUsize::new(0),
        }
    }

    /// Returns the total capacity in bytes.
    #[inline]
    pub const fn capacity(&self) -> usize {
        CAPACITY
    }

    /// Returns the number of bytes currently reserved.
    #[inline]
    pub fn used(&self) -> usize {
        self.cursor.load(Ordering::Relaxed)
    }

    /// Resets the arena, making all previously allocated memory available
    /// again. Existing pointers become dangling after a reset.
    ///
    /// # Safety
    ///
    /// The caller must ensure that no outstanding pointers into the arena
    /// are used after this call.
    #[inline]
    pub unsafe fn reset(&self) {
        self.cursor.store(0, Ordering::Release);
    }

    /// Attempts to allocate a block with the given layout.
    ///
    /// Returns `Ok(ptr)` on success or `Err(())` when the arena is exhausted.
    #[inline]
    pub fn allocate(&self, layout: Layout) -> Result<*mut u8, ()> {
        let size = layout.size();
        let align = layout.align();

        // Zero-sized allocations are allowed by the GlobalAlloc contract.
        if size == 0 {
            // Return a dangling but well-aligned non-null pointer.
            return Ok(align as *mut u8);
        }

        loop {
            let current = self.cursor.load(Ordering::Acquire);

            // Align the current offset upward.
            let aligned = (current + align - 1) & !(align - 1);
            let new_cursor = aligned.checked_add(size).ok_or(())?;

            if new_cursor > CAPACITY {
                return Err(());
            }

            // Try to claim the range.
            match self.cursor.compare_exchange_weak(
                current,
                new_cursor,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // Safety: we just reserved [aligned, new_cursor) inside
                    // the fixed buffer, and the buffer lives as long as self.
                    let base = self.storage.get() as *


        
       

        
   

        f
