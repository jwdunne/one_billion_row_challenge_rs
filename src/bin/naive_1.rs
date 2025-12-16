use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
};

fn main() {
    let file = File::open("data/weather_stations.csv").unwrap();
    let reader = BufReader::new(file);
    let mut stations: HashMap<String, Vec<f64>> = HashMap::new();

    for line in reader.lines() {
        let line = line.unwrap();

        if line.starts_with('#') {
            continue;
        }

        let (name, temp) = line.split_once(';').unwrap();
        let temp: f64 = temp.parse().unwrap();
        stations.entry(name.to_string()).or_default().push(temp);
    }
}
