//! Virtual memory: the kernel's own page tables.
//!
//! Layout of the kernel's (upper-half) address space:
//!
//! | Range                         | What                                   | Permissions      |
//! |-------------------------------|----------------------------------------|------------------|
//! | `HHDM + phys`                 | direct map of RAM, ACPI, framebuffer   | RW, no-exec      |
//! | `0xffff_d000_0000_0000`       | scratch page for self-tests            | RW, no-exec      |
//! | `0xffff_e000_0000_0000` +1 GiB| kernel heap (grows on demand)          | RW, no-exec      |
//! | `0xffff_fd00_0000_0000`       | thread stacks: 64 KiB slots, each a guard page + 16 KiB stack (`sched::stack`) | RW, no-exec |
//! | `0xffff_fe00_0000_0000`       | guard page (never mapped) + 64 KiB stack | RW, no-exec    |
//! | `0xffff_ffff_8000_0000`       | kernel image: text / rodata / data     | RX / R / RW      |

use super::pmm::{self, PmmFrameAllocator};
use crate::sync::IrqMutex;
use core::sync::atomic::{AtomicU64, Ordering};
use limine::memmap::*;
use hydrogen_mm::PAGE_SIZE;
use x86_64::registers::control::{Cr3, Cr3Flags};
use x86_64::structures::paging::mapper::{MapToError, Translate};
use x86_64::structures::paging::{
    Mapper, OffsetPageTable, Page, PageTable, PageTableFlags as F, PhysFrame, Size2MiB, Size4KiB,
};
use x86_64::{PhysAddr, VirtAddr};

pub const SCRATCH_VIRT: u64 = 0xffff_d000_0000_0000;
pub const HEAP_START: u64 = 0xffff_e000_0000_0000;
pub const HEAP_MAX: u64 = 1 << 30;
pub const STACK_GUARD: u64 = 0xffff_fe00_0000_0000;
pub const STACK_SIZE: u64 = 64 * 1024;
pub const STACK_TOP: u64 = STACK_GUARD + PAGE_SIZE as u64 + STACK_SIZE;

static MAPPER: IrqMutex<Option<OffsetPageTable<'static>>> = IrqMutex::new(None);
static HHDM: AtomicU64 = AtomicU64::new(0);
static PML4: AtomicU64 = AtomicU64::new(0);

const RW_NX: F = F::PRESENT.union(F::WRITABLE).union(F::NO_EXECUTE);

unsafe extern "C" {
    static __kernel_start: u8;
    static __text_start: u8;
    static __text_end: u8;
    static __rodata_start: u8;
    static __rodata_end: u8;
    static __data_start: u8;
    static __kernel_end: u8;
}

fn sym(s: &u8) -> u64 {
    s as *const u8 as u64
}

pub fn hhdm() -> u64 {
    HHDM.load(Ordering::Relaxed)
}

/// Physical address of the kernel's top-level page table.
pub fn kernel_pml4() -> u64 {
    PML4.load(Ordering::Relaxed)
}

#[derive(Default, Debug)]
pub struct VmmInit {
    pub pages_4k: usize,
    pub pages_2m: usize,
    pub kernel_pages: usize,
}

