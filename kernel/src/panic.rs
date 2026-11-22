//! Kernel panic handler: log to serial, paint the crimson screen, halt the core.

use crate::arch::x86_64::cpu::halt_forever;
use x86_64::instructions::interrupts::disable as disable_interrupts;
use crate::drivers::{framebuffer::FB, serial::print_unlocked};
use crate::{fmtbuf, splash};
use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    disable_interrupts();

    let location = match info.location() {
        Some(l) => fmtbuf!(128, "{}:{}:{}", l.file(), l.line(), l.column()),
        None => fmtbuf!(128, "unknown location"),
    };
    let message = fmtbuf!(512, "{}", info.message());

    print_unlocked(format_args!("\n[sigil] PANIC at {}: {}\n", location.as_str(), message.as_str()));
    print_unlocked(format_args!("[sigil] halted\n"));

    // SAFETY: interrupts are off and only one core runs (no SMP yet), so whatever held the
    // framebuffer lock can never release it; taking it over is the only way to report.
    unsafe { FB.force_unlock() };
    if let Some(fb) = FB.lock().as_mut() {
        splash::panic_screen(fb, location.as_str(), message.as_str());
    }

    halt_forever()
}
