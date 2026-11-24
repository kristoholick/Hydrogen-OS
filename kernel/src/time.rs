//! System tick counter, driven by the Local APIC timer.

use core::sync::atomic::{AtomicU64, Ordering};

pub const HZ: u32 = 100;
static TICKS: AtomicU64 = AtomicU64::new(0);
static USER_TICKS: AtomicU64 = AtomicU64::new(0);

/// Called from the timer interrupt; `user` = it interrupted a program in ring 3.
pub fn on_tick(user: bool) {
    TICKS.fetch_add(1, Ordering::Relaxed);
    if user {
        USER_TICKS.fetch_add(1, Ordering::Relaxed);
    }
}

/// Ticks that arrived while a program was running in ring 3.
pub fn user_ticks() -> u64 {
    USER_TICKS.load(Ordering::Relaxed)
}

pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}
