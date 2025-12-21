use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use hashbrown::HashMap;
use onebrc::hash_table::Table;
use pprof::criterion::{Output, PProfProfiler};
use std::{fs::read_to_string, hint::black_box};

fn bench_lookup_collisions(c: &mut Criterion) {
    let names: Vec<_> = read_to_string("data/weather_stations.csv")
        .expect("could not read data/weather_stations.csv")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| line.split_once(';').unwrap().0.to_string())
        .collect();

    let slots = 1 << 16;
    let mut table = Table::new(slots);

    let mut by_depth: HashMap<usize, (String, u64, u64)> = HashMap::new();

    for name in &names {
        let name_bytes = name.as_bytes();
        let (hash, prefix) = Table::hash(name_bytes);
        let ideal_slot = hash as usize & (slots - 1);
        let actual_slot = table.lookup(hash, prefix);
        let depth = (actual_slot + slots - ideal_slot) & (slots - 1);

        by_depth
            .entry(depth)
            .or_insert_with(|| (name.clone(), hash, prefix));

        table.update(actual_slot, hash, prefix, name_bytes, 0);
    }

    // Find a guaranteed miss
    let miss_hash = 0xDEADBEEFCAFEBABE_u64;
    let miss_prefix = 0u64;

    // Get examples for each scenario
    let (_, hit_hash_0, hit_prefix_0) = by_depth.get(&0).expect("no depth 0");
    let (_, hit_hash_1, hit_prefix_1) = by_depth.get(&1).expect("no depth 1");
    let max_depth = *by_depth.keys().max().unwrap();
    let (name, hit_hash_max, hit_prefix_max) = by_depth.get(&max_depth).expect("no max depth");

    println!("Max depth: {} ({})", max_depth, name);

    let mut group = c.benchmark_group("lookup");

    group.bench_function("miss", |b| {
        b.iter(|| table.lookup(black_box(miss_hash), black_box(miss_prefix)))
    });

    group.bench_function("hit_depth_0", |b| {
        b.iter(|| table.lookup(black_box(*hit_hash_0), black_box(*hit_prefix_0)))
    });

    group.bench_function("hit_depth_1", |b| {
        b.iter(|| table.lookup(black_box(*hit_hash_1), black_box(*hit_prefix_1)))
    });

    group.bench_function(&format!("hit_depth_{}", max_depth), |b| {
        b.iter(|| table.lookup(black_box(*hit_hash_max), black_box(*hit_prefix_max)))
    });

    group.finish();
}

fn bench_update(c: &mut Criterion) {
    let names: Vec<_> = read_to_string("data/weather_stations.csv")
        .expect("could not read data/weather_stations.csv")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| line.split_once(';').unwrap().0.to_string())
        .collect();

    let slots = 1 << 16;
    let mut table = Table::new(slots);

    for name in &names {
        let name_bytes = name.as_bytes();
        let (hash, prefix) = Table::hash(name_bytes);
        let slot = table.lookup(hash, prefix);
        table.update(slot, hash, prefix, name_bytes, 0);
    }

    let name = names[0].as_bytes();
    let (hash, prefix) = Table::hash(name);
    let slot = table.lookup(hash, prefix);

    let mut group = c.benchmark_group("update");

    group.bench_function("existing_entry", |b| {
        b.iter(|| {
            table.update(
                black_box(slot),
                black_box(hash),
                black_box(prefix),
                black_box(name),
                black_box(42),
            )
        })
    });

    group.finish();
}

fn bench_lookup_and_update(c: &mut Criterion) {
    let names: Vec<_> = read_to_string("data/weather_stations.csv")
        .expect("could not read data/weather_stations.csv")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| line.split_once(';').unwrap().0.to_string())
        .collect();

    let slots = 1 << 16;
    let mut table = Table::new(slots);

    for name in &names {
        let name_bytes = name.as_bytes();
        let (hash, prefix) = Table::hash(name_bytes);
        let slot = table.lookup(hash, prefix);
        table.update(slot, hash, prefix, name_bytes, 0);
    }

    let name = names[0].as_bytes();
    let (hash, prefix) = Table::hash(name);

    let mut group = c.benchmark_group("combined");

    group.bench_function("lookup_then_update", |b| {
        b.iter(|| {
            let slot = table.lookup(black_box(hash), black_box(prefix));
            table.update(
                black_box(slot),
                black_box(hash),
                black_box(prefix),
                black_box(name),
                black_box(42),
            )
        })
    });

    group.finish();
}

fn bench_hash(c: &mut Criterion) {
    let lengths: &[usize] = &[2, 4, 8, 9, 12, 16, 24, 32, 49];

    let names: Vec<Vec<u8>> = lengths
        .iter()
        .map(|&len| (0..len).map(|i| b'A' + (i % 26) as u8).collect())
        .collect();

    let mut group = c.benchmark_group("hash");

    for (i, name) in names.iter().enumerate() {
        group.throughput(criterion::Throughput::Bytes(name.len() as u64));
        group.bench_with_input(BenchmarkId::new("current", lengths[i]), name, |b, name| {
            b.iter(|| Table::hash(black_box(name)))
        });
    }
}

fn bench_prefix(c: &mut Criterion) {
    let lengths: &[usize] = &[2, 4, 8, 9, 12, 16, 24, 32, 49];

    let names: Vec<Vec<u8>> = lengths
        .iter()
        .map(|&len| (0..len).map(|i| b'A' + (i % 26) as u8).collect())
        .collect();

    let mut group = c.benchmark_group("prefix");

    for (i, name) in names.iter().enumerate() {
        group.throughput(criterion::Throughput::Bytes(name.len() as u64));
        group.bench_with_input(BenchmarkId::new("current", lengths[i]), name, |b, name| {
            b.iter(|| Table::prefix(black_box(name)))
        });
    }
}

fn bench_realistic_access(c: &mut Criterion) {
    let names: Vec<_> = read_to_string("data/weather_stations.csv")
        .expect("could not read data/weather_stations.csv")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| line.split_once(';').unwrap().0.to_string())
        .collect();
    let slots = 1 << 16;
    let mut table = Table::new(slots);

    let prepared: Vec<_> = names
        .iter()
        .map(|n| {
            let b = n.as_bytes();
            let (hash, prefix) = Table::hash(b);
            let slot = table.lookup(hash, prefix);
            table.update(slot, hash, prefix, b, 0);
            (hash, prefix, b.to_vec())
        })
        .collect();

    c.bench_function("realistic_cycle", |b| {
        let mut i = 0;
        b.iter(|| {
            let (hash, prefix, ref name) = prepared[i % prepared.len()];
            let slot = table.lookup(black_box(hash), black_box(prefix));
            table.update(slot, hash, prefix, name, 42);
            i += 1;
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)));
    targets = bench_lookup_collisions, bench_update, bench_lookup_and_update, bench_hash, bench_prefix, bench_realistic_access
}

criterion_main!(benches);