fn map_one<S>(m: &mut OffsetPageTable<'static>, virt: u64, phys: u64, flags: F) -> bool
where
    S: x86_64::structures::paging::PageSize + core::fmt::Debug,
    OffsetPageTable<'static>: Mapper<S>,
    MapToError<S>: core::fmt::Debug,
{
    let page = Page::<S>::containing_address(VirtAddr::new(virt));
    let frame = PhysFrame::<S>::containing_address(PhysAddr::new(phys));
    // SAFETY: callers only map physical memory the kernel owns or that the firmware reported
    // (RAM, ACPI, framebuffer, MMIO), at fresh virtual addresses in the layout above, so no
    // existing Rust reference is invalidated.
    match unsafe { m.map_to(page, frame, flags, &mut PmmFrameAllocator) } {
        Ok(flush) => {
            flush.ignore(); // building inactive tables, or caller flushes
            true
        }
        Err(MapToError::PageAlreadyMapped(_)) => false, // shared edge page between entries
        Err(e) => panic!("map {:#x} -> {:#x} failed: {:?}", virt, phys, e),
    }
}

/// Build the kernel's page tables, switch to them, and remember the mapper.
pub fn init(entries: &[&Entry], hhdm: u64, kernel_phys: u64, kernel_virt: u64) -> VmmInit {
    HHDM.store(hhdm, Ordering::Relaxed);
    let mut info = VmmInit::default();

    let pml4_phys = pmm::alloc_frame().expect("out of memory for PML4");
    let pml4_ptr = (hhdm + pml4_phys.as_u64()) as *mut PageTable;
    // SAFETY: the frame was just allocated (exclusively ours) and is reachable via the HHDM;
    // zeroing it makes an empty top-level table.
    let pml4: &'static mut PageTable = unsafe {
        pml4_ptr.write(PageTable::new());
        &mut *pml4_ptr
    };
    // SAFETY: all physical memory used for page tables is reachable at `hhdm + phys`.
    let mut m = unsafe { OffsetPageTable::new(pml4, VirtAddr::new(hhdm)) };

    // 1) direct map of everything the kernel may touch, using 2 MiB pages where possible
    for e in entries {
        let wanted = matches!(e.type_,
            MEMMAP_USABLE | MEMMAP_BOOTLOADER_RECLAIMABLE | MEMMAP_EXECUTABLE_AND_MODULES
            | MEMMAP_FRAMEBUFFER | MEMMAP_ACPI_RECLAIMABLE | MEMMAP_ACPI_NVS);
        if !wanted {
            continue;
        }
        let mut p = e.base & !(PAGE_SIZE as u64 - 1);
        let end = (e.base + e.length).next_multiple_of(PAGE_SIZE as u64);
        while p < end {
            const TWO_MIB: u64 = 2 * 1024 * 1024;
            if p % TWO_MIB == 0 && end - p >= TWO_MIB {
                if map_one::<Size2MiB>(&mut m, hhdm + p, p, RW_NX) { info.pages_2m += 1; }
                p += TWO_MIB;
            } else {
                if map_one::<Size4KiB>(&mut m, hhdm + p, p, RW_NX) { info.pages_4k += 1; }
                p += PAGE_SIZE as u64;
            }
        }
    }

    // 2) the kernel image, section by section, with the tightest permissions that work
    // SAFETY: these are linker-script symbols; only their addresses are used.
    let (ks, ts, te, rs, re, ds, ke) = unsafe {
        (sym(&__kernel_start), sym(&__text_start), sym(&__text_end), sym(&__rodata_start),
         sym(&__rodata_end), sym(&__data_start), sym(&__kernel_end))
    };
    let to_phys = |v: u64| v - kernel_virt + kernel_phys;
    let mut map_range = |m: &mut OffsetPageTable<'static>, start: u64, end: u64, flags: F| {
        let mut v = start & !(PAGE_SIZE as u64 - 1);
        while v < end {
            map_one::<Size4KiB>(m, v, to_phys(v), flags);
            info.kernel_pages += 1;
            v += PAGE_SIZE as u64;
        }
    };
    map_range(&mut m, ks, ts, F::PRESENT | F::NO_EXECUTE); //  boot requests (+GOT): read only
    map_range(&mut m, ts, te, F::PRESENT); //                  code: read + execute
    map_range(&mut m, rs, re, F::PRESENT | F::NO_EXECUTE); //  constants: read only
    map_range(&mut m, ds, ke, RW_NX); //                       data + bss: read/write

    // 3) the new kernel stack; the page at STACK_GUARD stays unmapped on purpose
    let mut v = STACK_GUARD + PAGE_SIZE as u64;
    while v < STACK_TOP {
        let f = pmm::alloc_frame().expect("out of memory for kernel stack");
        map_one::<Size4KiB>(&mut m, v, f.as_u64(), RW_NX);
        v += PAGE_SIZE as u64;
    }

    // 4) give every upper-half top-level entry a (possibly empty) table now. Process page tables
    //    copy these 256 entries, so kernel mappings made later appear in all of them (1 MiB).
    for e in m.level_4_table_mut().iter_mut().skip(256).filter(|e| e.is_unused()) {
        let f = pmm::alloc_frame().expect("out of memory for kernel page tables");
        // SAFETY: the frame is freshly allocated and reachable through the direct map.
        unsafe { ((hhdm + f.as_u64()) as *mut PageTable).write(PageTable::new()) };
        e.set_addr(f, F::PRESENT | F::WRITABLE);
    }

    // 5) switch. Everything the CPU touches next (this code, the current Limine stack,
    //    statics, framebuffer, Limine responses) is mapped above.
    // SAFETY: the new PML4 maps the kernel image, the direct map (which contains the current
    // stack — checked by the caller) and all memory in use, with identical virtual addresses.
    unsafe { Cr3::write(PhysFrame::containing_address(pml4_phys), Cr3Flags::empty()) };
    PML4.store(pml4_phys.as_u64(), Ordering::Relaxed);
    *MAPPER.lock() = Some(m);
    info
}

