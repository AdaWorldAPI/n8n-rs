use criterion::{black_box, criterion_group, criterion_main, Criterion};
use n8n_hamming::HammingVector;

fn bench_distance(c: &mut Criterion) {
    let v1 = HammingVector::from_seed("benchmark_vector_1");
    let v2 = HammingVector::from_seed("benchmark_vector_2");

    c.bench_function("hamming_distance", |b| {
        b.iter(|| black_box(v1.distance(&v2)))
    });
}

fn bench_bind(c: &mut Criterion) {
    let v1 = HammingVector::from_seed("benchmark_vector_1");
    let v2 = HammingVector::from_seed("benchmark_vector_2");

    c.bench_function("hamming_bind", |b| {
        b.iter(|| black_box(v1.bind(&v2)))
    });
}

fn bench_from_seed(c: &mut Criterion) {
    c.bench_function("hamming_from_seed", |b| {
        b.iter(|| black_box(HammingVector::from_seed("benchmark_seed")))
    });
}

fn bench_search(c: &mut Criterion) {
    let mut index = n8n_hamming::HammingIndex::new();
    for i in 0..10000 {
        index.insert(format!("vec_{}", i), HammingVector::from_seed(&format!("seed_{}", i)));
    }
    let query = HammingVector::from_seed("query");

    c.bench_function("hamming_search_10k", |b| {
        b.iter(|| black_box(index.search(&query, 10, None)))
    });
}

criterion_group!(benches, bench_distance, bench_bind, bench_from_seed, bench_search);
criterion_main!(benches);
