//! Microbenchmark (Fase R3 do `OPTIMIZATION_PLAN.md`): `ahash` vs hasher padrão
//! (SipHash) para `DashMap<String, _>` — o padrão real dos caches de
//! task/agent/output (`dds-dataspace::cache`), chaves `String` curtas (IDs
//! tipo `uuid`/`agent-<n>`).
//!
//! A Fase 2 já trocou os caches de produção para `ahash` (validado por
//! corretude — 75+ suítes verdes), mas a magnitude do ganho nunca foi medida
//! neste hardware. Não precisa de DDS real — é puro CPU/memória em processo.
//!
//! Rode com: `cargo bench -p dds-dataspace --bench cache_hasher`

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use dashmap::DashMap;
use std::hint::black_box;

const N_KEYS: usize = 10_000;

fn make_keys(n: usize) -> Vec<String> {
    (0..n)
        .map(|i| format!("task-{i:08x}-shared-waitset-roundtrip"))
        .collect()
}

fn bench_insert(c: &mut Criterion) {
    let keys = make_keys(N_KEYS);
    let mut group = c.benchmark_group("dashmap_insert");

    group.bench_with_input(
        BenchmarkId::new("sip_hash_default", N_KEYS),
        &keys,
        |b, keys| {
            b.iter(|| {
                let map: DashMap<String, u64> = DashMap::new();
                for (i, k) in keys.iter().enumerate() {
                    map.insert(black_box(k.clone()), i as u64);
                }
                black_box(map.len())
            });
        },
    );

    group.bench_with_input(BenchmarkId::new("ahash", N_KEYS), &keys, |b, keys| {
        b.iter(|| {
            let map: DashMap<String, u64, ahash::RandomState> =
                DashMap::with_hasher(ahash::RandomState::default());
            for (i, k) in keys.iter().enumerate() {
                map.insert(black_box(k.clone()), i as u64);
            }
            black_box(map.len())
        });
    });

    group.finish();
}

fn bench_lookup(c: &mut Criterion) {
    let keys = make_keys(N_KEYS);

    let sip_map: DashMap<String, u64> = DashMap::new();
    for (i, k) in keys.iter().enumerate() {
        sip_map.insert(k.clone(), i as u64);
    }
    let ahash_map: DashMap<String, u64, ahash::RandomState> =
        DashMap::with_hasher(ahash::RandomState::default());
    for (i, k) in keys.iter().enumerate() {
        ahash_map.insert(k.clone(), i as u64);
    }

    let mut group = c.benchmark_group("dashmap_lookup");

    group.bench_with_input(
        BenchmarkId::new("sip_hash_default", N_KEYS),
        &keys,
        |b, keys| {
            b.iter(|| {
                for k in keys {
                    black_box(sip_map.get(k));
                }
            });
        },
    );

    group.bench_with_input(BenchmarkId::new("ahash", N_KEYS), &keys, |b, keys| {
        b.iter(|| {
            for k in keys {
                black_box(ahash_map.get(k));
            }
        });
    });

    group.finish();
}

criterion_group!(benches, bench_insert, bench_lookup);
criterion_main!(benches);
