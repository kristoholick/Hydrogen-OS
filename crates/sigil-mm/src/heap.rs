//! Size-class heap.
//!
//! Small requests (≤ 2 KiB) come from per-size free lists carved out of 4 KiB pages:
//! fast, no searching, little waste. Larger requests get whole pages. Because every
//! page is 4 KiB-aligned and every class size divides 4096, a block of class `c` is
//! always aligned to `c` — which is how alignment requests are honoured.

use crate::PAGE_SIZE;
use core::alloc::Layout;
use core::ptr::{NonNull, null_mut};

/// Block sizes for small allocations.
pub const CLASSES: [usize; 8] = [16, 32, 64, 128, 256, 512, 1024, 2048];

/// Where the heap gets its pages from (in the kernel: fresh frames mapped into the heap region).
///
/// # Safety
/// `alloc_pages` must return page-aligned, writable memory of `count * PAGE_SIZE` bytes that
/// nobody else uses, and `free_pages` must accept exactly what `alloc_pages` handed out.
pub unsafe trait PageSource {
    fn alloc_pages(&mut self, count: usize) -> Option<NonNull<u8>>;
    /// # Safety
    /// `ptr` and `count` must come from one earlier `alloc_pages` call and no longer be in use.
    unsafe fn free_pages(&mut self, ptr: NonNull<u8>, count: usize);
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct HeapStats {
    /// Bytes currently handed out (rounded up to block/page size).
    pub bytes_in_use: usize,
    /// Live allocations.
    pub live: usize,
    /// Pages permanently given to small-block lists.
    pub small_pages: usize,
    /// Pages currently held by large allocations.
    pub large_pages: usize,
}

struct FreeBlock {
    next: Option<NonNull<FreeBlock>>,
}

pub struct SizeClassHeap<P: PageSource> {
    source: P,
    lists: [Option<NonNull<FreeBlock>>; CLASSES.len()],
    stats: HeapStats,
}

// SAFETY: the free lists point only into memory this heap owns exclusively; moving the
// heap to another core moves that ownership with it.
unsafe impl<P: PageSource + Send> Send for SizeClassHeap<P> {}

impl<P: PageSource> SizeClassHeap<P> {
    pub const fn new(source: P) -> Self {
        Self { source, lists: [None; CLASSES.len()], stats: HeapStats { bytes_in_use: 0, live: 0, small_pages: 0, large_pages: 0 } }
    }

    pub fn stats(&self) -> HeapStats {
        self.stats
    }

    pub fn source_mut(&mut self) -> &mut P {
        &mut self.source
    }

    fn class_for(layout: Layout) -> Option<usize> {
        let need = layout.size().max(layout.align()).max(1);
        CLASSES.iter().position(|&c| c >= need)
    }

    fn large_pages(layout: Layout) -> usize {
        layout.size().max(1).div_ceil(PAGE_SIZE)
    }

    /// Returns null when out of memory or when the alignment is above one page.
    pub fn alloc(&mut self, layout: Layout) -> *mut u8 {
        match Self::class_for(layout) {
            Some(c) => self.alloc_small(c),
            None => {
                if layout.align() > PAGE_SIZE {
                    return null_mut();
                }
                let pages = Self::large_pages(layout);
                match self.source.alloc_pages(pages) {
                    Some(p) => {
                        self.stats.large_pages += pages;
                        self.stats.bytes_in_use += pages * PAGE_SIZE;
                        self.stats.live += 1;
                        p.as_ptr()
                    }
                    None => null_mut(),
                }
            }
        }
    }

    fn alloc_small(&mut self, class: usize) -> *mut u8 {
        if self.lists[class].is_none() && !self.refill(class) {
            return null_mut();
        }
        let block = self.lists[class].unwrap();
        // SAFETY: every pointer on a free list is a block inside a page we own, written by
        // `refill` or `dealloc` with a valid `FreeBlock`.
        self.lists[class] = unsafe { block.as_ref().next };
        self.stats.bytes_in_use += CLASSES[class];
        self.stats.live += 1;
        block.as_ptr() as *mut u8
    }

    fn refill(&mut self, class: usize) -> bool {
        let Some(page) = self.source.alloc_pages(1) else { return false };
        self.stats.small_pages += 1;
        let size = CLASSES[class];
        // Push blocks in reverse so allocation walks the page front to back.
        for i in (0..PAGE_SIZE / size).rev() {
            // SAFETY: `page` is a fresh, exclusively owned 4 KiB page; `i * size` stays inside
            // it and is aligned to `size` (≥ 16), enough for a `FreeBlock`.
            unsafe {
                let b = page.as_ptr().add(i * size) as *mut FreeBlock;
                b.write(FreeBlock { next: self.lists[class] });
                self.lists[class] = Some(NonNull::new_unchecked(b));
            }
        }
        true
    }

