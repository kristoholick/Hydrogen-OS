//! System calls: what programs can ask the kernel to do. CPU-independent; the entry code is in
//! `arch`. The numbers are Hydrogen's own.

use crate::arch::x86_64::space::copy_from_user;
use crate::{kprintln, proc, sched, time};
use alloc::string::String;
use alloc::vec::Vec;

const LOG: u64 = 0;
const EXIT: u64 = 1;
const YIELD: u64 = 2;
const SLEEP: u64 = 3;
const GETPID: u64 = 4;
const UPTIME_MS: u64 = 5;

/// Longest text one `log` call takes, in bytes.
const LOG_MAX: u64 = 1024;

/// Errors are returned as negative numbers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(i64)]
pub enum Error {
    NoSuchCall = -1,
    BadAddress = -2,
    TooLong = -3,
}

/// Runs on the calling thread's kernel stack with interrupts on and no lock held.
pub fn dispatch(number: u64, a: [u64; 6]) -> i64 {
    let result = match number {
        LOG => log(a[0], a[1]),
        EXIT => proc::exit_current(a[0] as i32),
        YIELD => {
            sched::yield_now();
            Ok(0)
        }
        SLEEP => {
            // random code asks for random times: do not wait for those
            sched::sleep_ms(if proc::is_fuzz() { a[0] % 20 } else { a[0] });
            Ok(0)
        }
        GETPID => Ok(sched::current_id() as i64),
        UPTIME_MS => Ok((time::ticks() * 1000 / time::HZ as u64) as i64),
        _ => Err(Error::NoSuchCall),
    };
    result.unwrap_or_else(|e| e as i64)
}

/// Write text to the system log. Every line is tagged with the program's name and number and
/// control characters are replaced, so a program cannot forge the kernel's own log lines.
fn log(ptr: u64, len: u64) -> Result<i64, Error> {
    if len > LOG_MAX {
        return Err(Error::TooLong);
    }
    let mut bytes: Vec<u8> = alloc::vec![0; len as usize];
    copy_from_user(ptr, &mut bytes).ok_or(Error::BadAddress)?;
    if proc::is_fuzz() {
        return Ok(len as i64);
    }
    let name = proc::current_name();
    let pid = sched::current_id();
    for line in String::from_utf8_lossy(&bytes).lines() {
        let clean: String = line.chars().map(|c| if c.is_control() { '?' } else { c }).collect();
        kprintln!("[{} {}] {}", name.as_str(), pid, clean);
    }
    Ok(len as i64)
}
