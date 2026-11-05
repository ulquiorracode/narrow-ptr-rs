use core::hint::black_box;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use narrow_ptr::RelPtr;

const NODE_COUNTS: [usize; 3] = [10_000, 100_000, 1_000_000];

// Standard 64-bit absolute pointer node
#[repr(C)]
struct AbsNode {
    value: u32,
    _pad: u32,
    next: *const AbsNode,
}

// Compact 32-bit relative pointer node (narrow-ptr)
#[repr(C)]
struct NarrowNode {
    value: u32,
    next: RelPtr<NarrowNode, i32>,
}

fn bench_ptr_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("pointer_traversal_and_footprint");

    // Print static footprint comparison
    println!("=== MEMORY FOOTPRINT COMPARISON ===");
    println!(
        "  AbsNode (64-bit raw pointer):    {} bytes",
        core::mem::size_of::<AbsNode>()
    );
    println!(
        "  NarrowNode (32-bit RelPtr):      {} bytes",
        core::mem::size_of::<NarrowNode>()
    );
    println!(
        "  Memory Savings per node:         {}%",
        (1.0 - (core::mem::size_of::<NarrowNode>() as f64
            / core::mem::size_of::<AbsNode>() as f64))
            * 100.0
    );
    println!("=====================================");

    for &count in &NODE_COUNTS {
        // 1. Setup absolute pointer array
        let mut abs_nodes = Vec::with_capacity(count);
        for i in 0..count {
            abs_nodes.push(AbsNode {
                value: i as u32,
                _pad: 0,
                next: core::ptr::null(),
            });
        }
        for i in 0..count - 1 {
            let next_ptr = &abs_nodes[i + 1] as *const AbsNode;
            abs_nodes[i].next = next_ptr;
        }

        // 2. Setup narrow pointer array
        let mut narrow_nodes = Vec::with_capacity(count);
        for i in 0..count {
            narrow_nodes.push(NarrowNode {
                value: i as u32,
                next: RelPtr::from_raw_offset(0),
            });
        }
        for i in 0..count - 1 {
            let curr_ptr = &narrow_nodes[i].next as *const RelPtr<NarrowNode, i32>;
            let next_ptr = &narrow_nodes[i + 1] as *const NarrowNode;
            narrow_nodes[i].next = RelPtr::new(curr_ptr, next_ptr);
        }

        // Benchmark absolute traversal
        group.bench_function(BenchmarkId::new("traversal_abs_ptr_64", count), |b| {
            b.iter(|| {
                let mut current = abs_nodes.as_ptr();
                let mut sum = 0u64;
                while !current.is_null() {
                    unsafe {
                        sum += (*current).value as u64;
                        current = (*current).next;
                    }
                }
                black_box(sum);
            });
        });

        // Benchmark narrow relative traversal
        group.bench_function(
            BenchmarkId::new("traversal_narrow_rel_ptr_32", count),
            |b| {
                b.iter(|| {
                    let mut current = narrow_nodes.as_ptr();
                    let mut sum = 0u64;
                    let mut visited = 0;
                    while visited < count {
                        unsafe {
                            sum += (*current).value as u64;
                            if visited + 1 < count {
                                current = (*current).next.as_ptr();
                            }
                            visited += 1;
                        }
                    }
                    black_box(sum);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_ptr_comparison);
criterion_main!(benches);
