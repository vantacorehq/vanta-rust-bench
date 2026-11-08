use std::alloc::{GlobalAlloc, Layout};
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Fixed-capacity atomic bump allocator.
pub struct DeterministicAllocator<const N: usize> {
    cursor: AtomicUsize,
    storage: UnsafeCell<[MaybeUninit<u8>; N]>,
}

// Successful atomic reservations never overlap.
unsafe impl<const N: usize> Sync for DeterministicAllocator<N> {}

impl<const N: usize> DeterministicAllocator<N> {
    pub const fn new() -> Self {
        Self {
            cursor: AtomicUsize::new(0),
            storage: UnsafeCell::new([MaybeUninit::uninit(); N]),
        }
    }

    pub const fn capacity(&self) -> usize {
        N
    }

    pub fn used(&self) -> usize {
        self.cursor.load(Ordering::Relaxed)
    }

    pub fn remaining(&self) -> usize {
        N.saturating_sub(self.used())
    }

    /// Reserve an aligned region.
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
                    // The reserved range is within the arena.
                    let ptr = unsafe { base.add(offset) };
                    return NonNull::new(ptr);
                }
                Err(actual) => current = actual,
            }
        }
    }

    /// Reset only when no allocations from this arena are in use.
    ///
    /// # Safety
    /// The caller must ensure that no thread accesses any previously
    /// allocated region while the arena is being reset.
    pub unsafe fn reset(&self) {
        self.cursor.store(0, Ordering::Release);
    }
}

impl<const N: usize> Default for DeterministicAllocator<N> {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl<const N: usize> GlobalAlloc for DeterministicAllocator<N> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.allocate(layout)
            .map_or(std::ptr::null_mut(), |ptr| ptr.as_ptr())
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Individual deallocation is intentionally unsupported.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
                        .expect("arena capacity exhausted");

                    addresses.push(allocation.as_ptr() as usize);
                }

                addresses
            }));
        }

        let mut all_addresses = Vec::new();

        for handle in handles {
            all_addresses.extend(handle.join().unwrap());
        }

        let unique: HashSet<usize> =
            all_addresses.iter().copied().collect();

        assert_eq!(all_addresses.len(), THREADS * PER_THREAD);
        assert_eq!(unique.len(), all_addresses.len());

        for address in all_addresses {
            assert_eq!(address % 8, 0);
        }
    }
}


    


        
       

        
   

        f
