//! CMOS real-time clock — used only as an independent reference to check the APIC timer.

use super::io::{inb, outb};

/// Current seconds value (raw BCD or binary — we only compare it for changes).
pub fn seconds() -> u8 {
    // SAFETY: ports 0x70/0x71 are the CMOS index/data pair; register 0x00 is the RTC
    // seconds counter. Reading it changes nothing. (Bit 7 of 0x70 left clear: NMI stays on.)
    unsafe {
        outb(0x70, 0x00);
        inb(0x71)
    }
}
