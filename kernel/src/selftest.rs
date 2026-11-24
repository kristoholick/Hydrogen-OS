//! Boot-time self-checks. Each one either logs "ok" or panics with what went wrong,
//! which the headless tests turn into a pass/fail.

mod user;

use crate::arch::x86_64::rtc;
use crate::mm::{heap, pmm, vmm};
use crate::{kprintln, sched, time};
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
    kprintln!("[hydrogen] selftest pmm: ok (1000 frames allocated, unique, all returned)");
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
    kprintln!("[hydrogen] selftest paging: ok (map, alias check, unmap)");
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

        let s: String = format!("{} kernel, heap at {:#x}", "Hydrogen", vmm::HEAP_START);
        assert!(s.starts_with("Hydrogen kernel"));

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
    kprintln!("[hydrogen] selftest heap: ok (Vec 10k, Box, String, BTreeMap 1k, 1 MiB buffer; no leaks)");
}

/// Two kernel threads take turns (cooperative yield). Each appends "A1", "B1", ... to a shared
/// trace; the boot thread waits until both have exited and checks the exact order.
pub fn threads() {
    use crate::sync::IrqMutex;

    static TRACE: IrqMutex<String> = IrqMutex::new(String::new());
    const STEPS: u32 = 3;

    fn worker(tag: char) {
        for i in 1..=STEPS {
            let id = sched::current_id();
            kprintln!("[hydrogen] thread {} (id {}): step {}", tag, id, i);
            let mut t = TRACE.lock();
            if !t.is_empty() {
                t.push(' ');
            }
            t.push(tag);
            t.push_str(&format!("{}", i));
            drop(t);
            sched::yield_now();
        }
    }

    let a = sched::spawn("thread-a", || worker('A'));
    let b = sched::spawn("thread-b", || worker('B'));
    let mut rounds = 0u32;
    while sched::is_alive(a) || sched::is_alive(b) {
        sched::yield_now();
        rounds += 1;
        assert!(rounds < 1000, "threads never finished");
    }
    let trace = TRACE.lock().clone();
    assert_eq!(trace, "A1 B1 A2 B2 A3 B3", "threads did not alternate");
    kprintln!("[hydrogen] selftest threads: ok ({}; 2 threads on 16 KiB guarded stacks, both exited)", trace);
}

/// The timer takes the CPU away from threads that never give it up (Phase 3.2).
pub fn preemption() {
    locks_mask_interrupts();
    clock_while_printing();
    preempt_endless_loops();
    preempt_state_intact();
    preempt_no_leaks();
    preempt_idle();
}

/// First programs outside the kernel (Phase 3.3).
pub fn user() {
    user::run();
}

/// Wait (asleep) until the threads are gone.
fn join(ids: &[sched::ThreadId]) {
    let start = time::ticks();
    while ids.iter().any(|&id| sched::is_alive(id)) {
        assert!(time::ticks() - start < 500, "threads did not finish within 5 s");
        sched::sleep_ms(10);
    }
}

/// A thread must never be preempted while holding a lock, and must get its interrupts back.
fn locks_mask_interrupts() {
    use crate::sync::IrqMutex;
    use x86_64::instructions::interrupts::are_enabled;
    static OUTER: IrqMutex<()> = IrqMutex::new(());
    static INNER: IrqMutex<()> = IrqMutex::new(());
    assert!(are_enabled());
    let outer = OUTER.lock();
    assert!(!are_enabled(), "a held lock left interrupts on");
    drop(INNER.lock());
    assert!(!are_enabled(), "releasing an inner lock turned interrupts back on");
    drop(outer);
    assert!(are_enabled(), "releasing the last lock left interrupts off");
    kprintln!("[hydrogen] selftest preempt: ok (locks mask interrupts, also when nested)");
}

/// Printing must not hold interrupts off long enough to lose timer ticks. Compares the tick
/// counter with the CPU's own cycle counter, first while quiet, then while printing.
fn clock_while_printing() {
    let now = || {
        // SAFETY: rdtsc exists on every x86-64 CPU and has no side effects.
        (time::ticks(), unsafe { core::arch::x86_64::_rdtsc() })
    };
    sched::sleep_ms(10); // line up with a tick
    let (t0, c0) = now();
    sched::sleep_ms(200);
    let (t1, c1) = now();
    let cycles_per_tick = (c1 - c0) / (t1 - t0);
    for i in 1..=6 {
        kprintln!("[hydrogen] clock check, line {} of 6: printing must not stop the clock", i);
    }
    let (t2, c2) = now();
    let (counted, real) = (t2 - t1, (c2 - c1) / cycles_per_tick);
    assert!(counted.abs_diff(real) <= 2, "clock lost time while printing: counted {counted} ticks, really {real}");
    kprintln!("[hydrogen] selftest preempt: ok (clock kept time while printing: {} ticks counted, {} by cycle counter)", counted, real);
}

