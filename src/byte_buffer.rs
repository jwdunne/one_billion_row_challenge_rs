const MSB_MASK: u64 = 0x8080_8080_8080_8080;
const LSB_MASK: u64 = 0x0101_0101_0101_0101;
const SEMICOLON_MASK: u64 = LSB_MASK * (b';' as u64);
const NEWLINE_MASK: u64 = LSB_MASK * (b'\n' as u64);

pub trait ByteBuffer {
    fn byte_position(&self, needle: u8) -> Option<usize>;

    fn find_delimiters(&self) -> (u32, u32);

    fn find_delimiters_swar(&self) -> (u32, u32);

    #[cfg(all(target_feature = "avx512f", target_feature = "avx512bw"))]
    fn find_delimiters64(&self) -> (u64, u64);
}

impl ByteBuffer for [u8] {
    #[inline(always)]
    fn byte_position(&self, needle: u8) -> Option<usize> {
        let mut i = 0;

        let repeat = LSB_MASK * needle as u64;
        while i + 8 <= self.len() {
            let chunk = u64::from_ne_bytes(self[i..i + 8].try_into().unwrap());
            let xored = chunk ^ repeat;
            let matching_bytes = xored.wrapping_sub(LSB_MASK) & !xored & MSB_MASK;

            if matching_bytes != 0 {
                let j = (matching_bytes.trailing_zeros() / 8) as usize;
                return Some(i + j);
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

    #[inline(always)]
    #[cfg(all(target_feature = "avx512f", target_feature = "avx512bw"))]
    fn find_delimiters64(&self) -> (u64, u64) {
        let (semi, nl) = unsafe {
            #[cfg(target_arch = "x86")]
            use std::arch::x86::{
                __m512i, _mm512_cmpeq_epi8_mask, _mm512_loadu_si512, _mm512_set1_epi8,
            };

            #[cfg(target_arch = "x86_64")]
            use std::arch::x86_64::{
                __m512i, _mm512_cmpeq_epi8_mask, _mm512_loadu_si512, _mm512_set1_epi8,
            };

            let chunk = _mm512_loadu_si512(self[..64.min(self.len())].as_ptr() as *const __m512i);
            let semi = _mm512_set1_epi8(b';' as i8);
            let nl = _mm512_set1_epi8(b'\n' as i8);
            let semi_mask = _mm512_cmpeq_epi8_mask(chunk, semi);
            let nl_mask = _mm512_cmpeq_epi8_mask(chunk, nl);

            (semi_mask, nl_mask)
        };

        (semi, nl)
    }

    #[inline(always)]
    #[cfg(target_feature = "avx2")]
    fn find_delimiters(&self) -> (u32, u32) {
        let (semicolons, newlines) = unsafe {
            #[cfg(target_arch = "x86")]
            use std::arch::x86::{
                __m256i, _mm256_cmpeq_epi8, _mm256_loadu_si256, _mm256_movemask_epi8,
                _mm256_set1_epi8,
            };

            #[cfg(target_arch = "x86_64")]
            use std::arch::x86_64::{
                __m256i, _mm256_cmpeq_epi8, _mm256_loadu_si256, _mm256_movemask_epi8,
                _mm256_set1_epi8,
            };

            let a = _mm256_loadu_si256(self[0..32.min(self.len())].as_ptr() as *const __m256i);
            let b = _mm256_set1_epi8(b';' as i8);
            let result = _mm256_cmpeq_epi8(a, b);
            let semicolons = _mm256_movemask_epi8(result);

            let a = _mm256_loadu_si256(self[0..32.min(self.len())].as_ptr() as *const __m256i);
            let b = _mm256_set1_epi8(b'\n' as i8);
            let result = _mm256_cmpeq_epi8(a, b);
            let newlines = _mm256_movemask_epi8(result);

            (semicolons, newlines)
        };

        let valid_mask = if self.len() >= 32 {
            0xFFFFFFFF
        } else {
            (1u32 << self.len()) - 1
        };

        (semicolons as u32 & valid_mask, newlines as u32 & valid_mask)
    }

    #[cfg(not(target_feature = "avx2"))]
    fn find_delimiters(&self) -> (u32, u32) {
        self.find_delimiters_swar()
    }

    fn find_delimiters_swar(&self) -> (u32, u32) {
        let mut i = 0;

        let mut semicolons = 0u32;
        let mut newlines = 0u32;

        while i + 8 <= self.len().min(32) {
            let chunk = unsafe { (self[i..i + 8].as_ptr() as *const u64).read_unaligned() };

            let semicolon_diff = chunk ^ SEMICOLON_MASK;
            let newline_diff = chunk ^ NEWLINE_MASK;

            let mut semicolon_matches =
                semicolon_diff.wrapping_sub(LSB_MASK) & !semicolon_diff & MSB_MASK;
            let mut newline_matches =
                newline_diff.wrapping_sub(LSB_MASK) & !newline_diff & MSB_MASK;

            let offset = i as u32;

            while semicolon_matches != 0 {
                semicolons |= 1 << ((semicolon_matches.trailing_zeros() / 8) + offset);
                semicolon_matches &= semicolon_matches - 1;
            }

            while newline_matches != 0 {
                newlines |= 1 << ((newline_matches.trailing_zeros() / 8) + offset);
                newline_matches &= newline_matches - 1;
            }

            i += 8;
        }

        while i < self.len().min(32) {
            match self[i] {
                b';' => semicolons |= 1 << i,
                b'\n' => newlines |= 1 << i,
                _ => (),
            }

            i += 1;
        }

        (semicolons, newlines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt_to_nibbles(n: u32) -> String {
        let bitstr = format!("{:0>32b}", n);
        bitstr
            .as_bytes()
            .chunks(4)
            .map(str::from_utf8)
            .collect::<Result<Vec<&str>, _>>()
            .unwrap()
            .join(" ")
    }

    fn expected_mask(bytes: &[u8], needle: u8) -> u32 {
        bytes
            .iter()
            .enumerate()
            .filter(|&(_, b)| b == &needle)
            .fold(0u32, |mask, (i, _)| mask | (1 << i))
    }

    fn next(newlines: u32) -> usize {
        let mut last_newline = newlines;
        while last_newline.count_ones() > 1 {
            last_newline &= last_newline - 1;
        }

        return last_newline.trailing_zeros() as usize + 1;
    }

    #[test]
    fn test_find_delimiters_equivalence() {
        let bytes = b";12.3\nAbc;4.5\nDef;6.7\nGhi;8.9\nJon;3.3\n";

        let (semicolons1, newlines1) = bytes[..32].find_delimiters();
        let (semicolons2, newlines2) = bytes[..32].find_delimiters_swar();
        assert_eq!(semicolons1, semicolons2);
        assert_eq!(newlines1, newlines2);
    }

    #[test]
    fn test_find_delimiters() {
        let lines = vec![
            "Bāgepalli;17.8",
            "San Fernando;-1.9",
            "Kika;4.3",
            "Bo;6.8",
            "Poyo;39.2",
            "Pālakodu;10.4",
            "Konibodom;40.2",
        ];
        let str = lines.join("\n");
        let bytes = str.as_bytes();

        let (semicolons, newlines) = bytes[..32].find_delimiters();
        assert_eq!(semicolons, expected_mask(&bytes[..32], b';'));
        assert_eq!(newlines, expected_mask(&bytes[..32], b'\n'));

        println!("SC: {}", fmt_to_nibbles(semicolons));
        println!("NL: {}", fmt_to_nibbles(newlines));

        println!(
            "Round 1: {}",
            str::from_utf8(&bytes[next(newlines)..]).unwrap()
        );

        let start = next(newlines);
        let (semicolons, newlines) = bytes[start..start + 32].find_delimiters();
        assert_eq!(semicolons, expected_mask(&bytes[start..start + 32], b';'));
        assert_eq!(newlines, expected_mask(&bytes[start..start + 32], b'\n'));

        println!("SC: {}", fmt_to_nibbles(semicolons));
        println!("NL: {}", fmt_to_nibbles(newlines));

        println!(
            "Round 2: {}",
            str::from_utf8(&bytes[start + next(newlines)..]).unwrap()
        );

        let start = start + next(newlines);
        let (semicolons, newlines) = bytes[start..start + 32].find_delimiters();
        assert_eq!(semicolons, expected_mask(&bytes[start..start + 32], b';'));
        assert_eq!(newlines, expected_mask(&bytes[start..start + 32], b'\n'));

        println!("SC: {}", fmt_to_nibbles(semicolons));
        println!("NL: {}", fmt_to_nibbles(newlines));

        println!(
            "Round 3: {}",
            str::from_utf8(&bytes[start + next(newlines)..]).unwrap()
        );

        let start = start + next(newlines);
        let end = start + 32.min(bytes.len() - start);
        let (semicolons, newlines) = bytes[start..end].find_delimiters();
        assert_eq!(semicolons, expected_mask(&bytes[start..end], b';'));
        assert_eq!(newlines, expected_mask(&bytes[start..end], b'\n'));

        println!("SC: {}", fmt_to_nibbles(semicolons));
        println!("NL: {}", fmt_to_nibbles(newlines));

        println!("Complete",);
    }
    #[test]
    fn test_no_delimiters() {
        let line = "Llanfairpwllgwyngyllgogerychwyrndrobwllllantysiliogogogoch;12.3\n";
        let bytes = line.as_bytes();

        let (semicolons, newlines) = bytes[..32].find_delimiters();
        assert_eq!(semicolons, 0);
        assert_eq!(newlines, 0);
    }

    #[test]
    fn test_one_complete_line() {
        let lines = "Melbourne;23.4\nSan Francisco;-1.2\n";
        let bytes = lines.as_bytes();

        let (semicolons, newlines) = bytes[..32].find_delimiters();
        assert_eq!(semicolons, expected_mask(&bytes[..32], b';'));
        assert_eq!(newlines, expected_mask(&bytes[..32], b'\n'));
        assert_eq!(semicolons.count_ones(), 2); // Two semicolons visible
        assert_eq!(newlines.count_ones(), 1); // Only one newline complete
    }

    #[test]
    fn test_four_complete_lines() {
        let lines = "Xi;1.2\nBo;3.4\nAb;5.6\nCd;7.8\nLeftover";
        let bytes = lines.as_bytes();

        let (semicolons, newlines) = bytes[..32].find_delimiters();
        assert_eq!(semicolons, expected_mask(&bytes[..32], b';'));
        assert_eq!(newlines, expected_mask(&bytes[..32], b'\n'));
        assert_eq!(semicolons.count_ones(), 4);
        assert_eq!(newlines.count_ones(), 4);
    }

    #[test]
    fn test_delimiter_at_position_zero() {
        let bytes = b";12.3\nAbc;4.5\nDef;6.7\nGhi;8.9\nJon;3.3\n";

        let (semicolons, _) = bytes[..32].find_delimiters();
        assert_eq!(semicolons, expected_mask(&bytes[..32], b';'));
        assert!(semicolons & 1 != 0, "Expected semicolon at position 0");
    }

    #[test]
    fn test_delimiter_at_position_31() {
        let line = "SomeStationNameHere;12.34567899\n";
        assert_eq!(line.len(), 32);
        let bytes = line.as_bytes();

        let (semicolons, newlines) = bytes[..32].find_delimiters();
        assert_eq!(semicolons, expected_mask(&bytes[..32], b';'));
        assert_eq!(newlines, expected_mask(&bytes[..32], b'\n'));
        assert!(newlines & (1 << 31) != 0, "Expected newline at position 31");
    }

    #[test]
    fn test_delimiter_at_boundaries() {
        let line = "1234567;8\n901234;6.7\nAbcdef;8.9\n";
        let bytes = line.as_bytes();

        let (semicolons, newlines) = bytes[..32].find_delimiters();
        assert_eq!(semicolons, expected_mask(&bytes[..32], b';'));
        assert_eq!(newlines, expected_mask(&bytes[..32], b'\n'));

        assert!(
            semicolons & (1 << 7) != 0,
            "Expected semicolon at position 7"
        );
        assert!(newlines & (1 << 9) != 0, "Expected newline at position 9");
    }

    #[test]
    fn test_window_smaller_than_32() {
        let line = "Tokyo;35.6\n";
        let bytes = line.as_bytes();
        assert!(bytes.len() < 32);

        let (semicolons, newlines) = bytes.find_delimiters();
        assert_eq!(semicolons, expected_mask(bytes, b';'));
        assert_eq!(newlines, expected_mask(bytes, b'\n'));
    }

    #[test]
    fn test_byte_position() {
        let cases: Vec<(Vec<u8>, Option<usize>, Option<usize>)> = vec![
            (b"Xi;3.4\n".to_vec(), Some(2), Some(6)),
            (b"Lima;5.6\n".to_vec(), Some(4), Some(8)),
            (b"Berlin;12.3\n".to_vec(), Some(6), Some(11)),
            (b"Melbourne;23.4\n".to_vec(), Some(9), Some(14)),
            (b"San Francisco;-5.2\n".to_vec(), Some(13), Some(18)),
            (b"Thiruvananthapuram;31.2\n".to_vec(), Some(18), Some(23)),
            (
                b"Some Very Long Station Name That Goes On Forever;99.9\n".to_vec(),
                Some(48),
                Some(53),
            ),
            (b"".to_vec(), None, None),
            (b"Hell\nBo\n".to_vec(), None, Some(4)),
        ];

        for (input, semicolon, newline) in cases {
            assert_eq!(input.byte_position(b';'), semicolon);
            assert_eq!(input.byte_position(b'\n'), newline);
        }
    }

    #[test]
    fn test_byte_position_realistic() {
        let lines = vec![
            "Bāgepalli;17.8",
            "San Fernando;-1.9",
            "Kika;4.3",
            "Bo;6.8",
            "Poyo;39.2",
            "Pālakodu;10.4",
            "Konibodom;40.2",
        ];
        let str = lines.join("\n");
        let bytes = str.as_bytes();

        let end = bytes.len();

        // Bāgepalli;17.8
        let start = 0;
        let newline = bytes[start..end].byte_position(b'\n').unwrap();
        assert_eq!(newline, 15);

        let line = &bytes[start..start + newline];
        assert!(!line.is_empty());
        assert_eq!(line.byte_position(b';'), Some(10));

        // San Fernando;-1.9
        let start = start + newline + 1;
        let newline = bytes[start..end].byte_position(b'\n').unwrap();
        assert_eq!(newline, 17);

        let line = &bytes[start..start + newline];
        assert!(!line.is_empty());
        assert_eq!(line.byte_position(b';'), Some(12));

        // Kika;4.3
        let start = start + newline + 1;
        let newline = bytes[start..end].byte_position(b'\n').unwrap();
        assert_eq!(newline, 8);

        let line = &bytes[start..start + newline];
        assert!(!line.is_empty());
        assert_eq!(line.byte_position(b';'), Some(4));

        // Bo;6.8
        let start = start + newline + 1;
        let newline = bytes[start..end].byte_position(b'\n').unwrap();
        assert_eq!(newline, 6);

        let line = &bytes[start..start + newline];
        assert!(!line.is_empty());
        assert_eq!(line.byte_position(b';'), Some(2));
    }
}
