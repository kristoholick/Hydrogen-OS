//! x86 port I/O. The only place in the kernel that executes `in`/`out`.

/// Write one byte to an I/O port.
///
/// # Safety
/// The caller must run in ring 0 and own the device behind `port`; writing to an
/// arbitrary port can reconfigure hardware.
#[inline]
pub unsafe fn outb(port: u16, value: u8) {
    // SAFETY: caller guarantees ring 0 and exclusive ownership of `port` (see fn docs).
    // The instruction touches no memory and no stack, and preserves flags.
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port, in("al") value,
            options(nomem, nostack, preserves_flags));
    }
}

/// Read one byte from an I/O port.
///
/// # Safety
/// Same requirements as [`outb`]; some device registers change state when read.
#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    // SAFETY: caller guarantees ring 0 and exclusive ownership of `port` (see fn docs).
    unsafe {
        core::arch::asm!("in al, dx", in("dx") port, out("al") value,
            options(nomem, nostack, preserves_flags));
    }
    value
}

/// Write 32 bits to an I/O port.
///
/// # Safety
/// Same requirements as [`outb`].
#[inline]
pub unsafe fn outl(port: u16, value: u32) {
    // SAFETY: caller guarantees ring 0 and exclusive ownership of `port` (see `outb`).
    unsafe {
        core::arch::asm!("out dx, eax", in("dx") port, in("eax") value,
            options(nomem, nostack, preserves_flags));
    }
}

/// Read 32 bits from an I/O port.
///
/// # Safety
/// Same requirements as [`inb`].
#[inline]
pub unsafe fn inl(port: u16) -> u32 {
    let value: u32;
    // SAFETY: caller guarantees ring 0 and exclusive ownership of `port` (see `outb`).
    unsafe {
        core::arch::asm!("in eax, dx", in("dx") port, out("eax") value,
            options(nomem, nostack, preserves_flags));
    }
    value
}
