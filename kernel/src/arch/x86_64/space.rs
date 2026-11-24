//! Address spaces of user processes: one set of page tables each.
//!
//! The lower half (user) is private to the process. The upper half (kernel) is shared: all 256
//! upper top-level entries are copied from the kernel's table, which `vmm::init` fills once and
//! never changes again, so kernel mappings made later show up in every process by themselves.
//!
//! The kernel never dereferences user addresses. It walks the page tables in software and
//! reaches the frames through the direct map, so SMAP can stay on all the time and a bad user
//! pointer is an ordinary error, not a kernel page fault.

use crate::mm::{pmm, vmm};
use hydrogen_mm::PAGE_SIZE;
use x86_64::PhysAddr;
use x86_64::registers::control::{Cr3, Cr3Flags};
use x86_64::structures::paging::{PageTable, PageTableFlags as F, PhysFrame};

/// The first 64 KiB stay unmapped so null pointers (and small offsets from them) always fault.
pub const USER_START: u64 = 0x1_0000;
/// The last page below the canonical hole is never mapped: code there could run off the end
/// into a non-canonical address, which the CPU reports in awkward places (`iretq`/`sysret`).
pub const USER_END: u64 = 0x0000_7fff_ffff_f000;

const PAGE: u64 = PAGE_SIZE as u64;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MapError {
    /// Address not page-aligned or outside the user half.
    BadAddress,
    AlreadyMapped,
    /// A page must never be writable and executable at the same time.
    WriteAndExec,
    OutOfMemory,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Access {
    pub write: bool,
    pub exec: bool,
}

pub struct AddressSpace {
    pml4: u64,
}

/// Index into the page table of `level` (4 = top ... 1 = last).
fn index(virt: u64, level: u32) -> usize {
    ((virt >> (12 + 9 * (level - 1))) & 0x1ff) as usize
}

fn table(phys: u64) -> *mut PageTable {
    (vmm::hhdm() + phys) as *mut PageTable
}

fn zeroed_frame() -> Option<u64> {
    let f = pmm::alloc_frame()?.as_u64();
    // SAFETY: the frame was just allocated, is exclusively ours, and is reachable (and
    // page-aligned) through the direct map.
    unsafe { core::ptr::write_bytes((vmm::hhdm() + f) as *mut u8, 0, PAGE_SIZE) };
    Some(f)
}

impl AddressSpace {
    /// An empty user half plus the shared kernel half. `None` when out of memory.
    pub fn new() -> Option<Self> {
        let pml4 = zeroed_frame()?;
        // SAFETY: both tables are valid, page-aligned and reachable through the direct map; the
        // new one is exclusively ours and the kernel's upper half never changes after boot.
        unsafe {
            let (new, kernel) = (&mut *table(pml4), &*table(vmm::kernel_pml4()));
            for i in 256..512 {
                assert!(!kernel[i].is_unused(), "kernel mappings made later would be missing here");
                new[i] = kernel[i].clone();
            }
        }
        Some(Self { pml4 })
    }

    /// Map a fresh zeroed page at `virt`.
    pub fn map(&mut self, virt: u64, access: Access) -> Result<(), MapError> {
        if virt % PAGE != 0 || !(USER_START..USER_END).contains(&virt) {
            return Err(MapError::BadAddress);
        }
        if access.write && access.exec {
            return Err(MapError::WriteAndExec);
        }
        let mut phys = self.pml4;
        for level in [4, 3, 2] {
            // SAFETY: `phys` is a page table of this address space; `&mut self` makes the
            // access exclusive.
            let entry = unsafe { &mut (&mut *table(phys))[index(virt, level)] };
            if entry.is_unused() {
                let f = zeroed_frame().ok_or(MapError::OutOfMemory)?;
                entry.set_addr(PhysAddr::new(f), F::PRESENT | F::WRITABLE | F::USER_ACCESSIBLE);
            }
            phys = entry.addr().as_u64();
        }
        // SAFETY: as above; `phys` is now the last-level table.
        let entry = unsafe { &mut (&mut *table(phys))[index(virt, 1)] };
        if !entry.is_unused() {
            return Err(MapError::AlreadyMapped);
        }
        let mut flags = F::PRESENT | F::USER_ACCESSIBLE;
        flags.set(F::WRITABLE, access.write);
        flags.set(F::NO_EXECUTE, !access.exec);
        let f = zeroed_frame().ok_or(MapError::OutOfMemory)?;
        entry.set_addr(PhysAddr::new(f), flags);
        // No TLB flush: the page was not present before, and the CPU does not cache that.
        Ok(())
    }

    /// Fill already mapped pages, whatever their user permissions (used by the loader).
    pub fn fill(&mut self, virt: u64, bytes: &[u8]) -> Result<(), MapError> {
        for_each_piece(self.pml4, virt, bytes.len(), false, |dst, at, n| {
            // SAFETY: `dst` is the direct-map address of `n` bytes in a frame owned by this
            // address space; `at..at + n` is within `bytes`; kernel and user memory do not
            // overlap.
            unsafe { core::ptr::copy_nonoverlapping(bytes[at..].as_ptr(), dst, n) }
        })
        .ok_or(MapError::BadAddress)
    }

    pub fn activate(&self) {
        switch_to(self.pml4);
    }
}

impl Drop for AddressSpace {
    fn drop(&mut self) {
        assert_ne!(active(), self.pml4, "freeing the active page tables");
        // SAFETY: the table is ours, not active, and nothing else refers to its lower half.
        let top = unsafe { &*table(self.pml4) };
        for e in top.iter().take(256).filter(|e| !e.is_unused()) {
            free_tree(e.addr().as_u64(), 3);
        }
        pmm::free_frame(PhysAddr::new(self.pml4));
    }
}

/// Free the table at `phys` with everything below it (level 0 = the data frame itself).
fn free_tree(phys: u64, level: u32) {
    if level > 0 {
        // SAFETY: called by `drop` for tables of an inactive address space that is going away.
        for e in unsafe { &*table(phys) }.iter().filter(|e| !e.is_unused()) {
            free_tree(e.addr().as_u64(), level - 1);
        }
    }
    pmm::free_frame(PhysAddr::new(phys));
}

fn active() -> u64 {
    Cr3::read().0.start_address().as_u64()
}

fn switch_to(pml4: u64) {
    if active() != pml4 {
        // SAFETY: every top-level table maps the whole kernel half identically, so the code,
        // stack and data in use stay mapped. Writing CR3 also drops stale user translations.
        unsafe { Cr3::write(PhysFrame::containing_address(PhysAddr::new(pml4)), Cr3Flags::empty()) };
    }
}

/// Back to the kernel's own tables (no user half at all).
pub fn activate_kernel() {
    switch_to(vmm::kernel_pml4());
}

/// Physical address behind `virt` if a user program could access it (and write, if asked).
fn user_phys(pml4: u64, virt: u64, write: bool) -> Option<u64> {
    let mut need = F::PRESENT | F::USER_ACCESSIBLE;
    need.set(F::WRITABLE, write);
    let mut phys = pml4;
    for level in [4, 3, 2, 1] {
        // SAFETY: `phys` is a live page table reached from `pml4`; read only.
        let e = unsafe { &(&*table(phys))[index(virt, level)] };
        if !e.flags().contains(need) || (level > 1 && e.flags().contains(F::HUGE_PAGE)) {
            return None;
        }
        phys = e.addr().as_u64();
    }
    Some(phys + virt % PAGE)
}

/// Check that all of `virt..virt + len` is user memory, then call `f(direct-map address,
/// offset, length)` for each piece within one page. `None`, with nothing called, if any byte
/// is not accessible.
fn for_each_piece(pml4: u64, virt: u64, len: usize, write: bool, mut f: impl FnMut(*mut u8, usize, usize)) -> Option<()> {
    let end = virt.checked_add(len as u64)?;
    if virt < USER_START || end > USER_END {
        return None;
    }
    let pieces = || {
        let mut v = virt;
        core::iter::from_fn(move || {
            (v < end).then(|| {
                let (start, n) = (v, (PAGE - v % PAGE).min(end - v));
                v += n;
                (start, n as usize)
            })
        })
    };
    for (v, _) in pieces() {
        user_phys(pml4, v, write)?;
    }
    for (v, n) in pieces() {
        let p = user_phys(pml4, v, write)?;
        f((vmm::hhdm() + p) as *mut u8, (v - virt) as usize, n);
    }
    Some(())
}

/// Copy from the active process's memory. `None` if any byte of it is not readable user memory.
pub fn copy_from_user(virt: u64, out: &mut [u8]) -> Option<()> {
    for_each_piece(active(), virt, out.len(), false, |src, at, n| {
        // SAFETY: `src` is the direct-map address of `n` readable bytes of a user frame that
        // stays mapped (a process has one thread, and it is here); `out[at..at + n]` is ours.
        unsafe { core::ptr::copy_nonoverlapping(src, out[at..].as_mut_ptr(), n) }
    })
}
