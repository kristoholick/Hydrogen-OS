//! Tiny runtime for Hydrogen programs: start-up, system calls, printing.
//!
//! ```ignore
//! #![no_std]
//! #![no_main]
//! libhydrogen::entry!(main);
//! fn main() -> i32 { libhydrogen::println!("hi"); 0 }
//! ```

#![no_std]

use core::fmt;
use core::sync::atomic::{AtomicU64, Ordering};

pub mod sys {
    //! Raw system calls. What they do: `kernel/src/syscall/mod.rs`.

    pub const LOG: u64 = 0;
    pub const EXIT: u64 = 1;
    pub const YIELD: u64 = 2;
    pub const SLEEP: u64 = 3;
    pub const GETPID: u64 = 4;
    pub const UPTIME_MS: u64 = 5;

    pub const NO_SUCH_CALL: i64 = -1;
    pub const BAD_ADDRESS: i64 = -2;
    pub const TOO_LONG: i64 = -3;

    /// Any system call with up to two arguments. Safe: the kernel checks everything, and no
    /// call writes to this program's memory.
    pub fn call(number: u64, a: u64, b: u64) -> i64 {
        let result: i64;
        // SAFETY: `syscall` overwrites only rax, rcx and r11, all declared here.
        unsafe {
            core::arch::asm!(
                "syscall",
                inlateout("rax") number => result,
                in("rdi") a,
                in("rsi") b,
                lateout("rcx") _,
                lateout("r11") _,
                options(nostack)
            )
        };
        result
    }
}

static ARG: AtomicU64 = AtomicU64::new(0);

#[doc(hidden)]
pub fn set_arg(arg: u64) {
    ARG.store(arg, Ordering::Relaxed);
}

/// The number the program was started with (until there are real command lines).
pub fn arg() -> u64 {
    ARG.load(Ordering::Relaxed)
}

/// Write text to the system log. Returns the number of bytes, or a negative error.
pub fn log(text: &str) -> i64 {
    sys::call(sys::LOG, text.as_ptr() as u64, text.len() as u64)
}

pub fn exit(code: i32) -> ! {
    sys::call(sys::EXIT, code as u64, 0);
    unreachable!()
}

pub fn yield_now() {
    sys::call(sys::YIELD, 0, 0);
}

pub fn sleep_ms(ms: u64) {
    sys::call(sys::SLEEP, ms, 0);
}

pub fn getpid() -> u64 {
    sys::call(sys::GETPID, 0, 0) as u64
}

pub fn uptime_ms() -> u64 {
    sys::call(sys::UPTIME_MS, 0, 0) as u64
}

/// One `println!` becomes one `log` call; longer text is cut off.
struct Line {
    buf: [u8; 512],
    len: usize,
}

impl fmt::Write for Line {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            let mut tmp = [0; 4];
            let b = c.encode_utf8(&mut tmp).as_bytes();
            let Some(room) = self.buf.get_mut(self.len..self.len + b.len()) else { break };
            room.copy_from_slice(b);
            self.len += b.len();
        }
        Ok(())
    }
}

#[doc(hidden)]
pub fn print(args: fmt::Arguments) {
    let mut line = Line { buf: [0; 512], len: 0 };
    let _ = fmt::Write::write_fmt(&mut line, args);
    sys::call(sys::LOG, line.buf.as_ptr() as u64, line.len as u64);
}

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => { $crate::print(format_args!($($arg)*)) };
}

/// Names the program's main function: `fn() -> i32`, whose result is the exit code.
#[macro_export]
macro_rules! entry {
    ($main:path) => {
        // The kernel jumps here with rsp 16-byte aligned; `call` gives main the alignment the
        // ABI expects.
        core::arch::global_asm!(
            ".section .text._start, \"ax\"",
            ".global _start",
            "_start:",
            "    xor ebp, ebp",
            "    call {start}", // rdi = start argument
            "    ud2",
            start = sym start,
        );
        extern "C" fn start(arg: u64) -> ! {
            $crate::set_arg(arg);
            $crate::exit($main())
        }
    };
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("panic: {}", info);
    exit(101)
}