    /// # Safety
    /// `ptr` must come from `alloc` on this heap with the same `layout`, and not be used again.
    pub unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        let Some(nn) = NonNull::new(ptr) else { return };
        match Self::class_for(layout) {
            Some(c) => {
                let b = nn.as_ptr() as *mut FreeBlock;
                // SAFETY: the caller returns a block of class `c` that it no longer uses.
                unsafe { b.write(FreeBlock { next: self.lists[c] }) };
                self.lists[c] = Some(nn.cast());
                self.stats.bytes_in_use -= CLASSES[c];
            }
            None => {
                let pages = Self::large_pages(layout);
                // SAFETY: a large allocation came straight from `alloc_pages(pages)`.
                unsafe { self.source.free_pages(nn, pages) };
                self.stats.large_pages -= pages;
                self.stats.bytes_in_use -= pages * PAGE_SIZE;
            }
        }
        self.stats.live -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::{alloc, dealloc};

    /// Host page source backed by the normal allocator; counts outstanding pages.
    #[derive(Default)]
    struct HostPages {
        outstanding: usize,
    }

    unsafe impl PageSource for HostPages {
        fn alloc_pages(&mut self, count: usize) -> Option<NonNull<u8>> {
            self.outstanding += count;
            NonNull::new(unsafe { alloc(Layout::from_size_align(count * PAGE_SIZE, PAGE_SIZE).unwrap()) })
        }
        unsafe fn free_pages(&mut self, ptr: NonNull<u8>, count: usize) {
            self.outstanding -= count;
            unsafe { dealloc(ptr.as_ptr(), Layout::from_size_align(count * PAGE_SIZE, PAGE_SIZE).unwrap()) }
        }
    }

    fn lay(size: usize, align: usize) -> Layout {
        Layout::from_size_align(size, align).unwrap()
    }

    #[test]
    fn every_class_is_aligned_and_usable() {
        let mut h = SizeClassHeap::new(HostPages::default());
        for &c in &CLASSES {
            let l = lay(c, 8);
            let p = h.alloc(l);
            assert!(!p.is_null());
            assert_eq!(p as usize % c, 0, "class {c} not aligned");
            unsafe { p.write_bytes(0xAB, c) };
            unsafe { h.dealloc(p, l) };
        }
        assert_eq!(h.stats().bytes_in_use, 0);
        assert_eq!(h.stats().live, 0);
    }

    #[test]
    fn many_blocks_never_overlap() {
        let mut h = SizeClassHeap::new(HostPages::default());
        let l = lay(48, 8); // class 64
        let mut ptrs = Vec::new();
        for i in 0..2000u32 {
            let p = h.alloc(l) as *mut u32;
            assert!(!p.is_null());
            unsafe { p.write(i) };
            ptrs.push(p);
        }
        for (i, &p) in ptrs.iter().enumerate() {
            assert_eq!(unsafe { p.read() }, i as u32, "block {i} was overwritten");
        }
        for p in ptrs {
            unsafe { h.dealloc(p as *mut u8, l) };
        }
        assert_eq!(h.stats().bytes_in_use, 0);
    }

    #[test]
    fn freed_blocks_are_reused() {
        let mut h = SizeClassHeap::new(HostPages::default());
        let l = lay(100, 8);
        let a = h.alloc(l);
        unsafe { h.dealloc(a, l) };
        let b = h.alloc(l);
        assert_eq!(a, b);
        assert_eq!(h.stats().small_pages, 1);
    }

    #[test]
    fn alignment_bigger_than_size_is_honoured() {
        let mut h = SizeClassHeap::new(HostPages::default());
        let l = lay(8, 256);
        let p = h.alloc(l);
        assert_eq!(p as usize % 256, 0);
        unsafe { h.dealloc(p, l) };
    }

    #[test]
    fn large_allocations_use_and_return_whole_pages() {
        let mut h = SizeClassHeap::new(HostPages::default());
        let l = lay(3 * PAGE_SIZE + 1, 8);
        let p = h.alloc(l);
        assert!(!p.is_null());
        assert_eq!(p as usize % PAGE_SIZE, 0);
        assert_eq!(h.stats().large_pages, 4);
        assert_eq!(h.source_mut().outstanding, 4);
        unsafe { h.dealloc(p, l) };
        assert_eq!(h.stats().large_pages, 0);
        assert_eq!(h.source_mut().outstanding, 0);
    }

    #[test]
    fn huge_alignment_is_refused_not_faked() {
        let mut h = SizeClassHeap::new(HostPages::default());
        assert!(h.alloc(lay(16, 2 * PAGE_SIZE)).is_null());
    }

    struct EmptySource;
    unsafe impl PageSource for EmptySource {
        fn alloc_pages(&mut self, _: usize) -> Option<NonNull<u8>> {
            None
        }
        unsafe fn free_pages(&mut self, _: NonNull<u8>, _: usize) {}
    }

    #[test]
    fn out_of_memory_returns_null() {
        let mut h = SizeClassHeap::new(EmptySource);
        assert!(h.alloc(lay(32, 8)).is_null());
        assert!(h.alloc(lay(10 * PAGE_SIZE, 8)).is_null());
    }
}
