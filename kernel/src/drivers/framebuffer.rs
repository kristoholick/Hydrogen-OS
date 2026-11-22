//! Linear framebuffer handed over by the bootloader (UEFI GOP), plus tiny 2D helpers.
//! Phase 6 replaces the drawing code with the real compositor.

use crate::sync::IrqMutex;
use font8x8::legacy::BASIC_LEGACY;

#[derive(Clone, Copy)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const fn hex(v: u32) -> Self {
        Self((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }
    pub fn mix(self, other: Rgb, t_256: u32) -> Rgb {
        let m = |a: u8, b: u8| ((a as u32 * (256 - t_256) + b as u32 * t_256) >> 8) as u8;
        Rgb(m(self.0, other.0), m(self.1, other.1), m(self.2, other.2))
    }
}

pub struct Framebuffer {
    base: *mut u8,
    pub width: usize,
    pub height: usize,
    pitch: usize,
    shifts: (u8, u8, u8),
}

// SAFETY: the framebuffer memory is a plain MMIO/RAM region with no thread affinity;
// all access goes through the `FB` mutex.
unsafe impl Send for Framebuffer {}

pub static FB: IrqMutex<Option<Framebuffer>> = IrqMutex::new(None);

impl Framebuffer {
    /// # Safety
    /// `base` must point to a mapped, writable framebuffer of at least `pitch * height`
    /// bytes in 32-bpp format, valid for the rest of the kernel's life.
    pub unsafe fn new(base: *mut u8, width: usize, height: usize, pitch: usize, shifts: (u8, u8, u8)) -> Self {
        Self { base, width, height, pitch, shifts }
    }

    #[inline]
    fn encode(&self, c: Rgb) -> u32 {
        ((c.0 as u32) << self.shifts.0) | ((c.1 as u32) << self.shifts.1) | ((c.2 as u32) << self.shifts.2)
    }

    #[inline]
    pub fn put(&mut self, x: usize, y: usize, c: Rgb) {
        if x >= self.width || y >= self.height {
            return;
        }
        let px = self.encode(c);
        // SAFETY: x < width and y < height, so the 4-byte write lies inside the
        // `pitch * height` region guaranteed by `new`.
        unsafe { (self.base.add(y * self.pitch + x * 4) as *mut u32).write_volatile(px) };
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, c: Rgb) {
        for yy in y..(y + h).min(self.height) {
            for xx in x..(x + w).min(self.width) {
                self.put(xx, yy, c);
            }
        }
    }

    /// Vertical gradient over the whole screen.
    pub fn gradient(&mut self, top: Rgb, bottom: Rgb) {
        for y in 0..self.height {
            let c = top.mix(bottom, (y * 256 / self.height.max(1)) as u32);
            self.fill_rect(0, y, self.width, 1, c);
        }
    }

    pub fn hline(&mut self, x: usize, y: usize, w: usize, c: Rgb) {
        self.fill_rect(x, y, w, 1, c);
    }

    pub fn vline(&mut self, x: usize, y: usize, h: usize, c: Rgb) {
        self.fill_rect(x, y, 1, h, c);
    }

    /// Hairline that fades out towards both ends (the Keep's divider style).
    pub fn fade_line(&mut self, x: usize, y: usize, w: usize, c: Rgb, bg: Rgb) {
        for i in 0..w {
            let d = (i as i64 - w as i64 / 2).unsigned_abs() as u32;
            let t = (d * 256 / (w as u32 / 2).max(1)).min(256);
            self.put(x + i, y, c.mix(bg, t));
        }
    }

    /// Small filled diamond centred on (cx, cy).
    pub fn diamond(&mut self, cx: usize, cy: usize, r: usize, c: Rgb) {
        for dy in 0..=r {
            let half = r - dy;
            self.hline(cx - half, cy - dy, half * 2 + 1, c);
            self.hline(cx - half, cy + dy, half * 2 + 1, c);
        }
    }

    pub fn draw_char(&mut self, x: usize, y: usize, ch: char, scale: usize, c: Rgb) {
        let idx = if (ch as u32) < 128 { ch as usize } else { b'?' as usize };
        for (row, bits) in BASIC_LEGACY[idx].iter().enumerate() {
            for col in 0..8 {
                if bits & (1 << col) != 0 {
                    self.fill_rect(x + col * scale, y + row * scale, scale, scale, c);
                }
            }
        }
    }

    pub fn draw_text(&mut self, x: usize, y: usize, s: &str, scale: usize, c: Rgb) {
        for (i, ch) in s.chars().enumerate() {
            self.draw_char(x + i * 8 * scale, y, ch, scale, c);
        }
    }

    pub fn text_width(s: &str, scale: usize) -> usize {
        s.chars().count() * 8 * scale
    }

    pub fn draw_text_centered(&mut self, y: usize, s: &str, scale: usize, c: Rgb) {
        let w = Self::text_width(s, scale);
        self.draw_text(self.width.saturating_sub(w) / 2, y, s, scale, c);
    }
}
