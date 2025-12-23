use std::env;
use std::fs;
use std::str::FromStr;

use rand::Rng;
use rand_distr::Distribution;
use rand_distr::Normal;

#[derive(Debug)]
struct City {
    name: String,
    distribution: Normal<f64>,
}

impl City {
    fn new(name: &str, mean: f64) -> Self {
        Self {
            name: name.to_string(),
            distribution: Normal::new(mean, 10.0)
                .unwrap_or_else(|_| panic!("could not create normal distribution for: {}", name)),
        }
    }

    fn sample(&self, rng: &mut impl Rng) -> f64 {
        self.distribution.sample(rng).max(-99.9).min(99.9)
    }
}

#[derive(Debug)]
struct ParseCityError;

impl FromStr for City {
    type Err = ParseCityError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (name, mean_str) = s.split_once(';').ok_or(ParseCityError)?;
        let mean: f64 = mean_str.parse().map_err(|_| ParseCityError)?;
        Ok(City::new(name, mean))
    }
}

fn main() {
    let num: u32 = env::args()
        .nth(1)
        .expect("single argument n expected")
        .replace("_", "")
        .parse()
        .expect("expected int argument");

    let cities: Vec<City> = fs::read_to_string("data/weather_stations.csv")
        .expect("could not read weather_data.csv")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            line.parse()
                .unwrap_or_else(|_| panic!("failed to parse: {}", line))
        })
        .collect();

    let mut rng = rand::rng();

    for _ in 0..(num / 1000) {
        let mut lines: Vec<String> = vec![];
        for _ in 0..1000 {
            let city = &cities[rng.random_range(0..cities.len())];
            let temp = city.sample(&mut rng);
            lines.push(format!("{};{:.1}", city.name, temp));
        }
        println!("{}", lines.join("\n"));
    }
}
