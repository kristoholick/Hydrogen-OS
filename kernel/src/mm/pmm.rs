//! Physical memory manager: which 4 KiB frames of RAM are free.
//! The bookkeeping itself lives in `hydrogen_mm::bitmap` (unit-tested on the host).

use crate::sync::IrqMutex;
use limine::memmap::{Entry, MEMMAP_USABLE};
use hydrogen_mm::PAGE_SIZE;
use hydrogen_mm::bitmap::FrameBitmap;
use x86_64::PhysAddr;
use x86_64::structures::paging::{FrameAllocator, PhysFrame, Size4KiB};

static PMM: IrqMutex<Option<FrameBitmap<'static>>> = IrqMutex::new(None);

#[derive(Clone, Copy, Debug)]
pub struct PmmStats {
    pub total_frames: usize,
    pub free_frames: usize,
}

pub struct PmmInit {
    pub stats: PmmStats,
    pub bitmap_phys: u64,
    pub bitmap_bytes: usize,
}

/// Build the frame bitmap from the bootloader's memory map. Only `USABLE` RAM is ever handed
/// out; bootloader-reclaimable memory (Limine's page tables, our boot stack) is left alone.
pub fn init(entries: &[&Entry], hhdm: u64) -> PmmInit {
    let max_end = entries
        .iter()
        .filter(|e| e.type_ == MEMMAP_USABLE)
        .map(|e| e.base + e.length)
        .max()
        .expect("no usable memory in memory map");
    let frames = (max_end as usize) / PAGE_SIZE;
    let words = FrameBitmap::words_needed(frames);
    let bytes = words * 8;
    let pages = bytes.div_ceil(PAGE_SIZE);

    // Put the bitmap itself in the first usable region big enough to hold it.
    let home = entries
        .iter()
        .find(|e| e.type_ == MEMMAP_USABLE && e.base >= PAGE_SIZE as u64 && e.length as usize >= pages * PAGE_SIZE)
        .expect("no usable region large enough for the frame bitmap");

    // SAFETY: `home` is USABLE RAM that nothing else owns yet, reachable through Limine's
    // higher-half direct map, page-aligned (Limine guarantees usable entries are), and large
    // enough for `words` u64s. From here on the bitmap marks those frames as used.
    let storage: &'static mut [u64] =
        unsafe { core::slice::from_raw_parts_mut((hhdm + home.base) as *mut u64, words) };

    let mut bm = FrameBitmap::new(storage, frames);
    for e in entries.iter().filter(|e| e.type_ == MEMMAP_USABLE) {
        bm.mark_free_range(e.base as usize / PAGE_SIZE, e.length as usize / PAGE_SIZE);
    }
    bm.mark_used_range(home.base as usize / PAGE_SIZE, pages); // the bitmap's own frames
    bm.mark_used_range(0, 1); // never hand out physical address 0

    let stats = PmmStats { total_frames: bm.total(), free_frames: bm.free_count() };
    *PMM.lock() = Some(bm);
    PmmInit { stats, bitmap_phys: home.base, bitmap_bytes: bytes }
}

pub fn alloc_frame() -> Option<PhysAddr> {
    PMM.lock().as_mut()?.alloc().map(|f| PhysAddr::new((f * PAGE_SIZE) as u64))
}

pub fn free_frame(addr: PhysAddr) {
    let mut g = PMM.lock();
    let bm = g.as_mut().expect("PMM not initialised");
    if let Err(e) = bm.free(addr.as_u64() as usize / PAGE_SIZE) {
        drop(g);
        panic!("free_frame({:#x}) rejected: {:?}", addr.as_u64(), e);
    }
}

pub fn stats() -> PmmStats {
    let g = PMM.lock();
    let bm = g.as_ref().expect("PMM not initialised");
    PmmStats { total_frames: bm.total(), free_frames: bm.free_count() }
}

/// Adapter so the `x86_64` crate's page-table code can take frames from our PMM.
pub struct PmmFrameAllocator;

// SAFETY: frames come from the bitmap, which never hands the same frame out twice while it
// is in use, and only ever hands out USABLE RAM.
unsafe impl FrameAllocator<Size4KiB> for PmmFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        alloc_frame().map(PhysFrame::containing_address)
    }
}
