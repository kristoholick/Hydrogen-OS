//! Checks that the loader put everything where it belongs: constants, initialised and zeroed
//! variables, and the stack. Exit code 0 = all fine.
#![no_std]
#![no_main]

use core::sync::atomic::{AtomicU64, Ordering::Relaxed};
use libhydrogen::{getpid, println, sleep_ms};

libhydrogen::entry!(main);

static TABLE: [u32; 2000] = {
    let mut t = [0; 2000];
    let mut i = 0;
    while i < t.len() {
        t[i] = (i * i) as u32;
        i += 1;
    }
    t
};
static READY: [AtomicU64; 1500] = [const { AtomicU64::new(0x1234_5678_9abc_def0) }; 1500];
static ZERO: [AtomicU64; 5000] = [const { AtomicU64::new(0) }; 5000];

#[inline(never)]
fn deep(n: u64) -> u64 {
    let pad = core::hint::black_box([n; 64]);
    if n == 0 { pad[63] } else { deep(n - 1) + pad[(n % 64) as usize] }
}

fn main() -> i32 {
    let table = core::hint::black_box(&TABLE);
    if table.iter().enumerate().any(|(i, &v)| v as usize != i * i) {
        return 1;
    }
    if READY.iter().any(|v| v.load(Relaxed) != 0x1234_5678_9abc_def0) {
        return 2;
    }
    if ZERO.iter().any(|v| v.load(Relaxed) != 0) {
        return 3;
    }
    // Two copies run at once: variables must be private.
    let me = getpid();
    ZERO.iter().chain(&READY).for_each(|v| v.store(me, Relaxed));
    sleep_ms(50);
    if ZERO.iter().chain(&READY).any(|v| v.load(Relaxed) != me) {
        return 4;
    }
    if deep(40) != 40 * 41 / 2 {
        return 5; // uses over half of the 64 KiB stack
    }
    println!("constants, variables and stack are fine");
    0
}
