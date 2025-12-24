use std::arch::x86_64::{_MM_HINT_T0, _mm_prefetch};

// (2^64) / \phi
const MAGIC_CONST: i64 = 0x9E3779B97F4A7C15u64 as i64;

#[derive(Clone, PartialEq, Eq, Debug, Default)]
#[repr(C)]
pub struct Entry {
    pub sum: i32,
    pub count: u16,
    pub min: i16,
    pub max: i16,
    pub len: u8,
    _pad: [u8; 5],
}

pub struct Table {
    hash: Vec<u64>,
    prefix: Vec<u64>,
    data: Vec<Entry>,
    names: Vec<[u8; 128]>,
    size: usize,
}

impl Table {
    pub fn new(size: usize) -> Self {
        Self {
            hash: vec![0u64; size],
            prefix: vec![0u64; size],
            data: vec![Entry::default(); size],
            names: vec![[0u8; 128]; size],
            size,
        }
    }

    pub fn hash(name: &[u8]) -> (u64, u64) {
        let len = name.len();
        let prefix = Table::prefix(name);
        let suffix_offset = len.saturating_sub(8);
        let suffix_mask = ((len > 8) as u64).wrapping_neg();
        let suffix = unsafe { (name.as_ptr().add(suffix_offset) as *const u64).read_unaligned() }
            & suffix_mask;

        if len <= 16 {
            let hash = ((prefix ^ suffix) as i64).wrapping_mul(MAGIC_CONST);
            return ((hash ^ (hash >> 35)) as u64, prefix);
        }

        let mut hash: i64 = prefix as i64;
        let mut i = 8;
        while i + 8 < len {
            hash ^= unsafe { (name.as_ptr().add(i) as *const u64).read_unaligned() as i64 };
            i += 8;
        }

        hash ^= suffix as i64;

        hash = hash.wrapping_mul(MAGIC_CONST);
        hash ^= hash >> 35;
        (hash as u64, prefix)
    }

    #[inline(always)]
    pub fn prefix(name: &[u8]) -> u64 {
        let len = name.len().min(8);
        let mask = u64::MAX >> ((8 - len) * 8);
        let bytes = unsafe { (name.as_ptr() as *const u64).read_unaligned() };
        bytes & mask
    }

    #[inline(always)]
    pub fn prefetch(&self, hash: u64) {
        let slot = hash as usize & (self.size - 1);
        unsafe {
            _mm_prefetch(self.data.as_ptr().add(slot) as *const i8, _MM_HINT_T0);
        }
    }

    #[cfg(all(target_feature = "avx512f", target_feature = "avx512bw"))]
    #[inline(always)]
    pub fn lookup(&self, hash: u64, prefix: u64) -> usize {
        let size_mask = self.size - 1;
        let slot = hash as usize & size_mask;

        unsafe {
            #[cfg(target_arch = "x86")]
            use std::arch::x86::{
                __m512i, _mm512_cmpeq_epi64_mask, _mm512_loadu_si512, _mm512_set1_epi64,
                _mm512_setzero_si512,
            };

            #[cfg(target_arch = "x86_64")]
            use std::arch::x86_64::{
                __m512i, _mm512_cmpeq_epi64_mask, _mm512_loadu_si512, _mm512_set1_epi64,
                _mm512_setzero_si512,
            };

            let h = _mm512_loadu_si512(self.hash[slot..].as_ptr() as *const __m512i);
            let target_h = _mm512_set1_epi64(hash as i64);
            let match_h = _mm512_cmpeq_epi64_mask(h, target_h);

            let p = _mm512_loadu_si512(self.prefix[slot..].as_ptr() as *const __m512i);
            let target_p = _mm512_set1_epi64(prefix as i64);
            let match_p = _mm512_cmpeq_epi64_mask(p, target_p);

            let zero = _mm512_setzero_si512();
            let empty = _mm512_cmpeq_epi64_mask(h, zero);

            let matches = empty | (match_h & match_p);

            (slot + matches.trailing_zeros() as usize) & size_mask
        }
    }

