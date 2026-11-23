//! Boot-time self-checks. Each one either logs "ok" or panics with what went wrong,
//! which the headless tests turn into a pass/fail.

use crate::arch::x86_64::rtc;
use crate::mm::{heap, pmm, vmm};
use crate::{kprintln, time};
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;
use x86_64::PhysAddr;
use x86_64::structures::paging::PageTableFlags as F;

pub fn physical_memory() {
    // Reserve the list's own heap memory first: growing the heap takes frames from the PMM
    // too, and those must not be mistaken for a leak.
    let mut frames: Vec<u64> = Vec::with_capacity(1000);
    let before = pmm::stats().free_frames;
    frames.extend((0..1000).map(|_| pmm::alloc_frame().expect("out of frames").as_u64()));
    frames.sort_unstable();
    let n = frames.len();
    frames.dedup();
    assert_eq!(frames.len(), n, "PMM handed out the same frame twice");
    assert!(frames.iter().all(|f| f % 4096 == 0 && *f != 0), "bad frame address");
    for f in &frames {
        pmm::free_frame(PhysAddr::new(*f));
    }
    assert_eq!(pmm::stats().free_frames, before, "frames leaked");
    kprintln!("[sigil] selftest pmm: ok (1000 frames allocated, unique, all returned)");
}

pub fn paging() {
    let frame = pmm::alloc_frame().expect("out of frames").as_u64();
    vmm::map_page(vmm::SCRATCH_VIRT, frame, F::PRESENT | F::WRITABLE | F::NO_EXECUTE);
    assert_eq!(vmm::translate(vmm::SCRATCH_VIRT), Some(frame));
    let via_page = vmm::SCRATCH_VIRT as *mut u64;
    let via_hhdm = (vmm::hhdm() + frame) as *const u64;
    // SAFETY: SCRATCH_VIRT was just mapped to `frame`, which is also visible through the
    // direct map; both pointers are aligned and nothing else uses this frame.
    let seen = unsafe {
        via_page.write_volatile(0x5161_4C5F_4B45_4550);
        via_hhdm.read_volatile()
    };
    assert_eq!(seen, 0x5161_4C5F_4B45_4550, "two mappings of one frame disagree");
    assert_eq!(vmm::unmap_page(vmm::SCRATCH_VIRT), Some(frame));
    assert_eq!(vmm::translate(vmm::SCRATCH_VIRT), None, "page still mapped after unmap");
    pmm::free_frame(PhysAddr::new(frame));
    kprintln!("[sigil] selftest paging: ok (map, alias check, unmap)");
}

pub fn heap() {
    let base = heap::stats();
    {
        let mut v: Vec<u64> = Vec::new();
        for i in 0..10_000u64 {
            v.push(i);
        }
        assert_eq!(v.iter().sum::<u64>(), 10_000 * 9_999 / 2);

        let b = Box::new([7u8; 2048]);
        assert!(b.iter().all(|&x| x == 7));

        let s: String = format!("{} kernel, heap at {:#x}", "SIGIL", vmm::HEAP_START);
        assert!(s.starts_with("SIGIL kernel"));

        let mut map = BTreeMap::new();
        for i in 0..1000u32 {
            map.insert(i, i * i);
        }
        assert_eq!(map.get(&999), Some(&998_001));

        let big = alloc::vec![0xA5u8; 1 << 20]; // 1 MiB: takes the whole-page path
        assert_eq!(big[(1 << 20) - 1], 0xA5);
    }
    let after = heap::stats();
    assert_eq!(after.live, base.live, "heap allocations leaked");
    assert_eq!(after.bytes_in_use, base.bytes_in_use, "heap bytes leaked");
    kprintln!("[sigil] selftest heap: ok (Vec 10k, Box, String, BTreeMap 1k, 1 MiB buffer; no leaks)");
}

/// Count timer ticks across one second of the CMOS real-time clock — an independent clock,
/// so this checks the APIC calibration rather than trusting it. Returns ticks counted.
pub fn timer_vs_rtc() -> Option<u64> {
    let wait_edge = |limit: u64| -> Option<u64> {
        let start = time::ticks();
        let s = rtc::seconds();
        while rtc::seconds() == s {
            if time::ticks() - start > limit {
                return None;
            }
            core::hint::spin_loop();
        }
        Some(time::ticks())
    };
    wait_edge(300)?; // line up with the start of a fresh RTC second
    let t0 = time::ticks();
    let t1 = wait_edge(300)?;
    Some(t1 - t0)
}

/// Threads take turns: A and B each log their letter and yield five times, which must give
/// exactly "ABABABABAB". Then 64 short threads run to check stacks are freed and reused.
pub fn threads() {
    use crate::sync::IrqMutex;
    use crate::task;
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicU64, Ordering};

    assert!(task::stack_layout_ok(), "thread stack layout wrong (guard page mapped, or stack not mapped/unmapped)");
    let base_threads = task::thread_count();
    let base_stacks = task::live_stacks();

    // 1) strict alternation between two threads
    let trace = Arc::new(IrqMutex::new(String::new()));
    let mut ids = Vec::new();
    for (name, letter) in [("selftest-a", 'A'), ("selftest-b", 'B')] {
        let t = trace.clone();
        ids.push(task::spawn(name, move || {
            for _ in 0..5 {
                t.lock().push(letter);
                task::yield_now();
            }
        }).expect("spawn failed"));
    }
    for id in ids {
        task::join(id);
    }
    let seen = trace.lock().clone();
    assert_eq!(seen, "ABABABABAB", "threads did not take turns");

    // 2) many threads, each with its own stack; slots must be freed and reused
    let sum = Arc::new(AtomicU64::new(0));
    let mut ids = Vec::new();
    for i in 1..=64u64 {
        let s = sum.clone();
        ids.push(task::spawn("selftest-many", move || {
            let local = [i; 64]; // touch this thread's own stack
            task::yield_now();
            s.fetch_add(core::hint::black_box(local)[63], Ordering::Relaxed);
        }).expect("spawn failed"));
    }
    for id in ids {
        task::join(id);
    }
    assert_eq!(sum.load(Ordering::Relaxed), 64 * 65 / 2, "a thread's work went missing");

    task::yield_now(); // the scheduler frees finished threads on its next run
    assert_eq!(task::thread_count(), base_threads, "finished threads were not freed");
    assert_eq!(task::live_stacks(), base_stacks, "thread stacks leaked");
    kprintln!("[sigil] selftest threads: ok ({} interleaved; 64 more threads ran; stacks freed; guard pages in place)", seen);
}
