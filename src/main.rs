use std::alloc::{GlobalAlloc, Layout, System};
use std::time::Instant;
use vanta_rust_bench::DeterministicAllocator;

fn main() {
    const ITERATIONS: usize = 100_000;
    const ARENA_SIZE: usize = ITERATIONS * 128;

    // Keep the large arena on the heap, not the stack.
    let arena = Box::new(DeterministicAllocator::<ARENA_SIZE>::new());

    let layout = Layout::from_size_align(64, 8)
        .expect("Invalid allocation layout");

    let start = Instant::now();

    for _ in 0..ITERATIONS {
        let ptr = unsafe { System.alloc(layout) };

        if ptr.is_null() {
            panic!("System allocation failed");
        }

        unsafe {
            System.dealloc(ptr, layout);
        }
    }

    let system_elapsed = start.elapsed();

    let start = Instant::now();

    for _ in 0..ITERATIONS {
        arena.allocate(layout).expect("Arena exhausted");
    }

    let arena_elapsed = start.elapsed();

    println!("VANTACORE SYSTEMS — ALLOCATION BENCHMARK");
    println!("Iterations:       {ITERATIONS}");
    println!("System allocator: {system_elapsed:?}");
    println!("Arena allocator:  {arena_elapsed:?}");
    println!("Arena used:       {} bytes", arena.used());
    println!("Arena capacity:   {} bytes", arena.capacity());
    println!("Timings are local measurements.");
}
