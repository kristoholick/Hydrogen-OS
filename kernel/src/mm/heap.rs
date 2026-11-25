//! Kernel heap: `Box`, `Vec`, `String`, `BTreeMap`… backed by `hydrogen_mm::heap`.
//! Pages come from the PMM and are mapped on demand into the heap region.

use super::{pmm, vmm};
use crate::sync::IrqMutex;
use core::alloc::{GlobalAlloc, Layout};
use core::ptr::NonNull;
use hydrogen_mm::PAGE_SIZE;
use hydrogen_mm::heap::{HeapStats, PageSource, SizeClassHeap};
use hydrogen_mm::runs::Runs;
use x86_64::PhysAddr;
use x86_64::structures::paging::PageTableFlags as F;

const WORDS: usize = (vmm::HEAP_MAX / PAGE_SIZE as u64 / 64) as usize;

/// Pages of the heap's address region (1 GiB, 32 KiB of bookkeeping).
pub struct KernelPages {
    addresses: Runs<WORDS>,
}

fn unmap(start: u64, pages: usize) {
    for i in 0..pages as u64 {
        let p = vmm::unmap_page(start + i * PAGE_SIZE as u64).expect("heap page was not mapped");
        pmm::free_frame(PhysAddr::new(p));
    }
}

// SAFETY: every page handed out is a fresh PMM frame mapped RW at a heap address that `addresses`
// says nobody else uses, so it is page-aligned, writable and exclusively owned; `free_pages`
// unmaps and frees exactly those pages before the addresses can be handed out again.
unsafe impl PageSource for KernelPages {
    fn alloc_pages(&mut self, count: usize) -> Option<NonNull<u8>> {
        let first = self.addresses.take(count)?;
        let start = vmm::HEAP_START + (first * PAGE_SIZE) as u64;
        for i in 0..count {
            let Some(frame) = pmm::alloc_frame() else {
                unmap(start, i); // undo the partial mapping, then report out-of-memory
                assert!(self.addresses.give_back(first, count));
                return None;
            };
            vmm::map_page(start + (i * PAGE_SIZE) as u64, frame.as_u64(), F::PRESENT | F::WRITABLE | F::NO_EXECUTE);
        }
        NonNull::new(start as *mut u8)
    }

    unsafe fn free_pages(&mut self, ptr: NonNull<u8>, count: usize) {
        let start = ptr.as_ptr() as u64;
        unmap(start, count);
        assert!(self.addresses.give_back(((start - vmm::HEAP_START) / PAGE_SIZE as u64) as usize, count), "heap pages freed twice");
    }
}

pub struct KernelAllocator(IrqMutex<SizeClassHeap<KernelPages>>);

// SAFETY: all state sits behind an interrupt-safe lock; `SizeClassHeap` upholds the
// GlobalAlloc contract (unique, correctly aligned blocks; null on failure).
unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.0.lock().alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: forwarded GlobalAlloc contract — `ptr` came from `alloc` with `layout`.
        unsafe { self.0.lock().dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: KernelAllocator =
    KernelAllocator(IrqMutex::new(SizeClassHeap::new(KernelPages { addresses: Runs::new() })));

pub fn stats() -> HeapStats {
    ALLOCATOR.0.lock().stats()
}
