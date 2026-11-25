//! Hydrogen kernel — entry point.
//!
//! Boot path: UEFI firmware → Limine → `kmain` (higher half, long mode, Limine's page tables).
//!   stage 1 (`kmain`, on Limine's stack): serial, CPU tables, framebuffer, physical memory,
//!            our own page tables, then hop onto the kernel's own guarded stack.
//!   stage 2 (`kmain_stage2`): heap, interrupt controller + timer, self-checks, boot screen.

#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

extern crate alloc;

mod arch;
mod boot;
mod drivers;
mod faulttest;
mod fmtbuf;
mod mm;
mod panic;
mod proc;
mod sched;
mod selftest;
mod splash;
mod sync;
mod syscall;
mod time;

use arch::x86_64::{apic, cpu, gdt, idt, pic};
use drivers::framebuffer::{FB, Framebuffer};
use limine::memmap::MEMMAP_USABLE;

const MIB: u64 = 1024 * 1024;

/// Off only in the builds that crash on purpose, to save time: those have nothing to do with it.
const SELF_TESTS: bool = !cfg!(any(
    feature = "panic-test",
    feature = "fault-pagefault",
    feature = "fault-stack",
    feature = "fault-int3",
    feature = "fault-thread-stack",
    feature = "fault-smap",
    feature = "fault-smep"
));

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    drivers::serial::init();
    kprintln!();
    kprintln!("[hydrogen] Hydrogen kernel online");
    if cfg!(feature = "slow-serial") {
        kprintln!("[hydrogen] serial: deliberately slow (test build)");
    }

    if boot::BASE_REVISION.is_supported() {
        kprintln!("[hydrogen] limine base revision {} accepted", limine::BaseRevision::MAX_SUPPORTED);
    } else {
        kprintln!("[hydrogen] WARNING: base revision {} not supported (bootloader offers {:?})",
            limine::BaseRevision::MAX_SUPPORTED, boot::BASE_REVISION.actual_revision());
    }

    // --- CPU tables first, so every later mistake is reported instead of resetting the PC ---
    gdt::init();
    idt::init();
    let (ist_lo, ist_hi) = gdt::ist_range();
    kprintln!("[hydrogen] cpu tables: GDT+TSS loaded (emergency stacks {:#x}..{:#x}), IDT loaded (all 32 exceptions)", ist_lo, ist_hi);

    // --- framebuffer (so the crash screen works from here on) ---
    init_framebuffer();

    // --- memory protection bits ---
    assert!(cpu::has_nx(), "CPU lacks the no-execute (NX) feature");
    cpu::enable_memory_protection();
    kprintln!("[hydrogen] cpu: NX + write-protect enabled");

    // --- physical memory ---
    let memmap = boot::MEMMAP.response().expect("bootloader gave no memory map");
    let hhdm = boot::HHDM.response().expect("bootloader gave no direct-map offset").offset;
    let kaddr = boot::EXECUTABLE_ADDRESS.response().expect("bootloader gave no kernel address");
    let entries = memmap.entries();
    let usable: u64 = entries.iter().filter(|e| e.type_ == MEMMAP_USABLE).map(|e| e.length).sum();
    kprintln!("[hydrogen] memory map: {} entries, {} MiB usable, direct map at {:#x}", entries.len(), usable / MIB, hhdm);

    let p = mm::pmm::init(entries, hhdm);
    kprintln!("[hydrogen] pmm: {} frames tracked, {} free ({} MiB), bitmap {} KiB at phys {:#x}",
        p.stats.total_frames, p.stats.free_frames, p.stats.free_frames as u64 * 4096 / MIB,
        p.bitmap_bytes / 1024, p.bitmap_phys);

    // --- our own page tables ---
    let rsp: u64;
    // SAFETY: reads the stack pointer register; no side effects.
    unsafe { core::arch::asm!("mov {}, rsp", out(reg) rsp, options(nomem, nostack)) };
    assert!(rsp >= hhdm, "boot stack {rsp:#x} is outside the direct map; cannot switch page tables safely");
    let v = mm::vmm::init(entries, hhdm, kaddr.physical_base, kaddr.virtual_base);
    kprintln!("[hydrogen] paging: own page tables active ({} x 2 MiB + {} x 4 KiB direct-map pages, {} kernel pages)",
        v.pages_2m, v.pages_4k, v.kernel_pages);
    kprintln!("[hydrogen] stack: switching to 64 KiB kernel stack (top {:#x}, guard page {:#x})",
        mm::vmm::STACK_TOP, mm::vmm::STACK_GUARD);

    mm::vmm::switch_stack(kmain_stage2)
}

