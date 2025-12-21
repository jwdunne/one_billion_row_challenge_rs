use onebrc::hash_table::Table;
use std::{
    env,
    fs::File,
    io::{self, BufReader, Read},
};

fn parse_temp(bytes: &[u8]) -> i16 {
    let (neg, rest) = if bytes[0] == b'-' {
        (true, &bytes[1..])
    } else {
        (false, bytes)
    };

    let value = match rest.len() {
        3 => (rest[0] - b'0') as i16 * 10 + (rest[2] - b'0') as i16,
        4 => (rest[0] - b'0') as i16 * 100 + (rest[1] - b'0') as i16 * 10 + (rest[3] - b'0') as i16,
        5 => {
            (rest[0] - b'0') as i16 * 1000
                + (rest[1] - b'0') as i16 * 100
                + (rest[2] - b'0') as i16 * 10
                + (rest[3] - b'0') as i16
        }
        _ => panic!("Unexpected bytes: {:?}", rest),
    };

    if neg { -value } else { value }
}

trait ByteBuffer {
    fn byte_position(&self, needle: u8) -> Option<usize>;
}

impl ByteBuffer for [u8] {
    #[inline(always)]
    fn byte_position(&self, needle: u8) -> Option<usize> {
        let mut i = 0;

        let repeat = 0x0101_0101_0101_0101u64 * needle as u64;
        while i + 8 <= self.len() {
            let chunk = u64::from_ne_bytes(self[i..i + 8].try_into().unwrap());
            let xored = chunk ^ repeat;

            if (xored.wrapping_sub(0x0101_0101_0101_0101) & !xored & 0x8080_8080_8080_8080) != 0 {
                for j in 0..8 {
                    if self[i + j] == needle {
                        return Some(i + j);
                    }
                }
            }

            i += 8;
        }

        while i < self.len() {
            if self[i] == needle {
                return Some(i);
            }
            i += 1;
        }

        None
    }
}

fn main() -> io::Result<()> {
    let filename = env::args().nth(1).expect("expected filename argument");

    let file = File::open(filename).unwrap();
    let mut reader = BufReader::new(file);

    let mut stations: Table = Table::new(1 << 16);

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

        let mut start = 0;
        while let Some(end) = buf[start..last_newline].byte_position(b'\n') {
            let line = &buf[start..start + end];

            if line.is_empty() {
                continue;
            }

            if let Some(semicolon_pos) = line.byte_position(b';') {
                let temp = parse_temp(&line[semicolon_pos + 1..]);
                let name = &line[..semicolon_pos];
                let (hash, prefix) = Table::hash(name);
                let slot = stations.lookup(hash, prefix);
                stations.update(slot, hash, prefix, name, temp);
            } else {
                panic!(
                    "Cannot find ; delimiter in line: {:?}",
                    str::from_utf8(line).unwrap()
                );
            }

            start += end + 1;
        }

        rem_len = filled - last_newline - 1;
        rem[..rem_len].copy_from_slice(&buf[last_newline + 1..filled]);
    }

    let mut entries = stations.entries();

    entries.sort_unstable_by_key(|(name, station)| &name[..station.len as usize]);

    print!("{{");
    for (i, (name, entry)) in entries.iter().enumerate() {
        let name = str::from_utf8(&name[..entry.len as usize]).unwrap();
        let separator = if i != entries.len() - 1 { ", " } else { "" };
        print!(
            "{name}={min:.1}/{mean:.1}/{max:.1}{separator}",
            min = entry.min as f64 / 10.0,
            mean = (entry.sum as f64 / entry.count as f64) / 10.0,
            max = entry.max as f64 / 10.0
        );
    }
    println!("}}");

    Ok(())
}
