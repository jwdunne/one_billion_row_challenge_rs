use std::fs::read_to_string;

fn main() {
    let names: Vec<_> = read_to_string("data/weather_stations.csv")
        .expect("could not read data/weather_stations.csv")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| line.split_once(';').unwrap())
        .map(|(name, _)| name.to_string())
        .collect();

    let mut min = usize::MAX;
    let mut max = usize::MIN;
    let mut total = 0;
    for name in &names {
        min = min.min(name.len());
        max = max.max(name.len());
        total += name.len();
    }

    println!(
        "min={min} max={max} mean={mean}",
        mean = (total / &names.len())
    );
}
