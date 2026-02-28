//! TREX 벤치마크
//!
//! 사용법: cargo bench

use criterion::{criterion_group, criterion_main, Criterion};

fn bench_placeholder(c: &mut Criterion) {
    // Phase 1 구현 후 실제 벤치마크로 교체
    c.bench_function("placeholder", |b| {
        b.iter(|| {
            // PDF 추출 벤치마크 예정
            let _ = 1 + 1;
        });
    });
}

criterion_group!(benches, bench_placeholder);
criterion_main!(benches);
