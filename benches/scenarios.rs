//! Monte Carlo scenario generation throughput (native ChaCha20 path).

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use nanobook::scenarios::{ModelVersion, ValuationParams, monte_carlo_stock_valuation};
use std::hint::black_box;

fn bench_mc(c: &mut Criterion) {
    let params = ValuationParams::default();
    let mut group = c.benchmark_group("monte_carlo_native");

    for n_paths in [5_000, 50_000] {
        group.bench_with_input(
            BenchmarkId::new("advanced", n_paths),
            &n_paths,
            |b, &n_paths| {
                b.iter(|| {
                    black_box(
                        monte_carlo_stock_valuation(
                            "XYZ".to_string(),
                            74.0,
                            ModelVersion::Advanced,
                            n_paths,
                            1.0,
                            42,
                            0.18,
                            0.38,
                            params,
                            0.08,
                            None,
                            None,
                        )
                        .unwrap(),
                    )
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_mc);
criterion_main!(benches);
