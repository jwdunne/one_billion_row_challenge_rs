use onebrc::byte_buffer::ByteBuffer;
use onebrc::hash_table::Table;
use std::{
    env,
    fs::File,
    io::{self, BufReader, Read},
};

const DOT_BITS: u64 = 0x10101000;
const MAGIC_MULTIPLIER: u64 = 100 * 0x1000000 + 10 * 0x10000 + 1;

#[inline(always)]
fn parse_temp(bytes: &[u8]) -> i16 {
    let n = unsafe { (bytes.as_ptr() as *const u64).read_unaligned() } as u64;
    let n = n & (1 << (bytes.len() * 8)) - 1;

    let dot = (!n & DOT_BITS).trailing_zeros();
    let sign = (((!n) << 59) as i64 >> 63) as u64;
    let mask = !(sign & 0xff);
    let digits = ((n & mask) << (28 - dot)) & 0xf000f0f00;
    let abs = ((digits * MAGIC_MULTIPLIER) >> 32) & 0x3FF;
    ((abs ^ sign).wrapping_sub(sign)) as i16
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_branchless_parse_temp() {
        let temp1 = "-90.1".as_bytes();
        let temp2 = "-9.1".as_bytes();
        let temp3 = "90.1".as_bytes();
        let temp4 = "9.1".as_bytes();

        assert_eq!(parse_temp(temp1), -901);
        assert_eq!(parse_temp(temp2), -91);
        assert_eq!(parse_temp(temp3), 901);
        assert_eq!(parse_temp(temp4), 91);
    }

    #[test]
    fn test_arrayless_parse_temp() {
        let temp1 = "-90.1".as_bytes();
        let temp2 = "-9.1".as_bytes();
        let temp3 = "90.1".as_bytes();
        let temp4 = "9.1".as_bytes();

        assert_eq!(parse_temp2(temp1), parse_temp(temp1));
        assert_eq!(parse_temp2(temp2), parse_temp(temp2));
        assert_eq!(parse_temp2(temp3), parse_temp(temp3));
        assert_eq!(parse_temp2(temp4), parse_temp(temp4));
    }

    fn parse_temp2(bytes: &[u8]) -> i16 {
        let len = bytes.len();

        let neg_byte = unsafe { bytes.get_unchecked(0) };
        let neg = (*neg_byte == b'-') as i16;
        let neg_mask = neg.wrapping_neg();

        let dec_byte = unsafe { bytes.get_unchecked(len - 1) };
        let one_byte = unsafe { bytes.get_unchecked(len - 3) };
        let ten_byte = unsafe { bytes.get_unchecked(neg as usize) };

        let dec = (*dec_byte - b'0') as i16;
        let one = 10 * (*one_byte - b'0') as i16;

        let ten_mask = (((len as i16 - neg) == 4) as u8).wrapping_neg();
        let ten = 100 * ((*ten_byte - b'0') & ten_mask) as i16;

        ((ten + one + dec) ^ neg_mask) - neg_mask
    }
}
