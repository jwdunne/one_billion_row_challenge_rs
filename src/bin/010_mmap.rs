use memmap2::Mmap;
use onebrc::byte_buffer::ByteBuffer;
use onebrc::hash_table::Table;
use std::{
    env,
    fs::File,
    io::{self},
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
    let buf = unsafe { Mmap::map(&file)? };

    let mut tbl: Table = Table::new(1 << 16);

    let last_newline = buf.iter().rposition(|&b| b == b'\n').unwrap_or(0);
    let effective_buf = &buf[..last_newline];
    let buf_end = effective_buf.len();

    let q1 = effective_buf[..buf_end / 3]
        .iter()
        .rposition(|&b| b == b'\n')
        .unwrap_or(0);

    let q2 = effective_buf[..(buf_end / 3) * 2]
        .iter()
        .rposition(|&b| b == b'\n')
        .unwrap_or(0);

    let region_a = &effective_buf[..q1];
    let region_b = &effective_buf[q1 + 1..q2];
    let region_c = &effective_buf[q2 + 1..buf_end];

    let mut cursor_a = 0;
    let mut cursor_b = 0;
    let mut cursor_c = 0;
    let mut cursor_d = 0;

    while cursor_a < region_a.len() && cursor_b < region_b.len() && cursor_c < region_c.len() {
        let end_a = cursor_a + 32.min(region_a.len() - cursor_a);
        let end_b = cursor_b + 32.min(region_b.len() - cursor_b);
        let end_c = cursor_c + 32.min(region_c.len() - cursor_c);

        let window_a = &region_a[cursor_a..end_a];
        let window_b = &region_b[cursor_b..end_b];
        let window_c = &region_c[cursor_c..end_c];

        let (mut semi_a, mut nl_a) = window_a.find_delimiters();
        let (mut semi_b, mut nl_b) = window_b.find_delimiters();
        let (mut semi_c, mut nl_c) = window_c.find_delimiters();

        if nl_a == 0 {
            cursor_a = process_long_line(region_a, &mut tbl, cursor_a, region_a.len());
            continue;
        }

        if nl_b == 0 {
            cursor_b = process_long_line(region_b, &mut tbl, cursor_b, region_b.len());
            continue;
        }

        if nl_c == 0 {
            cursor_c = process_long_line(region_c, &mut tbl, cursor_c, region_c.len());
            continue;
        }

        let mut line_cursor_a = 0;
        let mut line_cursor_b = 0;
        let mut line_cursor_c = 0;
        let mut line_cursor_d = 0;

        while nl_a != 0 && nl_b != 0 && nl_c != 0 {
            let semi_pos_a = semi_a.trailing_zeros() as usize;
            let semi_pos_b = semi_b.trailing_zeros() as usize;
            let semi_pos_c = semi_c.trailing_zeros() as usize;
            let nl_pos_a = nl_a.trailing_zeros() as usize;
            let nl_pos_b = nl_b.trailing_zeros() as usize;
            let nl_pos_c = nl_c.trailing_zeros() as usize;

            let name_a = unsafe { window_a.get_unchecked(line_cursor_a..semi_pos_a) };
            let name_b = unsafe { window_b.get_unchecked(line_cursor_b..semi_pos_b) };
            let name_c = unsafe { window_c.get_unchecked(line_cursor_c..semi_pos_c) };
            let temp_a = unsafe { window_a.get_unchecked(semi_pos_a + 1..nl_pos_a) };
            let temp_b = unsafe { window_b.get_unchecked(semi_pos_b + 1..nl_pos_b) };
            let temp_c = unsafe { window_c.get_unchecked(semi_pos_c + 1..nl_pos_c) };

            let (hash_a, prefix_a) = Table::hash(name_a);
            let (hash_b, prefix_b) = Table::hash(name_b);
            let (hash_c, prefix_c) = Table::hash(name_c);

            tbl.prefetch(hash_a);
            tbl.prefetch(hash_b);
            tbl.prefetch(hash_c);

            let parsed_temp_a = parse_temp(temp_a);
            let parsed_temp_b = parse_temp(temp_b);
            let parsed_temp_c = parse_temp(temp_c);

            let slot_a = tbl.lookup(hash_a, prefix_a);
            let slot_b = tbl.lookup(hash_b, prefix_b);
            let slot_c = tbl.lookup(hash_c, prefix_c);

            tbl.update(slot_a, hash_a, prefix_a, name_a, parsed_temp_a);
            tbl.update(slot_b, hash_b, prefix_b, name_b, parsed_temp_b);
            tbl.update(slot_c, hash_c, prefix_c, name_c, parsed_temp_c);

            semi_a &= semi_a - 1;
            semi_b &= semi_b - 1;
            semi_c &= semi_c - 1;

            nl_a &= nl_a - 1;
            nl_b &= nl_b - 1;
            nl_c &= nl_c - 1;

            line_cursor_a = nl_pos_a + 1;
            line_cursor_b = nl_pos_b + 1;
            line_cursor_c = nl_pos_c + 1;
        }

        while nl_a != 0 {
            line_cursor_a = process_line(window_a, &mut tbl, line_cursor_a, semi_a, nl_a);
            semi_a &= semi_a - 1;
            nl_a &= nl_a - 1;
        }

        while nl_b != 0 {
            line_cursor_b = process_line(window_b, &mut tbl, line_cursor_b, semi_b, nl_b);
            semi_b &= semi_b - 1;
            nl_b &= nl_b - 1;
        }

        while nl_c != 0 {
            line_cursor_c = process_line(window_c, &mut tbl, line_cursor_c, semi_c, nl_c);
            semi_c &= semi_c - 1;
            nl_c &= nl_c - 1;
        }

        cursor_a += line_cursor_a;
        cursor_b += line_cursor_b;
        cursor_c += line_cursor_c;
        cursor_d += line_cursor_d;
    }

    cleanup_region(&mut tbl, region_a, cursor_a);
    cleanup_region(&mut tbl, region_b, cursor_b);
    cleanup_region(&mut tbl, region_c, cursor_c);

    let mut entries = tbl.entries();

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

#[inline(always)]
fn cleanup_region(tbl: &mut Table, region: &[u8], cursor: usize) {
    let mut cursor = cursor;

    while cursor < region.len() {
        let end_a = cursor + 32.min(region.len() - cursor);
        let window_a = &region[cursor..end_a];
        let (mut semi_a, mut nl_a) = window_a.find_delimiters();

        if nl_a == 0 {
            cursor = process_long_line(region, tbl, cursor, region.len());
            continue;
        }

        let mut line_cursor_a = 0;

        while nl_a != 0 {
            line_cursor_a = process_line(window_a, tbl, line_cursor_a, semi_a, nl_a);
            semi_a &= semi_a - 1;
            nl_a &= nl_a - 1;
        }

        cursor += line_cursor_a;
    }
}

#[inline(always)]
fn process_long_line(buf: &[u8], tbl: &mut Table, start: usize, end: usize) -> usize {
    let semi_pos = buf[start..].byte_position(b';').unwrap();
    let nl_pos = buf[start + semi_pos + 1..]
        .byte_position(b'\n')
        .unwrap_or(end - (start + semi_pos + 1));

    let name = &buf[start..start + semi_pos];
    let temp = parse_temp(&buf[start + semi_pos + 1..start + semi_pos + 1 + nl_pos]);

    let (hash, prefix) = Table::hash(name);
    let slot = tbl.lookup(hash, prefix);
    tbl.update(slot, hash, prefix, name, temp);

    start + semi_pos + 1 + nl_pos + 1
}

#[inline(always)]
fn process_line(buf: &[u8], tbl: &mut Table, start: usize, semi: u32, nl: u32) -> usize {
    let semi_pos = semi.trailing_zeros() as usize;
    let nl_pos = nl.trailing_zeros() as usize;

    let name = unsafe { buf.get_unchecked(start..semi_pos) };
    let temp = unsafe { buf.get_unchecked(semi_pos + 1..nl_pos) };
    let (hash, prefix) = Table::hash(name);
    tbl.prefetch(hash);

    let parsed_temp = parse_temp(temp);

    let slot = tbl.lookup(hash, prefix);
    tbl.update(slot, hash, prefix, name, parsed_temp);

    nl_pos + 1
}
