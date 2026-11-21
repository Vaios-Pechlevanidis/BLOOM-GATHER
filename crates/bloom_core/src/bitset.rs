//! A fixed-length bitset — the harvested state of a region's flowers.
//!
//! The only way to set a bit is [`BitSet::test_and_set`], which reports
//! whether the bit changed. There is deliberately no way to clear a bit:
//! harvested flowers never come back.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BitSet {
    len: u32,
    words: Vec<u64>,
}

impl BitSet {
    pub fn new(len: u32) -> Self {
        Self {
            len,
            words: vec![0; (len as usize).div_ceil(64)],
        }
    }

    pub fn len(&self) -> u32 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Out-of-range indices read as unset.
    pub fn get(&self, i: u32) -> bool {
        i < self.len && self.words[(i / 64) as usize] & (1 << (i % 64)) != 0
    }

    /// Set bit `i`. Returns `true` if it was previously unset (i.e. this call
    /// changed it) and `false` if it was already set or out of range.
    pub fn test_and_set(&mut self, i: u32) -> bool {
        if i >= self.len {
            return false;
        }
        let word = &mut self.words[(i / 64) as usize];
        let mask = 1 << (i % 64);
        let was_set = *word & mask != 0;
        *word |= mask;
        !was_set
    }

    pub fn count_ones(&self) -> u32 {
        self.words.iter().map(|w| w.count_ones()).sum()
    }

    pub fn iter_ones(&self) -> impl Iterator<Item = u32> + '_ {
        (0..self.len).filter(|&i| self.get(i))
    }

    /// Structural integrity after deserialising untrusted data: the word
    /// count matches the length and no bits are set beyond it.
    pub fn is_well_formed(&self) -> bool {
        if self.words.len() != (self.len as usize).div_ceil(64) {
            return false;
        }
        let tail = self.len % 64;
        match self.words.last() {
            Some(&last) if tail != 0 => last >> tail == 0,
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_once_only() {
        let mut b = BitSet::new(130);
        assert!(!b.get(129));
        assert!(b.test_and_set(129));
        assert!(!b.test_and_set(129), "second set must report no change");
        assert!(b.get(129));
        assert!(!b.test_and_set(130), "out of range");
        assert_eq!(b.count_ones(), 1);
    }

    #[test]
    fn counts_and_iterates() {
        let mut b = BitSet::new(200);
        for i in [0, 63, 64, 127, 199] {
            b.test_and_set(i);
        }
        assert_eq!(b.count_ones(), 5);
        assert_eq!(b.iter_ones().collect::<Vec<_>>(), vec![0, 63, 64, 127, 199]);
        assert!(b.is_well_formed());
    }

    #[test]
    fn detects_malformed_data() {
        let bad_len = BitSet {
            len: 100,
            words: vec![0],
        };
        assert!(!bad_len.is_well_formed());
        let stray_bit = BitSet {
            len: 100,
            words: vec![0, 1 << 40],
        };
        assert!(!stray_bit.is_well_formed());
        assert!(BitSet {
            len: 128,
            words: vec![u64::MAX, u64::MAX]
        }
        .is_well_formed());
    }
}
