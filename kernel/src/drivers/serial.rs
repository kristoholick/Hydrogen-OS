//! 16550 UART on COM1 — the kernel's first and most reliable log output.

use crate::arch::x86_64::io::{inb, outb};
use core::fmt;
use spin::Mutex;

const COM1: u16 = 0x3F8;

pub struct SerialPort {
    base: u16,
    ready: bool,
}

impl SerialPort {
    const fn new(base: u16) -> Self {
        Self { base, ready: false }
    }

    fn init(&mut self) {
        let b = self.base;
        // SAFETY: COM1 (0x3F8..0x3FF) is a standard legacy port owned solely by this driver;
        // this is the documented 16550 init sequence (38400 baud, 8N1, FIFO on).
        unsafe {
            outb(b + 1, 0x00); // no UART interrupts
            outb(b + 3, 0x80); // DLAB on to set the baud divisor
            outb(b, 0x03); //     divisor low  (115200 / 3 = 38400 baud)
            outb(b + 1, 0x00); // divisor high
            outb(b + 3, 0x03); // 8 bits, no parity, 1 stop bit, DLAB off
            outb(b + 2, 0xC7); // enable + clear FIFOs, 14-byte threshold
            outb(b + 4, 0x03); // DTR + RTS
        }
        self.ready = true;
    }

    fn write_byte(&mut self, byte: u8) {
        if !self.ready {
            return;
        }
        // SAFETY: same port ownership as in `init`. Line Status bit 5 = transmit buffer empty.
        unsafe {
            let mut spins = 0u32;
            while inb(self.base + 5) & 0x20 == 0 && spins < 100_000 {
                spins += 1;
            }
            outb(self.base, byte);
        }
    }
}

impl fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            if b == b'\n' {
                self.write_byte(b'\r');
            }
            self.write_byte(b);
        }
        Ok(())
    }
}

pub static SERIAL: Mutex<SerialPort> = Mutex::new(SerialPort::new(COM1));

pub fn init() {
    SERIAL.lock().init();
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use fmt::Write;
    let _ = SERIAL.lock().write_fmt(args);
}

/// Panic-path print: takes the port even if a panicking context still holds the lock.
pub fn print_unlocked(args: fmt::Arguments) {
    use fmt::Write;
    // SAFETY: called only from the panic handler after interrupts are masked on a
    // single core, so no other code can be using the port concurrently.
    unsafe { SERIAL.force_unlock() };
    let _ = SERIAL.lock().write_fmt(args);
}

#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => { $crate::drivers::serial::_print(format_args!($($arg)*)) };
}

#[macro_export]
macro_rules! kprintln {
    () => { $crate::kprint!("\n") };
    ($($arg:tt)*) => { $crate::kprint!("{}\n", format_args!($($arg)*)) };
}
