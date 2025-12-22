use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use onebrc::byte_buffer::ByteBuffer;

fn bench_byte_position(c: &mut Criterion) {
    let mut group = c.benchmark_group("byte_position");

    let test_cases: Vec<(&str, Vec<u8>)> = vec![
        ("len_4_pos_2", b"Xi;3.4".to_vec()),
        ("len_8_pos_4", b"Lima;5.6".to_vec()),
        ("len_12_pos_6", b"Berlin;12.3".to_vec()),
        ("len_16_pos_9", b"Melbourne;23.4".to_vec()),
        ("len_24_pos_13", b"San Francisco;-5.2".to_vec()),
        ("len_32_pos_18", b"Thiruvananthapuram;31.2".to_vec()),
        (
            "len_64_pos_45",
            b"Some Very Long Station Name That Goes On Forever;99.9".to_vec(),
        ),
    ];

    for (name, line) in &test_cases {
        group.throughput(Throughput::Bytes(line.len() as u64));
        group.bench_with_input(BenchmarkId::new("semicolon", name), line, |b, line| {
            b.iter(|| black_box(line.as_slice()).byte_position(b';'))
        });
    }

    group.finish();
}

fn bench_byte_position_worst_case(c: &mut Criterion) {
    let mut group = c.benchmark_group("byte_position_worst");

    let late: Vec<u8> = "A"
        .repeat(63)
        .into_bytes()
        .into_iter()
        .chain([b';'])
        .collect();

    let missing: Vec<u8> = "A".repeat(64).into_bytes();

    group.throughput(Throughput::Bytes(64));

    group.bench_function("needle_at_end_64", |b| {
        b.iter(|| black_box(late.as_slice()).byte_position(b';'))
    });

    group.bench_function("needle_missing_64", |b| {
        b.iter(|| black_box(missing.as_slice()).byte_position(b';'))
    });

    group.finish();
}

fn bench_byte_position_realistic(c: &mut Criterion) {
    let lines: Vec<Vec<u8>> = std::fs::read_to_string("data/weather_stations.csv")
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with('#'))
        .map(|l| l.as_bytes().to_vec())
        .collect();

    c.bench_function("byte_position_realistic_cycle", |b| {
        let mut i = 0;
        b.iter(|| {
            let line = &lines[i % lines.len()];
            let result = black_box(line).byte_position(b';');
            i += 1;
            result
        })
    });
}

fn bench_find_delimiters(c: &mut Criterion) {
    let mut group = c.benchmark_group("find_delimiters");

    let test_cases: Vec<(&str, Vec<u8>)> = vec![
        ("4_lines", b"Xi;1.2\nBo;3.4\nAb;5.6\nCd;7.8\n".to_vec()),
        ("2_lines", b"Melbourne;23.4\nSan Diego;-1.2\n".to_vec()),
        ("1_line", b"San Francisco;12.345\nPartialName".to_vec()),
        ("0_lines", b"Llanfairpwllgwyngyllgogerychwyrn".to_vec()),
    ];

    for (name, window) in &test_cases {
        group.throughput(Throughput::Bytes(window.len() as u64));
        group.bench_with_input(BenchmarkId::new("swar", name), window, |b, window| {
            b.iter(|| black_box(window.as_slice()).find_delimiters())
        });
    }

    group.finish();
}

fn bench_find_delimiters_window_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("find_delimiters_sizes");

    let line = b"Melbourne;23.4\n";
    let full: Vec<u8> = line.iter().cycle().take(64).cloned().collect();

    for size in [8, 16, 24, 32] {
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::new("swar", size),
            &full[..size],
            |b, window| b.iter(|| black_box(window).find_delimiters()),
        );
    }

    group.finish();
}

fn bench_find_delimiters_delimiter_density(c: &mut Criterion) {
    let mut group = c.benchmark_group("find_delimiters_density");

    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("0_delims", b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_vec()),
        ("2_delims", b"ThisIsAVeryLongStationName;12.3\n".to_vec()),
        ("4_delims", b"MediumName;1.2\nOtherName;3.4\nXX".to_vec()),
        ("8_delims", b"Xi;1\nBo;2\nAb;3\nCd;4\nEf;5\nGh;6\n".to_vec()),
    ];

    for (name, window) in &cases {
        group.throughput(Throughput::Bytes(32));
        group.bench_with_input(BenchmarkId::new("swar", name), window, |b, window| {
            b.iter(|| black_box(window.as_slice()).find_delimiters())
        });
    }

    group.finish();
}

fn bench_find_delimiters_realistic(c: &mut Criterion) {
    let lines: Vec<u8> = std::fs::read_to_string("data/weather_stations.csv")
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with('#'))
        .map(|l| format!("{};23.4\n", l.split_once(';').unwrap().0))
        .collect::<String>()
        .into_bytes();

    c.bench_function("find_delimiters_realistic", |b| {
        let mut i = 0;
        b.iter(|| {
            let start = i % (lines.len().saturating_sub(32));
            let end = (start + 32).min(lines.len());
            let result = black_box(&lines[start..end]).find_delimiters();
            i += 15; // Advance by ~1 line
            result
        })
    });
}

criterion_group!(
    benches,
    bench_byte_position,
    bench_byte_position_worst_case,
    bench_byte_position_realistic,
    bench_find_delimiters,
    bench_find_delimiters_realistic,
    bench_find_delimiters_window_sizes,
    bench_find_delimiters_delimiter_density
);
criterion_main!(benches);
