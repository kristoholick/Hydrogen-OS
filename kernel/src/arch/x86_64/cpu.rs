//! Basic CPU control.

use x86_64::registers::control::{Cr0, Cr0Flags};
use x86_64::registers::model_specific::{Efer, EferFlags};

/// Mask maskable interrupts on this core.
#[inline]
pub fn disable_interrupts() {
    x86_64::instructions::interrupts::disable();
}

/// Stop this core forever with interrupts masked. Used after a panic.
pub fn halt_forever() -> ! {
    loop {
        disable_interrupts();
        x86_64::instructions::hlt();
    }
}

/// Sleep between interrupts forever (timer keeps ticking). Used at the end of boot.
pub fn idle_forever() -> ! {
    loop {
        x86_64::instructions::interrupts::enable_and_hlt();
    }
}

/// Does the CPU support the no-execute page bit?
pub fn has_nx() -> bool {
    // CPUID is always available in 64-bit mode; leaf 0x8000_0001 exists on every x86-64 CPU.
    let r = core::arch::x86_64::__cpuid(0x8000_0001);
    r.edx & (1 << 20) != 0
}

/// Turn on the protections our page tables rely on:
/// NX (no-execute bit honoured) and WP (kernel cannot write to read-only pages).
pub fn enable_memory_protection() {
    // SAFETY: NX support was checked by the caller (`has_nx`); setting EFER.NXE only makes
    // the CPU honour bit 63 of page entries. CR0.WP only adds write checks for ring 0.
    unsafe {
        Efer::update(|f| f.insert(EferFlags::NO_EXECUTE_ENABLE));
        Cr0::update(|f| f.insert(Cr0Flags::WRITE_PROTECT));
    }
}