extern "C" fn kmain_stage2() -> ! {
    kprintln!("[hydrogen] stack: now on kernel stack");

    // --- memory self-checks (heap comes alive on first use) ---
    selftest::physical_memory();
    selftest::paging();
    selftest::heap();
    let hs = mm::heap::stats();
    kprintln!("[hydrogen] heap: online at {:#x} ({} small-block pages, {} large pages in use)",
        mm::vmm::HEAP_START, hs.small_pages, hs.large_pages);

    // --- what the firmware says about the machine, and its devices ---
    let machine = drivers::acpi::init();
    drivers::pci::init(&machine);

    // --- interrupts and timer ---
    pic::disable();
    let a = apic::init();
    kprintln!("[hydrogen] apic: local APIC id {} at phys {:#x} ({} mode), legacy PIC masked",
        a.id, a.phys_base, if a.x2apic { "x2APIC" } else { "xAPIC" });
    let timer_text = match apic::start_timer(time::HZ) {
        Some(per_ms) => {
            x86_64::instructions::interrupts::enable();
            kprintln!("[hydrogen] timer: APIC timer calibrated at {} ticks/ms, running at {} Hz; interrupts on", per_ms, time::HZ);
            match selftest::timer_vs_rtc() {
                Some(n) if (90..=110).contains(&n) => {
                    kprintln!("[hydrogen] timer ok: {} ticks in one RTC second (expected 100 +/- 10)", n);
                    fmtbuf!(48, "{} Hz (checked: {}/s)", time::HZ, n)
                }
                Some(n) => {
                    kprintln!("[hydrogen] timer check FAILED: {} ticks in one RTC second (expected 100 +/- 10)", n);
                    fmtbuf!(48, "{} Hz (OFF: {}/s)", time::HZ, n)
                }
                None => {
                    kprintln!("[hydrogen] timer check FAILED: RTC second never changed");
                    fmtbuf!(48, "{} Hz (unchecked)", time::HZ)
                }
            }
        }
        None => {
            kprintln!("[hydrogen] timer check FAILED: PIT calibration timed out");
            fmtbuf!(48, "not running")
        }
    };

    // --- doors for user programs, and the CPU's protections against them ---
    let g = cpu::enable_user_guards();
    arch::x86_64::syscall::init();
    kprintln!("[hydrogen] user mode: syscall door open; SMEP {}, SMAP {}, UMIP {}", on(g.smep), on(g.smap), on(g.umip));
    if !(g.smep && g.smap) {
        kprintln!("[hydrogen] WARNING: this CPU lacks SMEP or SMAP; the kernel has less protection from programs");
    }

    // --- threads: first cooperative (the exact-order check needs that), then preemptive ---
    sched::init();
    kprintln!("[hydrogen] sched: boot path is now thread 0 (kmain), idle thread is 1");
    if x86_64::instructions::interrupts::are_enabled() {
        if SELF_TESTS {
            selftest::threads();
        }
        sched::enable_preemption();
        kprintln!("[hydrogen] sched: preemption on ({} ms time slices)", 1000 / time::HZ);
        if SELF_TESTS {
            selftest::preemption();
            selftest::user();
            kprintln!("[hydrogen] boot modules: {} programs", boot::modules().count());
            for (name, data) in boot::modules() {
                kprintln!("[hydrogen]   {} ({} bytes)", name, data.len());
            }
            selftest::elf();
            selftest::isolation();
        }
        #[cfg(feature = "fuzz-user")]
        selftest::fuzz::run();
    } else {
        kprintln!("[hydrogen] sched: no timer, so no preemption; thread checks FAILED (not run)");
    }

    // --- deliberate fault tests (only in their own test builds) ---
    #[cfg(feature = "fault-int3")]
    faulttest::breakpoint();
    #[cfg(feature = "fault-pagefault")]
    faulttest::page_fault();
    #[cfg(feature = "fault-stack")]
    faulttest::stack_overflow();
    #[cfg(feature = "fault-thread-stack")]
    faulttest::thread_stack_overflow();
    #[cfg(any(feature = "fault-smap", feature = "fault-smep"))]
    faulttest::user_memory();

    // --- boot screen ---
    let protection = if g.smep && g.smap { "NX, W^X, SMEP, SMAP, guards" } else { "NX, W^X, guard pages" };
    draw_boot_screen(timer_text.as_str(), protection);

    #[cfg(feature = "panic-test")]
    panic!("deliberate panic-test: the crash screen works");

    #[allow(unreachable_code)]
    {
        kprintln!("[hydrogen] phase 3 boot complete; idling with timer running");
        sched::exit() // from here the idle thread runs whenever nobody else needs the CPU
    }
}

fn on(b: bool) -> &'static str {
    if b { "on" } else { "MISSING" }
}

fn init_framebuffer() {
    let Some(resp) = boot::FRAMEBUFFER.response() else {
        kprintln!("[hydrogen] WARNING: no framebuffer from bootloader");
        return;
    };
    let Some(f) = resp.framebuffers().first() else { return };
    kprintln!("[hydrogen] framebuffer: {}x{} pitch {} bpp {}", f.width, f.height, f.pitch, f.bpp);
    if f.bpp != 32 {
        kprintln!("[hydrogen] WARNING: unsupported framebuffer depth, screen left blank");
        return;
    }
    // SAFETY: Limine guarantees the framebuffer is mapped (in the direct map, which our own
    // page tables keep), writable, at least pitch*height bytes and valid for good; 32 bpp checked.
    let fbo = unsafe {
        Framebuffer::new(f.address() as *mut u8, f.width as usize, f.height as usize, f.pitch as usize,
            (f.red_mask_shift, f.green_mask_shift, f.blue_mask_shift))
    };
    *FB.lock() = Some(fbo);
}

fn draw_boot_screen(timer: &str, protection: &str) {
    let bootloader = match boot::BOOTLOADER_INFO.response() {
        Some(r) => fmtbuf!(48, "{} {}", r.name(), r.version()),
        None => fmtbuf!(48, "unknown"),
    };
    let s = mm::pmm::stats();
    let memory = fmtbuf!(48, "{} MiB free of {} MiB",
        s.free_frames as u64 * 4096 / MIB, s.total_frames as u64 * 4096 / MIB);
    let mut fb = FB.lock();
    if let Some(fb) = fb.as_mut() {
        let display = fmtbuf!(48, "{} x {}, 32 bpp", fb.width, fb.height);
        splash::boot_screen(fb, &splash::BootFacts {
            bootloader: bootloader.as_str(),
            display: display.as_str(),
            memory: memory.as_str(),
            protection,
            timer,
        });
        kprintln!("[hydrogen] boot screen drawn");
    }
}
