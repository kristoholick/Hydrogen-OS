//! Fixed-size string buffer for formatting text before we have a heap (Phase 2).

use core::fmt;

pub struct FmtBuf<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> FmtBuf<N> {
    pub const fn new() -> Self {
        Self { buf: [0; N], len: 0 }
    }

    pub fn as_str(&self) -> &str {
        // Only whole UTF-8 strings are ever appended (truncation happens on char boundaries).
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("?")
    }
}

impl<const N: usize> fmt::Write for FmtBuf<N> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for ch in s.chars() {
            let mut tmp = [0u8; 4];
            let enc = ch.encode_utf8(&mut tmp).as_bytes();
            if self.len + enc.len() > N {
                return Ok(()); // silently truncate
            }
            self.buf[self.len..self.len + enc.len()].copy_from_slice(enc);
            self.len += enc.len();
        }
        Ok(())
    }
}

/// `fmtbuf!(64, "x = {}", x)` → a `FmtBuf<64>` holding the formatted text.
#[macro_export]
macro_rules! fmtbuf {
    ($n:literal, $($arg:tt)*) => {{
        let mut b = $crate::fmtbuf::FmtBuf::<$n>::new();
        let _ = core::fmt::Write::write_fmt(&mut b, format_args!($($arg)*));
        b
    }};
}
