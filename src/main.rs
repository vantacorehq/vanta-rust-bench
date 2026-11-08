use std::alloc::{GlobalAlloc, Layout};
use std::alloc::System;
use std::time::Instant;

use vanta_rust_bench::DeterministicAllocator;

fn main() {
    const ITERATIONS: usize = 100_000;
    const ARENA_SIZE: usize = ITERATIONS * 128;

    let arena = DeterministicAllocator::<ARENA_SIZE>::new();
    let layout = Layout::from_size_align(64, 8).unwrap();

    let start = Instant::now();

    for _ in 0..ITERATIONS {
        let _ = unsafe { System.alloc(layout) };
    }

    let system_elapsed = start.elapsed();

    let start = Instant::now();

    for _ in 0..ITERATIONS {
        let _ = arena.allocate(layout);
    }

    let arena_elapsed = start.elapsed();

    println!("VANTACORE SYSTEMS — ALLOCATION BENCHMARK");
    println!("----------------------------------------");
    println!("Iterations:       {ITERATIONS}");
    println!("System allocator: {system_elapsed:?}");
    println!("Arena allocator:  {arena_elapsed:?}");
    println!("Arena used:       {} bytes", arena.used());
    println!("Arena capacity:   {} bytes", arena.capacity());
    println!();
    println!("Note: timings are local measurements, not universal performance claims.");
}
