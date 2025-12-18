use hashbrown::HashMap;
use std::{
    env,
    fs::File,
    io::{self, BufRead, BufReader, Read},
    str::Utf8Error,
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

fn main() -> io::Result<()> {
    let filename = env::args().nth(1).expect("expected filename argument");

    let file = File::open(filename).unwrap();
    let mut reader = BufReader::new(file);
    let mut stations: HashMap<String, Stats> = HashMap::new();

    let mut buf = [0u8; 4 << 20];
    let mut rem_len = 0;
    let mut rem = [0u8; 64];

    loop {
        buf[..rem_len].copy_from_slice(&rem[..rem_len]);

        let bytes_read = reader.read(&mut buf[rem_len..])?;

        if bytes_read == 0 {
            break;
        }

        let filled = rem_len + bytes_read;

        let last_newline = buf[..filled].iter().rposition(|&b| b == b'\n').unwrap_or(0);

        for line in buf[..last_newline].split(|&b| b == b'\n') {
            if line.is_empty() {
                continue;
            }

            if let Ok(s) = str::from_utf8(line) {
                let (name, temp) = s.split_once(';').unwrap();
                let temp: f64 = temp.parse().unwrap();

                let station = stations.entry_ref(name).or_default();
                station.min = temp.min(station.min);
                station.max = temp.max(station.max);
                station.sum += temp;
                station.count += 1.0;
            } else {
                panic!("Failed to convert bytes to UTF-8 str: {:?}", line);
            }
        }

        rem_len = filled - last_newline - 1;
        rem[..rem_len].copy_from_slice(&buf[last_newline + 1..filled]);
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

    Ok(())
}
