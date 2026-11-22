//! Basic CPU control.

/// Mask maskable interrupts on this core.
#[inline]
pub fn disable_interrupts() {
    // SAFETY: `cli` only clears IF. The kernel runs in ring 0, where it is always allowed,
    // and it has no memory or stack effects.
    unsafe { core::arch::asm!("cli", options(nomem, nostack)) };
}

/// Stop this core forever with interrupts masked. Used at the end of boot and after a panic.
pub fn halt_forever() -> ! {
    loop {
        disable_interrupts();
        // SAFETY: `hlt` pauses the core until the next interrupt; with IF cleared only an
        // NMI can wake it, and the loop puts it straight back to sleep.
        unsafe { core::arch::asm!("hlt", options(nomem, nostack)) };
    }
}
