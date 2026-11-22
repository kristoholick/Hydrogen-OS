//! System tick counter, driven by the Local APIC timer.

use core::sync::atomic::{AtomicU64, Ordering};

pub const HZ: u32 = 100;
static TICKS: AtomicU64 = AtomicU64::new(0);

/// Called from the timer interrupt.
pub fn on_tick() {
    TICKS.fetch_add(1, Ordering::Relaxed);
}

pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}
