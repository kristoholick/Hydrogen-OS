//! Guarded kernel stacks for threads.
//!
//! Each stack lives in its own 128 KiB slot of the thread-stack region (see `mm::vmm`):
//! one unmapped guard page, then 64 KiB of mapped stack. Running off the bottom hits the
//! guard page, which the exception handlers report as "thread stack overflow".

use crate::mm::{pmm, vmm};
use crate::sync::IrqMutex;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};
use sigil_mm::PAGE_SIZE;
use x86_64::PhysAddr;
use x86_64::structures::paging::PageTableFlags as F;

/// Slots handed back by dropped stacks, reused before fresh ones.
static FREE_SLOTS: IrqMutex<Vec<usize>> = IrqMutex::new(Vec::new());
/// Next never-used slot.
static NEXT_SLOT: AtomicUsize = AtomicUsize::new(0);
/// Stacks currently alive (for leak checks).
static LIVE: AtomicUsize = AtomicUsize::new(0);

pub struct KernelStack {
    slot: usize,
}

fn stack_bottom(slot: usize) -> u64 {
    vmm::THREAD_STACKS + slot as u64 * vmm::THREAD_SLOT + PAGE_SIZE as u64
}

fn unmap_pages(bottom: u64, pages: u64) {
    for i in 0..pages {
        if let Some(p) = vmm::unmap_page(bottom + i * PAGE_SIZE as u64) {
            pmm::free_frame(PhysAddr::new(p));
        }
    }
}

impl KernelStack {
    /// Map a fresh 64 KiB stack. `None` if out of memory or out of slots.
    pub fn new() -> Option<Self> {
        let slot = match FREE_SLOTS.lock().pop() {
            Some(s) => s,
            None => {
                let s = NEXT_SLOT.fetch_add(1, Ordering::Relaxed);
                if s >= vmm::THREAD_SLOTS {
                    NEXT_SLOT.fetch_sub(1, Ordering::Relaxed);
                    return None;
                }
                s
            }
        };
        let bottom = stack_bottom(slot);
        let pages = vmm::THREAD_STACK_SIZE / PAGE_SIZE as u64;
        for i in 0..pages {
            let Some(frame) = pmm::alloc_frame() else {
                unmap_pages(bottom, i);
                FREE_SLOTS.lock().push(slot);
                return None;
            };
            vmm::map_page(bottom + i * PAGE_SIZE as u64, frame.as_u64(), F::PRESENT | F::WRITABLE | F::NO_EXECUTE);
        }
        LIVE.fetch_add(1, Ordering::Relaxed);
        Some(Self { slot })
    }

    /// One past the highest byte of the stack (16-byte aligned).
    pub fn top(&self) -> u64 {
        stack_bottom(self.slot) + vmm::THREAD_STACK_SIZE
    }

    /// Address of this stack's guard page (never mapped).
    pub fn guard(&self) -> u64 {
        stack_bottom(self.slot) - PAGE_SIZE as u64
    }
}

/// Self-check: a fresh stack has its whole body mapped, its guard page unmapped and
/// recognised as a thread guard; once dropped, nothing of it stays mapped.
pub fn layout_ok() -> bool {
    let Some(s) = KernelStack::new() else { return false };
    let (guard, bottom, top) = (s.guard(), stack_bottom(s.slot), s.top());
    let alive_ok = vmm::translate(guard).is_none()
        && vmm::stack_guard_kind(guard) == Some("thread")
        && vmm::translate(bottom).is_some()
        && vmm::translate(top - 8).is_some();
    drop(s);
    alive_ok && vmm::translate(bottom).is_none() && vmm::translate(top - 8).is_none()
}

impl Drop for KernelStack {
    fn drop(&mut self) {
        unmap_pages(stack_bottom(self.slot), vmm::THREAD_STACK_SIZE / PAGE_SIZE as u64);
        FREE_SLOTS.lock().push(self.slot);
        LIVE.fetch_sub(1, Ordering::Relaxed);
    }
}

pub fn live() -> usize {
    LIVE.load(Ordering::Relaxed)
}
