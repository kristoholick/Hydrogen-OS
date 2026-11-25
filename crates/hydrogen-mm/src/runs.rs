//! Hands out runs of consecutive numbers (pages of an address range) and takes them back.
//! One bit per page, no allocation of its own: usable underneath a heap.

/// Tracks `WORDS * 64` pages. First fit, so the range stays compact.
pub struct Runs<const WORDS: usize> {
    used: [u64; WORDS],
    first_free: usize, // every page before this one is used
}

impl<const WORDS: usize> Runs<WORDS> {
    pub const PAGES: usize = WORDS * 64;

    pub const fn new() -> Self {
        Self { used: [0; WORDS], first_free: 0 }
    }

    fn is_used(&self, page: usize) -> bool {
        self.used[page / 64] & (1 << (page % 64)) != 0
    }

    fn flip(&mut self, start: usize, count: usize) {
        for p in start..start + count {
            self.used[p / 64] ^= 1 << (p % 64);
        }
    }

    /// Take `count` consecutive free pages; returns the first.
    pub fn take(&mut self, count: usize) -> Option<usize> {
        if count == 0 || count > Self::PAGES {
            return None;
        }
        let mut start = self.first_free;
        let mut page = start;
        while page - start < count {
            if page >= Self::PAGES {
                return None;
            }
            if page % 64 == 0 && self.used[page / 64] == u64::MAX {
                page += 64; // skip a full word at once
                start = page;
            } else if self.is_used(page) {
                page += 1;
                start = page;
            } else {
                page += 1;
            }
        }
        self.flip(start, count);
        if start == self.first_free {
            self.first_free = start + count;
        }
        Some(start)
    }

    /// Give back a run from `take`. `false`, with nothing changed, if any of it is not in use.
    #[must_use]
    pub fn give_back(&mut self, start: usize, count: usize) -> bool {
        let Some(end) = start.checked_add(count).filter(|&e| e <= Self::PAGES) else { return false };
        if count == 0 || !(start..end).all(|p| self.is_used(p)) {
            return false;
        }
        self.flip(start, count);
        self.first_free = self.first_free.min(start);
        true
    }
}

impl<const WORDS: usize> Default for Runs<WORDS> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_lowest_and_reuses() {
        let mut r = Runs::<2>::new();
        assert_eq!(r.take(10), Some(0));
        assert_eq!(r.take(5), Some(10));
        assert_eq!(r.take(1), Some(15));
        assert!(r.give_back(0, 10));
        assert_eq!(r.take(11), Some(16)); // does not fit into the hole of 10
        assert_eq!(r.take(4), Some(0));
        assert_eq!(r.take(6), Some(4));
        assert_eq!(r.take(1), Some(27));
    }

    #[test]
    fn fills_up_completely_and_empties_again() {
        let mut r = Runs::<3>::new();
        assert_eq!(r.take(193), None);
        assert_eq!(r.take(192), Some(0));
        assert_eq!(r.take(1), None);
        assert!(r.give_back(191, 1));
        assert_eq!(r.take(2), None);
        assert_eq!(r.take(1), Some(191));
        assert!(r.give_back(0, 192));
        assert_eq!(r.take(192), Some(0));
    }

    #[test]
    fn full_words_are_skipped_correctly() {
        let mut r = Runs::<4>::new();
        assert_eq!(r.take(1), Some(0));
        assert_eq!(r.take(191), Some(1));
        assert!(r.give_back(0, 1));
        assert_eq!(r.take(64), Some(192)); // across one free page, then three full words
        assert_eq!(r.take(1), Some(0));
        assert_eq!(r.take(1), None);
    }

    #[test]
    fn wrong_give_backs_are_refused() {
        let mut r = Runs::<1>::new();
        assert_eq!(r.take(8), Some(0));
        assert!(!r.give_back(0, 0));
        assert!(!r.give_back(4, 5)); // partly free
        assert!(!r.give_back(63, 2)); // past the end
        assert!(!r.give_back(usize::MAX, 2)); // wraps around
        assert!(r.give_back(0, 8));
        assert!(!r.give_back(0, 8)); // twice
        assert_eq!(r.take(0), None);
        assert_eq!(r.take(64), Some(0)); // nothing was changed by the refused calls
        assert!(!r.give_back(63, 2)); // past the end, the rest in use
    }

    /// Random use, compared with a simple model.
    #[test]
    fn matches_a_simple_model() {
        const N: usize = 5 * 64;
        let mut r = Runs::<5>::new();
        let mut model = [false; N];
        let mut held: Vec<(usize, usize)> = Vec::new();
        let mut x = 0x2545_f491_4f6c_dd1du64;
        let mut rnd = |n: usize| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x % n as u64) as usize
        };
        let mut refused = 0;
        for _ in 0..200_000 {
            if rnd(5) < 3 {
                let count = 1 + rnd(40);
                let expected = (0..=N - count).find(|&s| model[s..s + count].iter().all(|u| !u));
                assert_eq!(r.take(count), expected);
                match expected {
                    Some(s) => {
                        model[s..s + count].fill(true);
                        held.push((s, count));
                    }
                    None => refused += 1,
                }
            } else if !held.is_empty() {
                let (s, count) = held.swap_remove(rnd(held.len()));
                assert!(r.give_back(s, count));
                model[s..s + count].fill(false);
            }
        }
        assert!(refused > 1000, "never full: test proves little");
    }
}
