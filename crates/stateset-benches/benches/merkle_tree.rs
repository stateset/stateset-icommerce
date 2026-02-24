use criterion::{Criterion, black_box, criterion_group, criterion_main};
use stateset_crypto::merkle::compute_merkle_root;

/// Generate `n` deterministic 32-byte leaf hashes.
///
/// Each leaf is derived from its index to ensure reproducibility across runs.
fn generate_leaves(n: usize) -> Vec<[u8; 32]> {
    (0..n)
        .map(|i| {
            let mut leaf = [0u8; 32];
            let bytes = (i as u64).to_le_bytes();
            leaf[..8].copy_from_slice(&bytes);
            // Fill remaining bytes with a simple pattern
            for (j, byte) in leaf[8..].iter_mut().enumerate() {
                *byte = ((i + j) & 0xFF) as u8;
            }
            leaf
        })
        .collect()
}

fn bench_merkle_tree(c: &mut Criterion) {
    let mut group = c.benchmark_group("merkle_tree");

    for size in [10, 100, 1_000, 10_000] {
        let leaves = generate_leaves(size);

        group.bench_function(format!("merkle_{size}"), |bencher| {
            bencher.iter(|| compute_merkle_root(black_box(&leaves)));
        });
    }

    group.finish();
}

criterion_group!(benches, bench_merkle_tree);
criterion_main!(benches);