/// Map one 4 KiB page in the active tables.
pub fn map_page(virt: u64, phys: u64, flags: F) {
    let mut g = MAPPER.lock();
    let m = g.as_mut().expect("VMM not initialised");
    let page = Page::<Size4KiB>::containing_address(VirtAddr::new(virt));
    let frame = PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(phys));
    // SAFETY: see `map_one`; callers pass fresh virtual addresses from their own regions.
    match unsafe { m.map_to(page, frame, flags, &mut PmmFrameAllocator) } {
        Ok(flush) => flush.flush(),
        Err(e) => panic!("map_page {:#x} failed: {:?}", virt, e),
    }
}

/// Unmap one 4 KiB page in the active tables, returning the physical frame it pointed to.
pub fn unmap_page(virt: u64) -> Option<u64> {
    let mut g = MAPPER.lock();
    let m = g.as_mut()?;
    let page = Page::<Size4KiB>::containing_address(VirtAddr::new(virt));
    match m.unmap(page) {
        Ok((frame, flush)) => {
            flush.flush();
            Some(frame.start_address().as_u64())
        }
        Err(_) => None,
    }
}

pub fn translate(virt: u64) -> Option<u64> {
    MAPPER.lock().as_ref()?.translate_addr(VirtAddr::new(virt)).map(|p| p.as_u64())
}

/// Map a device's registers (uncached) into the direct-map area; returns the virtual address.
pub fn map_mmio(phys: u64, len: u64) -> u64 {
    let flags = RW_NX | F::NO_CACHE | F::WRITE_THROUGH;
    let mut p = phys & !(PAGE_SIZE as u64 - 1);
    while p < phys + len {
        let v = hhdm() + p;
        if translate(v).is_none() {
            map_page(v, p, flags);
        }
        p += PAGE_SIZE as u64;
    }
    hhdm() + phys
}

pub fn is_stack_guard(addr: u64) -> bool {
    (STACK_GUARD..STACK_GUARD + PAGE_SIZE as u64).contains(&addr)
}

/// Move onto the new kernel stack and continue in `next`. The old (bootloader) stack is
/// abandoned; nothing on it is used again.
pub fn switch_stack(next: extern "C" fn() -> !) -> ! {
    // SAFETY: STACK_TOP is the 16-byte-aligned top of a mapped 64 KiB stack; `call` pushes
    // the return address so `next` starts with the alignment the ABI expects. `next`
    // never returns, so nothing depends on the old stack afterwards.
    unsafe {
        core::arch::asm!(
            "mov rsp, {top}",
            "xor ebp, ebp",
            "call {next}",
            "ud2",
            top = in(reg) STACK_TOP,
            next = in(reg) next,
            options(noreturn)
        )
    }
}
