use hashbrown::HashMap;
use std::{
    env,
    fs::File,
    io::{BufRead, BufReader},
};

struct Stats {
    min: f64,
    max: f64,
    sum: f64,
    count: f64,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            min: f64::MAX,
            max: f64::MIN,
            sum: 0.0,
            count: 0.0,
        }
    }
}

fn main() {
    let filename = env::args().nth(1).expect("expected filename argument");

    let file = File::open(filename).unwrap();
    let reader = BufReader::new(file);
    let mut stations: HashMap<String, Stats> = HashMap::new();

    for line in reader.lines() {
        let line = line.unwrap();

        if line.starts_with('#') {
            continue;
        }

        let (name, temp) = line.split_once(';').unwrap();
        let temp: f64 = temp.parse().unwrap();

        let station = stations.entry_ref(name).or_default();

        station.min = temp.min(station.min);
        station.max = temp.max(station.max);
        station.sum = temp + station.sum;
        station.count += 1.0;
    }

    let mut entries: Vec<_> = stations.iter().collect();

    entries.sort_unstable_by_key(|station| station.0);

    print!("{{");
    for (i, (name, stats)) in entries.iter().enumerate() {
        let separator = if i != entries.len() - 1 { ", " } else { "" };
        print!(
            "{name}={min:.1}/{mean:.1}/{max:.1}{separator}",
            min = stats.min,
            mean = stats.sum / stats.count,
            max = stats.max
        );
    }
    println!("}}");
}
