//! Kernel thread stacks: 16 KiB each, with an unmapped guard page directly below, so an
//! overflow page-faults instead of silently overwriting another thread's memory.
//!
//! Slot `i` occupies `[THREAD_STACKS + i*SLOT, THREAD_STACKS + (i+1)*SLOT)`:
//!   +0 .. +4 KiB        guard page (never mapped)
//!   +4 KiB .. +20 KiB   stack (RW, no-exec)
//!   rest of the slot    unmapped spacing
//!
//! Slots of finished threads are reused, so the page tables behind them stay bounded too.

use crate::mm::{pmm, vmm};
use crate::sync::IrqMutex;
use alloc::vec::Vec;
use hydrogen_mm::PAGE_SIZE;
use x86_64::PhysAddr;
use x86_64::structures::paging::PageTableFlags as F;

const THREAD_STACKS: u64 = 0xffff_fd00_0000_0000;
const STACK_SIZE: u64 = 16 * 1024;
const PAGE: u64 = PAGE_SIZE as u64;
const SLOT: u64 = 64 * 1024;
const MAX_SLOTS: u64 = 1 << 20; // 64 GiB of address space; far more than ever needed

struct Slots {
    next: u64,
    free: Vec<u64>,
}

static SLOTS: IrqMutex<Slots> = IrqMutex::new(Slots { next: 0, free: Vec::new() });

pub struct ThreadStack {
    slot: u64,
}

impl ThreadStack {
    pub fn new() -> Self {
        let slot = {
            let mut s = SLOTS.lock();
            match s.free.pop() {
                Some(slot) => slot,
                None => {
                    assert!(s.next < MAX_SLOTS, "out of thread stack slots");
                    s.next += 1;
                    s.next - 1
                }
            }
        };
        let stack = Self { slot };
        for v in stack.pages() {
            let f = pmm::alloc_frame().expect("out of memory for thread stack");
            vmm::map_page(v, f.as_u64(), F::PRESENT | F::WRITABLE | F::NO_EXECUTE);
        }
        stack
    }

    fn pages(&self) -> impl Iterator<Item = u64> + use<> {
        let first = THREAD_STACKS + self.slot * SLOT + PAGE;
        (first..first + STACK_SIZE).step_by(PAGE_SIZE)
    }

    /// 16-byte-aligned address just past the highest stack byte.
    pub fn top(&self) -> u64 {
        THREAD_STACKS + self.slot * SLOT + PAGE + STACK_SIZE
    }
}

impl Drop for ThreadStack {
    fn drop(&mut self) {
        for v in self.pages() {
            let f = vmm::unmap_page(v).expect("thread stack page was not mapped");
            pmm::free_frame(PhysAddr::new(f));
        }
        SLOTS.lock().free.push(self.slot); // only reusable once fully unmapped
    }
}

/// Is `addr` inside the guard page of some thread stack slot?
pub fn is_guard(addr: u64) -> bool {
    (THREAD_STACKS..THREAD_STACKS + MAX_SLOTS * SLOT).contains(&addr) && (addr - THREAD_STACKS) % SLOT < PAGE
}