/// Three threads spin without ever yielding. The boot thread must still get to run, must wake
/// from sleep on time (with three rivals, queueing behind them would be 2+ ticks late), and the
/// three must share the CPU fairly.
fn preempt_endless_loops() {
    static STOP: AtomicBool = AtomicBool::new(false);
    fn spin() {
        while !STOP.load(Ordering::Relaxed) {
            core::hint::spin_loop();
        }
    }
    let a = sched::spawn("spin-a", spin);
    let b = sched::spawn("spin-b", spin);
    let c = sched::spawn("spin-c", spin);
    let began = time::ticks();
    for ms in [10u64, 50, 100, 250] {
        let t0 = time::ticks();
        let late = sched::sleep_ms(ms);
        let took = time::ticks() - t0;
        assert!(took >= ms / 10, "sleep({ms}) woke early: {took} ticks");
        assert!(late <= 1, "sleep({ms}) woke {late} ticks late");
        kprintln!("[hydrogen] sleep({} ms) took {} ticks of 10 ms, {} late", ms, took, late);
    }
    let total = time::ticks() - began;
    let got = [a, b, c].map(|id| sched::cpu_ticks(id).unwrap());
    assert!(got.iter().all(|&t| t >= total / 5), "unfair: {got:?} of {total} ticks");
    STOP.store(true, Ordering::Relaxed);
    join(&[a, b, c]);
    kprintln!("[hydrogen] selftest preempt: ok (3 threads in endless loops got {:?} of {} ticks; sleeper still on time)", got, total);
}

/// Four threads compute and allocate without yielding, each comparing every result with the
/// answer computed undisturbed. A register, stack or heap block mixed up by a forced switch
/// would change an answer.
fn preempt_state_intact() {
    static EXPECTED: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];
    static ROUNDS: AtomicU64 = AtomicU64::new(0);
    static WRONG: AtomicU64 = AtomicU64::new(0);
    static SHARED_CPU: AtomicU64 = AtomicU64::new(0);

    #[inline(never)]
    fn work(seed: u64) -> u64 {
        let mut r = [seed; 12];
        let mut list: Vec<u64> = Vec::new();
        for i in 0..200u64 {
            for k in 0..12 {
                let mixed = r[(k + 5) % 12] ^ i;
                r[k] = r[k].wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(mixed).rotate_left(k as u32 + 1);
            }
            list.push(r[(i % 12) as usize]);
        }
        let text = format!("{:x}", r[3]);
        list.iter().chain(&r).fold(text.len() as u64, |a, &x| a.rotate_left(7) ^ x)
    }
    fn worker(n: usize) {
        let end = time::ticks() + 40;
        while time::ticks() < end {
            if work(n as u64 + 1) != EXPECTED[n].load(Ordering::Relaxed) {
                WRONG.fetch_add(1, Ordering::Relaxed);
            }
            ROUNDS.fetch_add(1, Ordering::Relaxed);
        }
        // ran in at least 3 separate slices, so it was interrupted and resumed
        let slices = sched::cpu_ticks(sched::current_id()).unwrap();
        SHARED_CPU.fetch_add((slices >= 3) as u64, Ordering::Relaxed);
    }
    for (n, e) in EXPECTED.iter().enumerate() {
        e.store(x86_64::instructions::interrupts::without_interrupts(|| work(n as u64 + 1)), Ordering::Relaxed);
    }
    join(&[
        sched::spawn("work-0", || worker(0)),
        sched::spawn("work-1", || worker(1)),
        sched::spawn("work-2", || worker(2)),
        sched::spawn("work-3", || worker(3)),
    ]);
    let (rounds, wrong) = (ROUNDS.load(Ordering::Relaxed), WRONG.load(Ordering::Relaxed));
    assert_eq!(wrong, 0, "{wrong} of {rounds} results corrupted by thread switches");
    assert_eq!(SHARED_CPU.load(Ordering::Relaxed), 4, "some worker was hardly preempted: test proved nothing");
    kprintln!("[hydrogen] selftest preempt: ok (4 threads, {} results under forced switches, 0 wrong)", rounds);
}

/// Memory of finished threads comes back: 50 threads leave no trace.
fn preempt_no_leaks() {
    fn short_life() {
        let s = format!("thread {}", sched::current_id());
        sched::sleep_ms(10); // be switched away and back while owning heap data
        assert!(s.starts_with("thread"));
    }
    fn fifty() {
        for _ in 0..5 {
            let ids: [_; 10] = core::array::from_fn(|_| sched::spawn("short", short_life));
            join(&ids);
        }
    }
    let snapshot = || {
        let h = heap::stats();
        (pmm::stats().free_frames, h.live, h.bytes_in_use, sched::thread_count())
    };
    fifty(); // warm-up: lists and page tables that are kept for reuse reach their full size
    let before = snapshot();
    fifty();
    assert_eq!(snapshot(), before, "leak: (free frames, heap blocks, heap bytes, threads) changed");
    kprintln!("[hydrogen] selftest preempt: ok (50 threads started and finished, no memory leaked)");
}

/// With every thread asleep, the idle thread (which halts the CPU) gets the time.
fn preempt_idle() {
    let before = sched::cpu_ticks(sched::IDLE).unwrap();
    sched::sleep_ms(200);
    let idle = sched::cpu_ticks(sched::IDLE).unwrap() - before;
    assert!((19..=22).contains(&idle), "idle thread got {idle} ticks of about 20");
    kprintln!("[hydrogen] selftest preempt: ok (idle thread took {} ticks while all others slept)", idle);
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
