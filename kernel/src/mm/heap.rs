//! Kernel heap: `Box`, `Vec`, `String`, `BTreeMap`… backed by `sigil_mm::heap`.
//! Pages come from the PMM and are mapped on demand into the heap region.

use super::{pmm, vmm};
use crate::sync::IrqMutex;
use core::alloc::{GlobalAlloc, Layout};
use core::ptr::NonNull;
use sigil_mm::PAGE_SIZE;
use sigil_mm::heap::{HeapStats, PageSource, SizeClassHeap};
use x86_64::PhysAddr;
use x86_64::structures::paging::PageTableFlags as F;

pub struct KernelPages {
    next: u64, // next unused virtual address in the heap region (simple bump; 1 GiB of room)
}

// SAFETY: every page handed out is a fresh PMM frame mapped RW at a never-used heap address,
// so it is page-aligned, writable and exclusively owned; `free_pages` unmaps and frees
// exactly those pages.
unsafe impl PageSource for KernelPages {
    fn alloc_pages(&mut self, count: usize) -> Option<NonNull<u8>> {
        let size = (count * PAGE_SIZE) as u64;
        if self.next + size > vmm::HEAP_START + vmm::HEAP_MAX {
            return None;
        }
        let start = self.next;
        for i in 0..count as u64 {
            let Some(frame) = pmm::alloc_frame() else {
                // undo the partial mapping, then report out-of-memory
                for j in 0..i {
                    if let Some(p) = vmm::unmap_page(start + j * PAGE_SIZE as u64) {
                        pmm::free_frame(PhysAddr::new(p));
                    }
                }
                return None;
            };
            vmm::map_page(start + i * PAGE_SIZE as u64, frame.as_u64(), F::PRESENT | F::WRITABLE | F::NO_EXECUTE);
        }
        self.next += size;
        NonNull::new(start as *mut u8)
    }

    unsafe fn free_pages(&mut self, ptr: NonNull<u8>, count: usize) {
        for i in 0..count as u64 {
            if let Some(p) = vmm::unmap_page(ptr.as_ptr() as u64 + i * PAGE_SIZE as u64) {
                pmm::free_frame(PhysAddr::new(p));
            }
        }
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
    KernelAllocator(IrqMutex::new(SizeClassHeap::new(KernelPages { next: vmm::HEAP_START })));

pub fn stats() -> HeapStats {
    ALLOCATOR.0.lock().stats()
}
