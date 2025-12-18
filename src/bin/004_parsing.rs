use hashbrown::HashMap;
use std::{
    env,
    fs::File,
    io::{self, BufReader, Read},
};

struct Stats {
    min: i32,
    max: i32,
    sum: i32,
    count: i32,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            min: i32::MAX,
            max: i32::MIN,
            sum: 0,
            count: 0,
        }
    }
}

fn parse_temp(bytes: &[u8]) -> i32 {
    let (neg, rest) = if bytes[0] == b'-' {
        (true, &bytes[1..])
    } else {
        (false, bytes)
    };

    let value = match rest.len() {
        3 => (rest[0] - b'0') as i32 * 10 + (rest[2] - b'0') as i32,
        4 => (rest[0] - b'0') as i32 * 100 + (rest[1] - b'0') as i32 * 10 + (rest[3] - b'0') as i32,
        5 => {
            (rest[0] - b'0') as i32 * 1000
                + (rest[1] - b'0') as i32 * 100
                + (rest[2] - b'0') as i32 * 10
                + (rest[3] - b'0') as i32
        }
        _ => panic!("Unexpected bytes: {:?}", rest),
    };

    if neg { -value } else { value }
}

fn main() -> io::Result<()> {
    let filename = env::args().nth(1).expect("expected filename argument");

    let file = File::open(filename).unwrap();
    let mut reader = BufReader::new(file);
    let mut stations: HashMap<Vec<u8>, Stats> = HashMap::new();

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

            if let Some(semicolon_pos) = line.iter().position(|&b| b == b';') {
                let temp = parse_temp(&line[semicolon_pos + 1..]);
                let station = stations.entry_ref(&line[..semicolon_pos]).or_default();
                station.min = temp.min(station.min);
                station.max = temp.max(station.max);
                station.sum += temp;
                station.count += 1;
            } else {
                panic!("Cannot find ; delimiter in line: {:?}", line);
            }
        }

        rem_len = filled - last_newline - 1;
        rem[..rem_len].copy_from_slice(&buf[last_newline + 1..filled]);
    }

    let mut entries: Vec<_> = stations.iter().collect();

    entries.sort_unstable_by_key(|station| station.0);

    print!("{{");
    for (i, (key, stats)) in entries.iter().enumerate() {
        let name = str::from_utf8(key).unwrap();
        let separator = if i != entries.len() - 1 { ", " } else { "" };
        print!(
            "{name}={min:.1}/{mean:.1}/{max:.1}{separator}",
            min = stats.min as f64 / 10.0,
            mean = (stats.sum as f64 / stats.count as f64) / 10.0,
            max = stats.max as f64 / 10.0
        );
    }
    println!("}}");

    Ok(())
}
