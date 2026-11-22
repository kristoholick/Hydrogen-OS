//! SIGIL kernel — entry point.
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
mod selftest;
mod splash;
mod sync;
mod time;

use arch::x86_64::{apic, cpu, gdt, idt, pic};
use drivers::framebuffer::{FB, Framebuffer};
use limine::memmap::MEMMAP_USABLE;

const MIB: u64 = 1024 * 1024;

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    drivers::serial::init();
    kprintln!();
    kprintln!("[sigil] SIGIL kernel online");

    if boot::BASE_REVISION.is_supported() {
        kprintln!("[sigil] limine base revision {} accepted", limine::BaseRevision::MAX_SUPPORTED);
    } else {
        kprintln!("[sigil] WARNING: base revision {} not supported (bootloader offers {:?})",
            limine::BaseRevision::MAX_SUPPORTED, boot::BASE_REVISION.actual_revision());
    }

    // --- CPU tables first, so every later mistake is reported instead of resetting the PC ---
    gdt::init();
    idt::init();
    let (ist_lo, ist_hi) = gdt::ist_range();
    kprintln!("[sigil] cpu tables: GDT+TSS loaded (emergency stacks {:#x}..{:#x}), IDT loaded (all 32 exceptions)", ist_lo, ist_hi);

    // --- framebuffer (so the crash screen works from here on) ---
    init_framebuffer();

    // --- memory protection bits ---
    assert!(cpu::has_nx(), "CPU lacks the no-execute (NX) feature");
    cpu::enable_memory_protection();
    kprintln!("[sigil] cpu: NX + write-protect enabled");

    // --- physical memory ---
    let memmap = boot::MEMMAP.response().expect("bootloader gave no memory map");
    let hhdm = boot::HHDM.response().expect("bootloader gave no direct-map offset").offset;
    let kaddr = boot::EXECUTABLE_ADDRESS.response().expect("bootloader gave no kernel address");
    let entries = memmap.entries();
    let usable: u64 = entries.iter().filter(|e| e.type_ == MEMMAP_USABLE).map(|e| e.length).sum();
    kprintln!("[sigil] memory map: {} entries, {} MiB usable, direct map at {:#x}", entries.len(), usable / MIB, hhdm);

    let p = mm::pmm::init(entries, hhdm);
    kprintln!("[sigil] pmm: {} frames tracked, {} free ({} MiB), bitmap {} KiB at phys {:#x}",
        p.stats.total_frames, p.stats.free_frames, p.stats.free_frames as u64 * 4096 / MIB,
        p.bitmap_bytes / 1024, p.bitmap_phys);

    // --- our own page tables ---
    let rsp: u64;
    // SAFETY: reads the stack pointer register; no side effects.
    unsafe { core::arch::asm!("mov {}, rsp", out(reg) rsp, options(nomem, nostack)) };
    assert!(rsp >= hhdm, "boot stack {rsp:#x} is outside the direct map; cannot switch page tables safely");
    let v = mm::vmm::init(entries, hhdm, kaddr.physical_base, kaddr.virtual_base);
    kprintln!("[sigil] paging: own page tables active ({} x 2 MiB + {} x 4 KiB direct-map pages, {} kernel pages)",
        v.pages_2m, v.pages_4k, v.kernel_pages);
    kprintln!("[sigil] stack: switching to 64 KiB kernel stack (top {:#x}, guard page {:#x})",
        mm::vmm::STACK_TOP, mm::vmm::STACK_GUARD);

    mm::vmm::switch_stack(kmain_stage2)
}

extern "C" fn kmain_stage2() -> ! {
    kprintln!("[sigil] stack: now on kernel stack");

    // --- memory self-checks (heap comes alive on first use) ---
    selftest::physical_memory();
    selftest::paging();
    selftest::heap();
    let hs = mm::heap::stats();
    kprintln!("[sigil] heap: online at {:#x} ({} small-block pages, {} large pages in use)",
        mm::vmm::HEAP_START, hs.small_pages, hs.large_pages);

    // --- interrupts and timer ---
    pic::disable();
    let a = apic::init();
    kprintln!("[sigil] apic: local APIC id {} at phys {:#x} ({} mode), legacy PIC masked",
        a.id, a.phys_base, if a.x2apic { "x2APIC" } else { "xAPIC" });
    let timer_text = match apic::start_timer(time::HZ) {
        Some(per_ms) => {
            x86_64::instructions::interrupts::enable();
            kprintln!("[sigil] timer: APIC timer calibrated at {} ticks/ms, running at {} Hz; interrupts on", per_ms, time::HZ);
            match selftest::timer_vs_rtc() {
                Some(n) if (90..=110).contains(&n) => {
                    kprintln!("[sigil] timer ok: {} ticks in one RTC second (expected 100 +/- 10)", n);
                    fmtbuf!(48, "{} Hz (checked: {}/s)", time::HZ, n)
                }
                Some(n) => {
                    kprintln!("[sigil] timer check FAILED: {} ticks in one RTC second (expected 100 +/- 10)", n);
                    fmtbuf!(48, "{} Hz (OFF: {}/s)", time::HZ, n)
                }
                None => {
                    kprintln!("[sigil] timer check FAILED: RTC second never changed");
                    fmtbuf!(48, "{} Hz (unchecked)", time::HZ)
                }
            }
        }
        None => {
            kprintln!("[sigil] timer check FAILED: PIT calibration timed out");
            fmtbuf!(48, "not running")
        }
    };

    // --- deliberate fault tests (only in their own test builds) ---
    #[cfg(feature = "fault-int3")]
    faulttest::breakpoint();
    #[cfg(feature = "fault-pagefault")]
    faulttest::page_fault();
    #[cfg(feature = "fault-stack")]
    faulttest::stack_overflow();

    // --- boot screen ---
    draw_boot_screen(timer_text.as_str());

    #[cfg(feature = "panic-test")]
    panic!("deliberate panic-test: the crash screen works");

    #[allow(unreachable_code)]
    {
        kprintln!("[sigil] phase 2 boot complete; idling with timer running");
        arch::idle_forever()
    }
}

fn init_framebuffer() {
    let Some(resp) = boot::FRAMEBUFFER.response() else {
        kprintln!("[sigil] WARNING: no framebuffer from bootloader");
        return;
    };
    let Some(f) = resp.framebuffers().first() else { return };
    kprintln!("[sigil] framebuffer: {}x{} pitch {} bpp {}", f.width, f.height, f.pitch, f.bpp);
    if f.bpp != 32 {
        kprintln!("[sigil] WARNING: unsupported framebuffer depth, screen left blank");
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

fn draw_boot_screen(timer: &str) {
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
            protection: "NX, W^X, guard page",
            timer,
        });
        kprintln!("[sigil] boot screen drawn");
    }
}
