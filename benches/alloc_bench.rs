use criterion::{
    black_box,
    criterion_group,
    criterion_main,
    BatchSize,
    Criterion,
    Throughput,
};

use std::alloc::{GlobalAlloc, Layout, System};
use vanta_rust_bench::DeterministicAllocator;

const BATCH_SIZE: usize = 1024;
const ARENA_CAPACITY: usize = BATCH_SIZE * 128 + 4096;

static ARENA: DeterministicAllocator<ARENA_CAPACITY> =
    DeterministicAllocator::new();

fn allocation_layout() -> Layout {
    Layout::from_size_align(64, 8)
        .expect("allocation layout must be valid")
}

fn benchmark_system(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocator_comparison");

    group.throughput(Throughput::Elements(BATCH_SIZE as u64));

    group.bench_function("system_alloc_dealloc", |b| {
        b.iter(|| {
            let layout = allocation_layout();

            for _ in 0..BATCH_SIZE {
                let allocation = unsafe {
                    System.alloc(layout)
                };

                assert!(
                    !allocation.is_null(),
                    "system allocator returned null"
                );

                black_box(allocation);

                unsafe {
                    System.dealloc(allocation, layout);
                }
            }
        });
    });

    group.finish();
}

fn benchmark_arena(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocator_comparison");

    group.throughput(Throughput::Elements(BATCH_SIZE as u64));

    group.bench_function("deterministic_arena_alloc_dealloc", |b| {
        b.iter_batched(
            || unsafe {
                ARENA.reset();
            },
            |_| {
                let layout = allocation_layout();

                for _ in 0..BATCH_SIZE {
                    let allocation = unsafe {
                        ARENA.alloc(layout)
                    };

                    assert!(
                        !allocation.is_null(),
                        "arena capacity exhausted"
                    );

                    black_box(allocation);

                    unsafe {
                        ARENA.dealloc(allocation, layout);
                    }
                }
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_system,
    benchmark_arena
);

criterion_main!(benches);
