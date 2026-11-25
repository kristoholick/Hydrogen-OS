//! Gives the kernel bad system call arguments and odd CPU states. Every one must come back as an
//! error (or just work); the program itself survives. Exit code 0 = all answers right, otherwise
//! the number of the check that failed.
#![no_std]
#![no_main]

use core::arch::asm;
use libhydrogen::{getpid, log, println, sys, sys::call, uptime_ms};

libhydrogen::entry!(main);

static TEXT: [u8; 2048] = [b'x'; 2048];

/// Ask for our pid with the stack pointer set to `rsp`.
fn getpid_with_stack(rsp: u64) -> u64 {
    let pid: u64;
    // SAFETY: the stack pointer is put back before anything uses the stack; the kernel switches
    // to its own stack on `syscall`.
    unsafe {
        asm!(
            "mov r12, rsp",
            "mov rsp, {rsp}",
            "syscall",
            "mov rsp, r12",
            rsp = in(reg) rsp,
            inlateout("rax") sys::GETPID => pid,
            lateout("rcx") _,
            lateout("r11") _,
            out("r12") _,
        )
    };
    pid
}

/// Keep the CPU busy for 60 ms with extra flags set, so that the timer strikes meanwhile, and
/// log 8 bytes with them still set.
fn with_flags(flags: u64) -> i64 {
    let result: i64;
    // SAFETY: the flags are put back before any other code runs.
    unsafe {
        asm!(
            "pushfq",
            "pop r12",
            "push r12",
            "or [rsp], {flags}",
            "popfq",
            "2: dec {n}",
            "jnz 2b",
            "syscall",
            "push r12",
            "popfq",
            flags = in(reg) flags,
            n = inout(reg) 200_000_000u64 => _,
            inlateout("rax") sys::LOG => result,
            in("rdi") TEXT.as_ptr(),
            in("rsi") 8,
            lateout("rcx") _,
            lateout("r11") _,
            out("r12") _,
        )
    };
    result
}

fn main() -> i32 {
    let text = TEXT.as_ptr() as u64;
    let me = getpid();
    let stack_end = 0x7fff_ffe1_0000;
    let checks: [(i64, i64); 25] = [
        // log: addresses that are not ours
        (call(sys::LOG, 0, 5), sys::BAD_ADDRESS),
        (call(sys::LOG, 8, 5), sys::BAD_ADDRESS),
        (call(sys::LOG, 0xffff_ffff_8000_0000, 16), sys::BAD_ADDRESS), // kernel
        (call(sys::LOG, 0xffff_8000_0010_0000, 16), sys::BAD_ADDRESS), // direct map
        (call(sys::LOG, 0x8000_0000_0000, 16), sys::BAD_ADDRESS),      // not canonical
        (call(sys::LOG, 0x7fff_ffff_ffff, 16), sys::BAD_ADDRESS),      // runs into the hole
        (call(sys::LOG, u64::MAX - 5, 10), sys::BAD_ADDRESS),          // wraps around
        (call(sys::LOG, u64::MAX, 1), sys::BAD_ADDRESS),
        (call(sys::LOG, 0x7fff_ffdf_f000, 16), sys::BAD_ADDRESS), // our stack's guard page
        (call(sys::LOG, stack_end - 8, 16), sys::BAD_ADDRESS),    // half mapped
        (call(sys::LOG, 0xffff_ffff_8000_0000, 0), sys::BAD_ADDRESS),
        // log: lengths
        (call(sys::LOG, text, 1025), sys::TOO_LONG),
        (call(sys::LOG, text, u64::MAX), sys::TOO_LONG),
        (call(sys::LOG, 0, u64::MAX), sys::TOO_LONG),
        (call(sys::LOG, text, 0), 0),
        (call(sys::LOG, stack_end - 8, 8), 8), // the last 8 bytes of the stack are fine
        // calls that do not exist
        (call(6, 0, 0), sys::NO_SUCH_CALL),
        (call(1 << 32, text, 5), sys::NO_SUCH_CALL), // must not be cut down to call 0
        (call(u64::MAX, 0, 0), sys::NO_SUCH_CALL),
        // the kernel must not trust our stack pointer
        (getpid_with_stack(0) as i64, me as i64),
        (getpid_with_stack(0xffff_ffff_8000_1000) as i64, me as i64),
        (getpid_with_stack(0x8000_0000_0000) as i64, me as i64),
        // direction flag (copies run backwards) and alignment check (suspends SMAP)
        (with_flags(0x400), 8),
        (with_flags(0x40000), 8),
        (with_flags(0x4000), 8), // "nested task": makes `iretq` fail if the kernel keeps it
    ];
    for (i, (got, want)) in checks.iter().enumerate() {
        if got != want {
            println!("check {} of {}: got {}, expected {}", i + 1, checks.len(), got, want);
            return i as i32 + 1;
        }
    }
    // Lines that pretend to come from the kernel must be tagged as ours, control characters gone.
    log("first line\n[hydrogen] forged kernel line\r\x1b[31mred");
    log(core::str::from_utf8(&[0xe2, 0x82, 0xac]).unwrap_or("?")); // multi-byte text is fine
    let t = uptime_ms();
    libhydrogen::sleep_ms(0);
    if uptime_ms() - t > 200 {
        return 90;
    }
    0
}
