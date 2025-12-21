// (2^64) / \phi
const MAGIC_CONST: i64 = 0x9E3779B97F4A7C15u64 as i64;

#[derive(Clone, PartialEq, Eq, Debug)]
#[repr(C)]
pub struct Entry {
    hash: u64,
    prefix: u64,

    pub sum: i32,
    pub count: u16,
    pub min: i16,
    pub max: i16,
    pub len: u8,
    _pad: [u8; 5],
}

impl Entry {
    pub fn default() -> Self {
        Self {
            hash: 0,
            prefix: 0,
            sum: 0,
            count: 0,
            min: 0,
            max: 0,
            len: 0,
            _pad: [0u8; 5],
        }
    }
}

pub struct Table {
    data: Vec<Entry>,
    names: Vec<[u8; 100]>,
    size: usize,
}

impl Table {
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![Entry::default(); size],
            names: vec![[0u8; 100]; size],
            size,
        }
    }

    #[inline(always)]
    pub fn hash(name: &[u8]) -> (u64, u64) {
        let len = name.len();
        let prefix = Table::prefix(name);

        if len <= 8 {
            let hash = (prefix as i64).wrapping_mul(MAGIC_CONST);
            return ((hash ^ (hash >> 35)) as u64, prefix);
        }

        let suffix = unsafe { (name.as_ptr().add(len - 8) as *const u64).read_unaligned() };

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
        let len = name.len();

        if len >= 8 {
            return unsafe { (name.as_ptr() as *const u64).read_unaligned() };
        }

        let bytes: [u8; 8] = match len {
            1 => [name[0], 0, 0, 0, 0, 0, 0, 0],
            2 => [name[0], name[1], 0, 0, 0, 0, 0, 0],
            3 => [name[0], name[1], name[2], 0, 0, 0, 0, 0],
            4 => [name[0], name[1], name[2], name[3], 0, 0, 0, 0],
            5 => [name[0], name[1], name[2], name[3], name[4], 0, 0, 0],
            6 => [name[0], name[1], name[2], name[3], name[4], name[5], 0, 0],
            7 => [
                name[0], name[1], name[2], name[3], name[4], name[5], name[6], 0,
            ],
            _ => unreachable!(),
        };

        u64::from_ne_bytes(bytes)
    }

    #[inline(always)]
    pub fn lookup(&self, hash: u64, prefix: u64) -> usize {
        let size_mask = self.size - 1;
        let slot = hash as usize & size_mask;

        for i in 0..5 {
            let slot = (slot + i) & size_mask;
            let data = unsafe { self.data.get_unchecked(slot) };

            if data.hash == 0 || data.prefix == prefix && data.hash == hash {
                return slot;
            }
        }

        slot + 4
    }

    #[inline(always)]
    pub fn update(&mut self, slot: usize, hash: u64, prefix: u64, name: &[u8], temp: i16) {
        let len = name.len();
        let entry = unsafe { self.data.get_unchecked_mut(slot) };

        if entry.hash != 0 {
            entry.sum += temp as i32;
            entry.count += 1;
            entry.min = entry.min.min(temp);
            entry.max = entry.max.max(temp);
            return;
        }

        entry.hash = hash;
        entry.prefix = prefix;
        entry.sum = temp as i32;
        entry.count = 1;
        entry.min = temp;
        entry.max = temp;
        entry.len = len as u8;

        self.names[slot][..len].copy_from_slice(name);
    }

    #[inline(always)]
    pub fn entries(&self) -> Vec<(&[u8; 100], &Entry)> {
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