    #[cfg(not(any(target_feature = "avx512f", target_feature = "avx512bw")))]
    #[inline(always)]
    pub fn lookup(&self, hash: u64, prefix: u64) -> usize {
        let size_mask = self.size - 1;
        let slot = hash as usize & size_mask;

        let &h0 = unsafe { self.hash.get_unchecked(slot) };
        let &h1 = unsafe { self.hash.get_unchecked((slot + 1) & size_mask) };
        let &h2 = unsafe { self.hash.get_unchecked((slot + 2) & size_mask) };
        let &h3 = unsafe { self.hash.get_unchecked((slot + 3) & size_mask) };
        let &h4 = unsafe { self.hash.get_unchecked((slot + 4) & size_mask) };

        let &p0 = unsafe { self.prefix.get_unchecked(slot) };
        let &p1 = unsafe { self.prefix.get_unchecked((slot + 1) & size_mask) };
        let &p2 = unsafe { self.prefix.get_unchecked((slot + 2) & size_mask) };
        let &p3 = unsafe { self.prefix.get_unchecked((slot + 3) & size_mask) };
        let &p4 = unsafe { self.prefix.get_unchecked((slot + 4) & size_mask) };

        let m0 = ((h0 == 0) | ((h0 == hash) & (p0 == prefix))) as u32;
        let m1 = ((h1 == 0) | ((h1 == hash) & (p1 == prefix))) as u32;
        let m2 = ((h2 == 0) | ((h2 == hash) & (p2 == prefix))) as u32;
        let m3 = ((h3 == 0) | ((h3 == hash) & (p3 == prefix))) as u32;
        let m4 = ((h4 == 0) | ((h4 == hash) & (p4 == prefix))) as u32;

        let mask = m0 | (m1 << 1) | (m2 << 2) | (m3 << 3) | (m4 << 4);
        let first = mask.trailing_zeros() as usize;

        (slot + first) & size_mask
    }

    #[inline(never)]
    pub fn update(&mut self, slot: usize, hash: u64, prefix: u64, name: &[u8], temp: i16) {
        let len = name.len();
        let entry = unsafe { self.data.get_unchecked_mut(slot) };

        if entry.len != 0 {
            entry.sum += temp as i32;
            entry.count += 1;
            entry.min = entry.min.min(temp);
            entry.max = entry.max.max(temp);
            return;
        }

        entry.sum = temp as i32;
        entry.count = 1;
        entry.min = temp;
        entry.max = temp;
        entry.len = len as u8;

        self.hash[slot] = hash;
        self.prefix[slot] = prefix;
        self.names[slot][..len].copy_from_slice(name);
    }

    #[inline(never)]
    pub fn entries(&self) -> Vec<(&[u8; 128], &Entry)> {
        self.data
            .iter()
            .enumerate()
            .filter(|(_, m)| m.len != 0)
            .map(|(i, m)| (&self.names[i], m))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash() {
        let a = "Cardinal".as_bytes();
        let b = "Greater Manchester".as_bytes();
        let c = "Ur".as_bytes();

        let a_hash = Table::hash(a);
        let b_hash = Table::hash(b);
        let c_hash = Table::hash(c);

        assert_eq!(a_hash, a_hash);
        assert_eq!(b_hash, b_hash);
        assert_eq!(c_hash, c_hash);

        assert_ne!(a_hash, b_hash);
        assert_ne!(a_hash, c_hash);
        assert_ne!(b_hash, c_hash);
    }

    #[test]
    fn test_lookup() {
        let tbl = Table::new(16);

        let key1 = "Cardinal".as_bytes();
        let key2 = "Wolsey".as_bytes();

        let (hash1, prefix1) = Table::hash(key1);
        let (hash2, prefix2) = Table::hash(key2);

        assert_ne!(tbl.lookup(hash1, prefix1), tbl.lookup(hash2, prefix2));
    }

    #[test]
    fn test_lookup_and_update() {
        let mut tbl = Table::new(16);

        let key1 = "Cardinal".as_bytes();
        let key2 = "Wolsey".as_bytes();

        let (hash1, prefix1) = Table::hash(key1);
        let (hash2, prefix2) = Table::hash(key2);

        let slot1 = tbl.lookup(hash1, prefix1);
        let slot2 = tbl.lookup(hash2, prefix2);

        tbl.update(slot1, hash1, prefix1, key1, 300);
        tbl.update(slot2, hash2, prefix2, key2, 20);

        assert_eq!(tbl.data[slot1].sum, 300);
        assert_eq!(tbl.data[slot2].sum, 20);
    }
}
