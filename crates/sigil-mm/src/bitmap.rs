//! Bitmap frame allocator: one bit per 4 KiB physical frame (1 = used, 0 = free).

/// Why a `free` call was rejected.
#[derive(Debug, PartialEq, Eq)]
pub enum FreeError {
    /// The frame number is past the end of physical memory.
    OutOfRange,
    /// The frame was already free — freeing it again would corrupt the books.
    DoubleFree,
}

pub struct FrameBitmap<'a> {
    words: &'a mut [u64],
    frames: usize,
    free: usize,
    hint: usize, // word index where the next search starts
}

impl<'a> FrameBitmap<'a> {
    /// Number of `u64` words needed to track `frames` frames.
    pub const fn words_needed(frames: usize) -> usize {
        frames.div_ceil(64)
    }

    /// Wraps `words` as a bitmap for `frames` frames. Every frame starts **used**;
    /// the caller then frees the ranges the firmware reports as usable RAM.
    pub fn new(words: &'a mut [u64], frames: usize) -> Self {
        assert!(words.len() * 64 >= frames, "bitmap storage too small");
        words.fill(u64::MAX);
        Self { words, frames, free: 0, hint: 0 }
    }

    pub fn total(&self) -> usize {
        self.frames
    }
    pub fn free_count(&self) -> usize {
        self.free
    }
    pub fn used_count(&self) -> usize {
        self.frames - self.free
    }

    pub fn is_used(&self, frame: usize) -> bool {
        frame >= self.frames || self.words[frame / 64] & (1 << (frame % 64)) != 0
    }

    fn set(&mut self, frame: usize) {
        self.words[frame / 64] |= 1 << (frame % 64);
    }
    fn clear(&mut self, frame: usize) {
        self.words[frame / 64] &= !(1 << (frame % 64));
    }

    /// Mark `[start, start + count)` free. Frames past the end are ignored.
    pub fn mark_free_range(&mut self, start: usize, count: usize) {
        let end = start.saturating_add(count).min(self.frames);
        for f in start..end {
            if self.is_used(f) {
                self.clear(f);
                self.free += 1;
            }
        }
        self.hint = 0;
    }

    /// Mark `[start, start + count)` used. Frames past the end are ignored.
    pub fn mark_used_range(&mut self, start: usize, count: usize) {
        let end = start.saturating_add(count).min(self.frames);
        for f in start..end {
            if !self.is_used(f) {
                self.set(f);
                self.free -= 1;
            }
        }
    }

    /// Take one free frame.
    pub fn alloc(&mut self) -> Option<usize> {
        if self.free == 0 {
            return None;
        }
        let n = self.words.len();
        for i in 0..n {
            let w = (self.hint + i) % n;
            if self.words[w] != u64::MAX {
                let frame = w * 64 + (!self.words[w]).trailing_zeros() as usize;
                if frame >= self.frames {
                    continue; // padding bits of the last word are always "used"
                }
                self.set(frame);
                self.free -= 1;
                self.hint = w;
                return Some(frame);
            }
        }
        None
    }

    /// Take `count` physically contiguous frames whose first frame is a multiple of
    /// `align` frames (needed later for DMA buffers). Linear scan; fine for rare use.
    pub fn alloc_contiguous(&mut self, count: usize, align: usize) -> Option<usize> {
        if count == 0 || count > self.free {
            return None;
        }
        let align = align.max(1);
        let mut start = 0;
        while start + count <= self.frames {
            match (start..start + count).find(|&f| self.is_used(f)) {
                Some(used) => start = (used + 1).next_multiple_of(align),
                None => {
                    self.mark_used_range(start, count);
                    return Some(start);
                }
            }
        }
        None
    }

    /// Give one frame back.
    pub fn free(&mut self, frame: usize) -> Result<(), FreeError> {
        if frame >= self.frames {
            return Err(FreeError::OutOfRange);
        }
        if !self.is_used(frame) {
            return Err(FreeError::DoubleFree);
        }
        self.clear(frame);
        self.free += 1;
        self.hint = self.hint.min(frame / 64);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn make(frames: usize) -> Vec<u64> {
        vec![0; FrameBitmap::words_needed(frames)]
    }

    #[test]
    fn starts_all_used() {
        let mut w = make(100);
        let mut b = FrameBitmap::new(&mut w, 100);
        assert_eq!(b.free_count(), 0);
        assert_eq!(b.alloc(), None);
    }

    #[test]
    fn alloc_returns_unique_frames_until_empty() {
        let mut w = make(130); // spans 3 words, last one partial
        let mut b = FrameBitmap::new(&mut w, 130);
        b.mark_free_range(0, 130);
        let mut seen = HashSet::new();
        while let Some(f) = b.alloc() {
            assert!(f < 130);
            assert!(seen.insert(f), "frame {f} handed out twice");
        }
        assert_eq!(seen.len(), 130);
        assert_eq!(b.free_count(), 0);
    }

    #[test]
    fn only_freed_ranges_are_handed_out() {
        let mut w = make(1000);
        let mut b = FrameBitmap::new(&mut w, 1000);
        b.mark_free_range(200, 50);
        b.mark_free_range(900, 500); // runs past the end: clamped
        assert_eq!(b.free_count(), 50 + 100);
        while let Some(f) = b.alloc() {
            assert!((200..250).contains(&f) || (900..1000).contains(&f));
        }
    }

    #[test]
    fn free_and_reuse() {
        let mut w = make(64);
        let mut b = FrameBitmap::new(&mut w, 64);
        b.mark_free_range(0, 64);
        let f = b.alloc().unwrap();
        assert_eq!(b.free(f), Ok(()));
        assert_eq!(b.free(f), Err(FreeError::DoubleFree));
        assert_eq!(b.free(64), Err(FreeError::OutOfRange));
        assert_eq!(b.free_count(), 64);
    }

    #[test]
    fn used_ranges_are_protected() {
        let mut w = make(256);
        let mut b = FrameBitmap::new(&mut w, 256);
        b.mark_free_range(0, 256);
        b.mark_used_range(10, 20);
        assert_eq!(b.free_count(), 236);
        while let Some(f) = b.alloc() {
            assert!(!(10..30).contains(&f));
        }
    }

    #[test]
    fn contiguous_respects_alignment_and_holes() {
        let mut w = make(512);
        let mut b = FrameBitmap::new(&mut w, 512);
        b.mark_free_range(0, 512);
        b.mark_used_range(5, 1); // hole
        let s = b.alloc_contiguous(16, 16).unwrap();
        assert_eq!(s % 16, 0);
        assert!(s >= 16, "must skip the hole at frame 5");
        for f in s..s + 16 {
            assert!(b.is_used(f));
        }
        assert_eq!(b.alloc_contiguous(10_000, 1), None);
    }
}
