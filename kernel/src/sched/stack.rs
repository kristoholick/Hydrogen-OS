//! Kernel thread stacks: 16 KiB each, with an unmapped guard page directly below, so an
//! overflow page-faults instead of silently overwriting another thread's memory.
//!
//! Slot `i` occupies `[THREAD_STACKS + i*SLOT, THREAD_STACKS + (i+1)*SLOT)`:
//!   +0 .. +4 KiB        guard page (never mapped)
//!   +4 KiB .. +20 KiB   stack (RW, no-exec)
//!   rest of the slot    unmapped spacing

use crate::mm::{pmm, vmm};
use core::sync::atomic::{AtomicU64, Ordering};
use hydrogen_mm::PAGE_SIZE;
use x86_64::structures::paging::PageTableFlags as F;

pub const THREAD_STACKS: u64 = 0xffff_fd00_0000_0000;
pub const STACK_SIZE: u64 = 16 * 1024;
const SLOT: u64 = 64 * 1024;
const MAX_SLOTS: u64 = 1 << 20; // 64 GiB of address space; far more than ever needed

static NEXT_SLOT: AtomicU64 = AtomicU64::new(0);

pub struct ThreadStack {
    base: u64, // start of the slot = the guard page
}

impl ThreadStack {
    pub fn new() -> Self {
        let slot = NEXT_SLOT.fetch_add(1, Ordering::Relaxed);
        assert!(slot < MAX_SLOTS, "out of thread stack slots");
        let base = THREAD_STACKS + slot * SLOT;
        let mut v = base + PAGE_SIZE as u64;
        while v < base + PAGE_SIZE as u64 + STACK_SIZE {
            let f = pmm::alloc_frame().expect("out of memory for thread stack");
            vmm::map_page(v, f.as_u64(), F::PRESENT | F::WRITABLE | F::NO_EXECUTE);
            v += PAGE_SIZE as u64;
        }
        Self { base }
    }

    /// 16-byte-aligned address just past the highest stack byte.
    pub fn top(&self) -> u64 {
        self.base + PAGE_SIZE as u64 + STACK_SIZE
    }
}

/// Is `addr` inside the guard page of some thread stack slot?
pub fn is_guard(addr: u64) -> bool {
    addr >= THREAD_STACKS
        && addr < THREAD_STACKS + MAX_SLOTS * SLOT
        && (addr - THREAD_STACKS) % SLOT < PAGE_SIZE as u64
}
