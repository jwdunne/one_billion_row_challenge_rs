use hashbrown::HashMap;
use onebrc::hash_table::Table;
use std::{fs::read_to_string, u64};

fn main() {
    let mut names: Vec<_> = read_to_string("data/weather_stations.csv")
        .expect("could not read data/weather_stations.csv")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| line.split_once(';').unwrap())
        .map(|(name, _)| name.to_string())
        .collect();

    names.sort_unstable();
    names.dedup();

    let mut min = usize::MAX;
    let mut max = usize::MIN;
    let mut total = 0;
    for name in &names {
        min = min.min(name.len());
        max = max.max(name.len());
        total += name.len();
    }

    for prefix_len in 1..=16 {
        let mut prefixes: HashMap<&[u8], usize> = HashMap::new();

        for name in &names {
            if name.len() >= prefix_len {
                *prefixes.entry(&name.as_bytes()[..prefix_len]).or_default() += 1;
            }
        }

        let max_sharing = prefixes.values().max().unwrap_or(&0);
        let unique = prefixes.len();
        println!("Prefix len {prefix_len}: {unique} unique prefixes; {max_sharing} max sharing");
    }

    let mut pow2 = (64_000 / 10_000) * names.len() as u32;
    pow2 -= 1;
    pow2 |= pow2 >> 1;
    pow2 |= pow2 >> 2;
    pow2 |= pow2 >> 4;
    pow2 |= pow2 >> 8;
    pow2 |= pow2 >> 16;
    pow2 += 1;

    let slots = 1 << 16;

    let mean = total / names.len();

    let stddev: f64 = names
        .iter()
        .map(|n| (n.len() as i32 - mean as i32).pow(2) as f64)
        .sum::<f64>()
        .sqrt();

    let gte_mean = names.iter().filter(|n| n.len() >= mean).count();
    let lt_mean = names.iter().filter(|n| n.len() < mean).count();

    println!(
        "min={min} max={max} mean={mean} gte_mean={gte_mean} lt_mean={lt_mean} stddev={stddev:.2} total={total} slots={slots} est={pow2}",
        mean = (total / names.len()),
        total = names.len(),
    );

    println!();
    println!("== Name length distribution ==");

    let lengths: &[usize] = &[2, 4, 8, 9, 12, 16, 24, 32, 49];
    let mut length_dist: HashMap<usize, usize> = HashMap::new();

    for name in &names {
        let len = name.len();
        for &length in lengths {
            if len < length {
                length_dist
                    .entry(length)
                    .and_modify(|e| *e += 1)
                    .or_insert(1);
            }
        }
    }

    let mut length_dist: Vec<(&usize, &usize)> = length_dist.iter().collect();
    length_dist.sort_by_key(|(l, _)| *l);

    for (length, count) in length_dist {
        println!("len < {} = {}", length, count);
    }

    let mut hash_collisions: HashMap<u64, Vec<String>> = HashMap::new();
    let mut slot_collisions: HashMap<u64, Vec<String>> = HashMap::new();

    for name in &names {
        let (hash, _) = Table::hash(name.as_bytes());

        slot_collisions
            .entry(hash & (slots - 1) as u64)
            .and_modify(|e| e.push(name.clone()))
            .or_insert_with(|| vec![name.clone()]);

        hash_collisions
            .entry(hash)
            .and_modify(|e| {
                e.push(name.clone());
            })
            .or_insert_with(|| vec![name.clone()]);
    }

    let mut slot_collisions: Vec<(&u64, &Vec<String>)> = slot_collisions.iter().collect();
    slot_collisions.sort_unstable_by_key(|&(_, cs)| -(cs.len() as i32));

    println!();
    println!(
        "Total slot collisions: {}",
        slot_collisions
            .iter()
            .filter(|(_, v)| v.len() > 1)
            .fold(0, |acc, (_, c)| acc + c.len())
    );

    println!(
        "Total hash collisions: {}",
        hash_collisions
            .iter()
            .filter(|(_, v)| v.len() > 1)
            .fold(0, |acc, (_, c)| acc + c.len())
    );

    println!("== Top 10 slot collisions ==");

    for (i, name) in slot_collisions.iter().enumerate().take(10) {
        println!("{i}. {name:?}");
    }

    let mut table = Table::new(slots);
    let mut probe_depths: HashMap<usize, usize> = HashMap::new();

    for name in &names {
        let name_bytes = name.as_bytes();
        let (hash, prefix) = Table::hash(name_bytes);
        let ideal_slot = hash as usize & (slots - 1);

        let actual_slot = table.lookup(hash, prefix);
        let depth = (actual_slot + slots - ideal_slot) & (slots - 1);

        *probe_depths.entry(depth).or_insert(0) += 1;

        table.update(actual_slot, hash, prefix, name_bytes, 0);
    }

    let mut slot_depths: Vec<(&usize, &usize)> = probe_depths.iter().collect();
    slot_depths.sort_unstable_by_key(|&(&depth, _)| -(depth as i32));

    println!();
    println!("== Probe depth distribution ==");
    for (&depth, &count) in slot_depths {
        println!("depth {}: {}", depth, count);
    }

    println!();
    println!("== Powers of 2 ==");
    for i in 10..20 {
        println!("1 << {} = {}", i, 1 << i);
    }

    println!();
    println!("== Masking ==");
    for i in 1..8 {
        let mask = (1 << (i * 8)) - 1;
        println!("{}", fmt_to_nibbles(mask));
        println!("{:#018x}", mask);
    }

    println!();
    println!("== Branchless conditions ==");

    for len in 0..16 {
        println!(
            "({len} > 8).wrapping_neg() = {:#018x}",
            ((len > 8) as u64).wrapping_neg()
        );
        println!("{len} > 8 = {:#018x}", ((len > 8) as u64));
    }

    let mask1 = (true as u64).wrapping_neg();
    let mask2 = (false as u64).wrapping_neg();
    println!("{:#018x}", (1u64 ^ mask1) - mask1);
    println!("{:#018x}", (1u64 ^ mask2) - mask2);
}

fn fmt_to_nibbles(n: u64) -> String {
    let bitstr = format!("{:0>64b}", n);
    bitstr
        .as_bytes()
        .chunks(4)
        .map(str::from_utf8)
        .collect::<Result<Vec<&str>, _>>()
        .unwrap()
        .join(" ")
}
