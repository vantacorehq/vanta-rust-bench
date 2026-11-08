//! Fixed-capacity atomic bump allocator.
//!
//! The allocator reserves aligned memory regions using an atomic cursor.
//! Individual deallocations are intentionally no-ops. Memory is reclaimed
//! only by resetting the complete arena after all allocations are unused.

use std::alloc::{GlobalAlloc, Layout};
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::ptr::{self, NonNull};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Fixed-capacity arena allocator.
///
/// The allocator supports concurrent allocation through atomic cursor
/// reservation. It does not support individual memory reclamation.
pub struct DeterministicAllocator<const N: usize> {
    cursor: AtomicUsize,
    storage: UnsafeCell<[MaybeUninit<u8>; N]>,
}

// Atomic reservations guarantee that successful allocations own
// non-overlapping ranges. Reset requires exclusive logical ownership.
unsafe impl<const N: usize> Sync for DeterministicAllocator<N> {}

impl<const N: usize> DeterministicAllocator<N> {
    /// Creates an empty arena.
    pub const fn new() -> Self {
        Self {
            cursor: AtomicUsize::new(0),
            storage: UnsafeCell::new([MaybeUninit::uninit(); N]),
        }
    }

    /// Returns total arena capacity.
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Returns the number of reserved bytes, including alignment padding.
    pub fn used(&self) -> usize {
        self.cursor.load(Ordering::Acquire)
    }

    /// Returns the unreserved capacity.
    pub fn remaining(&self) -> usize {
        N.saturating_sub(self.used())
    }

    /// Reserves an aligned region from the arena.
    ///
    /// Returns `None` if the requested layout cannot fit.
    pub fn allocate(&self, layout: Layout) -> Option<NonNull<u8>> {
        let size = layout.size().max(1);
        let alignment = layout.align();

        let base = self.storage.get().cast::<u8>();
        let base_address = base as usize;

        let mut current = self.cursor.load(Ordering::Relaxed);

        loop {
            let current_address = base_address.checked_add(current)?;

            let aligned_address = current_address
                .checked_add(alignment - 1)?
                & !(alignment - 1);

            let offset = aligned_address.checked_sub(base_address)?;
            let end = offset.checked_add(size)?;

            if end > N {
                return None;
            }

            match self.cursor.compare_exchange_weak(
                current,
                end,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    let allocation = unsafe { base.add(offset) };
                    return NonNull::new(allocation);
                }
                Err(actual) => current = actual,
            }
        }
    }

    /// Resets the arena allocation cursor.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that:
    ///
    /// - No allocation previously returned by this arena is being accessed.
    /// - No previous allocation will ever be accessed again.
    /// - No thread is concurrently allocating from this arena.
    ///
    /// Resetting while allocations remain in use can cause memory aliasing.
    pub unsafe fn reset(&self) {
        self.cursor.store(0, Ordering::SeqCst);
    }
}

impl<const N: usize> Default for DeterministicAllocator<N> {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: allocate returns disjoint reserved regions. dealloc deliberately
// does not reclaim memory; reset is governed by its documented safety contract.
unsafe impl<const N: usize> GlobalAlloc for DeterministicAllocator<N> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.allocate(layout)
            .map_or(ptr::null_mut(), NonNull::as_ptr)
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

#[cfg(test)]
mod tests {
    use super::DeterministicAllocator;
    use std::alloc::Layout;
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn allocates_requested_size() {
        let arena = DeterministicAllocator::<1024>::new();
        let layout = Layout::from_size_align(64, 8).unwrap();

        let allocation = arena.allocate(layout).unwrap();

        assert_eq!((allocation.as_ptr() as usize) % 8, 0);
        assert!(arena.used() >= 64);
    }

    #[test]
    fn respects_large_alignment() {
        let arena = DeterministicAllocator::<4096>::new();
        let layout = Layout::from_size_align(128, 256).unwrap();

        let allocation = arena.allocate(layout).unwrap();

        assert_eq!((allocation.as_ptr() as usize) % 256, 0);
    }

    #[test]
    fn rejects_exhausted_arena() {
        let arena = DeterministicAllocator::<128>::new();
        let layout = Layout::from_size_align(64, 8).unwrap();

        assert!(arena.allocate(layout).is_some());
        assert!(arena.allocate(layout).is_some());
        assert!(arena.allocate(layout).is_none());
    }

    #[test]
    fn handles_zero_sized_layout() {
        let arena = DeterministicAllocator::<128>::new();
        let layout = Layout::from_size_align(0, 16).unwrap();

        let allocation = arena.allocate(layout).unwrap();

        assert_eq!((allocation.as_ptr() as usize) % 16, 0);
        assert!(arena.used() >= 1);
    }

    #[test]
    fn reset_reclaims_capacity() {
        let arena = DeterministicAllocator::<128>::new();
        let layout = Layout::from_size_align(64, 8).unwrap();

        assert!(arena.allocate(layout).is_some());
        assert!(arena.allocate(layout).is_some());
        assert!(arena.allocate(layout).is_none());

        unsafe {
            arena.reset();
        }

        assert_eq!(arena.used(), 0);
        assert_eq!(arena.remaining(), 128);
        assert!(arena.allocate(layout).is_some());
    }

    #[test]
    fn reports_capacity_and_remaining_space() {
        let arena = DeterministicAllocator::<1024>::new();

        assert_eq!(arena.capacity(), 1024);
        assert_eq!(arena.used(), 0);
        assert_eq!(arena.remaining(), 1024);

        let layout = Layout::from_size_align(64, 8).unwrap();

        assert!(arena.allocate(layout).is_some());
        assert!(arena.used() >= 64);
        assert!(arena.remaining() < 1024);
    }

    #[test]
    fn concurrent_allocations_are_disjoint() {
        const THREADS: usize = 8;
        const PER_THREAD: usize = 128;

        let arena = Arc::new(DeterministicAllocator::<65536>::new());
        let layout = Layout::from_size_align(64, 8).unwrap();

        let mut handles = Vec::new();

        for _ in 0..THREADS {
            let arena = Arc::clone(&arena);

            handles.push(thread::spawn(move || {
                let mut addresses = Vec::with_capacity(PER_THREAD);

                for _ in 0..PER_THREAD {
                    let allocation = arena
                        .allocate(layout)
                        .expect("arena capacity unexpectedly exhausted");

                    addresses.push(allocation.as_ptr() as usize);
                }

                addresses
            }));
        }

        let mut all_addresses = Vec::new();

        for handle in handles {
            all_addresses.extend(handle.join().unwrap());
        }

        let unique: HashSet<usize> = all_addresses.iter().copied().collect();

        assert_eq!(all_addresses.len(), THREADS * PER_THREAD);
        assert_eq!(unique.len(), all_addresses.len());

        for address in all_addresses {
            assert_eq!(address % 8, 0);
        }
    }
}
