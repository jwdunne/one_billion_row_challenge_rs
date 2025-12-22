use onebrc::byte_buffer::ByteBuffer;
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
        _ => panic!("Unexpected bytes: {:?}", str::from_utf8(rest)),
    };

    if neg { -value } else { value }
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
        let effective_buf = &buf[..last_newline];
        let buf_end = effective_buf.len();

        let mut start = 0;

        loop {
            if start >= last_newline {
                break;
            }

            let end = start + 32.min(buf_end - start);
            let window = &effective_buf[start..end];

            let (mut semicolons, mut newlines) = window.find_delimiters();

            if newlines == 0 {
                let semicolon_pos = effective_buf[start..].byte_position(b';').unwrap();
                let newline_pos = effective_buf[start + semicolon_pos + 1..]
                    .byte_position(b'\n')
                    .unwrap_or(buf_end - (start + semicolon_pos + 1));

                let name = &effective_buf[start..start + semicolon_pos];
                let temp = parse_temp(
                    &effective_buf
                        [start + semicolon_pos + 1..start + semicolon_pos + 1 + newline_pos],
                );
                let (hash, prefix) = Table::hash(name);
                let slot = stations.lookup(hash, prefix);
                stations.update(slot, hash, prefix, name, temp);

                start = start + semicolon_pos + 1 + newline_pos + 1;
                continue;
            }

            let mut line_start = 0;

            while newlines != 0 {
                let semicolon_pos = semicolons.trailing_zeros() as usize;
                let newline_pos = newlines.trailing_zeros() as usize;

                let name = &window[line_start..semicolon_pos];
                let temp = parse_temp(&window[semicolon_pos + 1..newline_pos]);

                let (hash, prefix) = Table::hash(name);
                let slot = stations.lookup(hash, prefix);
                stations.update(slot, hash, prefix, name, temp);

                semicolons &= semicolons - 1;
                newlines &= newlines - 1;
                line_start = newline_pos + 1;
            }

            start += line_start;
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
