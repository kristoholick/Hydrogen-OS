//! 16550 UART on COM1 — the kernel's first and most reliable log output.
//!
//! The port is slow (65 characters take about 17 ms at 38400 baud), so text first goes into a ring
//! and is then sent a byte at a time, with the lock released in between. Sending a whole
//! line under the lock kept interrupts off long enough to lose timer ticks. Each line enters
//! the ring in one piece, so lines from different threads never mix.

use crate::arch::x86_64::io::{inb, outb};
use crate::sync::IrqMutex;
use core::fmt;

const COM1: u16 = 0x3F8;
const RING: usize = 4096;
/// Bytes sent per lock. One byte takes about 0.26 ms at 38400 baud, but up to 14 ms was measured
/// in QEMU on a busy PC, so no more than one: the timer interrupt gets in after every byte.
const CHUNK: usize = 1;

pub struct SerialPort {
    base: u16,
    ready: bool,
    ring: [u8; RING],
    head: usize, // next byte to send
    len: usize,  // bytes waiting
}

impl SerialPort {
    const fn new(base: u16) -> Self {
        Self { base, ready: false, ring: [0; RING], head: 0, len: 0 }
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

    fn queue_byte(&mut self, byte: u8) {
        if !self.ready {
            return;
        }
        if self.len == RING {
            self.send_chunk(); // full: make room rather than lose log text
        }
        self.ring[(self.head + self.len) % RING] = byte;
        self.len += 1;
    }

    /// Send up to `CHUNK` waiting bytes. Returns whether more are waiting.
    fn send_chunk(&mut self) -> bool {
        for _ in 0..CHUNK.min(self.len) {
            self.send_byte(self.ring[self.head]);
            self.head = (self.head + 1) % RING;
            self.len -= 1;
        }
        self.len > 0
    }

    fn send_byte(&mut self, byte: u8) {
        // SAFETY: same port ownership as in `init`. Line Status bit 5 = transmit buffer empty.
        unsafe {
            let mut spins = 0u32;
            while inb(self.base + 5) & 0x20 == 0 && spins < 100_000 {
                spins += 1;
            }
            outb(self.base, byte);
            #[cfg(feature = "slow-serial")]
            {
                let start = core::arch::x86_64::_rdtsc();
                // about 0.26 ms at 3 GHz
                while core::arch::x86_64::_rdtsc() - start < 800_000 {}
            }
        }
    }
}

impl fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            if b == b'\n' {
                self.queue_byte(b'\r');
            }
            self.queue_byte(b);
        }
        Ok(())
    }
}

pub static SERIAL: IrqMutex<SerialPort> = IrqMutex::new(SerialPort::new(COM1));

pub fn init() {
    SERIAL.lock().init();
}

/// Counts the bytes a text takes on the wire.
struct Size(usize);

impl fmt::Write for Size {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0 += s.len() + s.bytes().filter(|&b| b == b'\n').count();
        Ok(())
    }
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use fmt::Write;
    // Measure first: the text goes into the ring in one piece, and only when it fits, because
    // making room byte by byte would keep interrupts off for as long as sending takes.
    let mut size = Size(0);
    let _ = size.write_fmt(args);
    loop {
        let mut port = SERIAL.lock();
        if port.len + size.0 <= RING || port.len == 0 {
            let _ = port.write_fmt(args);
            break;
        }
        port.send_chunk();
    }
    while SERIAL.lock().send_chunk() {} // the lock is free between chunks
}

/// Panic-path print: takes the port even if a panicking context still holds the lock, and
/// sends everything that was still waiting first.
pub fn print_unlocked(args: fmt::Arguments) {
    use fmt::Write;
    // SAFETY: called only from the panic handler after interrupts are masked on a
    // single core, so no other code can be using the port concurrently.
    unsafe { SERIAL.force_unlock() };
    let mut port = SERIAL.lock();
    let _ = port.write_fmt(args);
    while port.send_chunk() {}
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
