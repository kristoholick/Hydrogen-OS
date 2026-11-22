//! SIGIL kernel — entry point.
//!
//! Boot path: UEFI firmware → Limine → `kmain` (higher half, long mode, Limine's page tables).
//! Phase 1 scope: serial log, framebuffer splash, panic screen, then halt.

#![no_std]
#![no_main]

mod arch;
mod boot;
mod drivers;
mod fmtbuf;
mod panic;
mod splash;

use drivers::framebuffer::{FB, Framebuffer};
use limine::memmap::MEMMAP_USABLE;

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    drivers::serial::init();
    kprintln!();
    kprintln!("[sigil] SIGIL kernel online");

    // --- boot protocol revision ---
    let protocol = if boot::BASE_REVISION.is_supported() {
        kprintln!("[sigil] limine base revision {} accepted", limine::BaseRevision::MAX_SUPPORTED);
        fmtbuf!(48, "base revision {}", limine::BaseRevision::MAX_SUPPORTED)
    } else {
        let actual = boot::BASE_REVISION.actual_revision();
        kprintln!("[sigil] WARNING: base revision {} not supported (bootloader offers {:?})",
            limine::BaseRevision::MAX_SUPPORTED, actual);
        fmtbuf!(48, "revision fallback {:?}", actual)
    };

    // --- bootloader ---
    let bootloader = match boot::BOOTLOADER_INFO.response() {
        Some(r) => fmtbuf!(48, "{} {}", r.name(), r.version()),
        None => fmtbuf!(48, "unknown"),
    };
    kprintln!("[sigil] bootloader: {}", bootloader.as_str());

    // --- memory map (read only; the allocator comes in Phase 2) ---
    let mut usable: u64 = 0;
    if let Some(mm) = boot::MEMMAP.response() {
        let entries = mm.entries();
        for e in entries {
            if e.type_ == MEMMAP_USABLE {
                usable += e.length;
            }
        }
        kprintln!("[sigil] memory map: {} entries, {} MiB usable", entries.len(), usable / (1024 * 1024));
    } else {
        kprintln!("[sigil] WARNING: no memory map from bootloader");
    }
    let memory = fmtbuf!(48, "{} MiB usable", usable / (1024 * 1024));

    if let Some(h) = boot::HHDM.response() {
        kprintln!("[sigil] higher-half direct map at {:#x}", h.offset);
    }

    // --- framebuffer ---
    let mut display = fmtbuf!(48, "none");
    if let Some(resp) = boot::FRAMEBUFFER.response() {
        if let Some(f) = resp.framebuffers().first() {
            kprintln!("[sigil] framebuffer: {}x{} pitch {} bpp {}", f.width, f.height, f.pitch, f.bpp);
            if f.bpp == 32 {
                // SAFETY: Limine guarantees the framebuffer is mapped, writable and at least
                // pitch*height bytes, and stays valid; we checked it is 32 bpp.
                let fbo = unsafe {
                    Framebuffer::new(
                        f.address() as *mut u8,
                        f.width as usize,
                        f.height as usize,
                        f.pitch as usize,
                        (f.red_mask_shift, f.green_mask_shift, f.blue_mask_shift),
                    )
                };
                *FB.lock() = Some(fbo);
                display = fmtbuf!(48, "{} x {}, {} bpp", f.width, f.height, f.bpp);
            } else {
                kprintln!("[sigil] WARNING: unsupported framebuffer depth, screen left blank");
            }
        }
    } else {
        kprintln!("[sigil] WARNING: no framebuffer from bootloader");
    }

    if let Some(fb) = FB.lock().as_mut() {
        splash::boot_screen(fb, &splash::BootFacts {
            bootloader: bootloader.as_str(),
            display: display.as_str(),
            memory: memory.as_str(),
            protocol: protocol.as_str(),
        });
        kprintln!("[sigil] boot screen drawn");
    }

    #[cfg(feature = "panic-test")]
    panic!("deliberate panic-test: the crash screen works");

    #[allow(unreachable_code)]
    {
        kprintln!("[sigil] phase 1 boot complete; halting");
        arch::halt_forever()
    }
}
