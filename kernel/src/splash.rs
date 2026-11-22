//! Phase 1 boot screen and crash screen, drawn in the Keep's palette.
//! Deliberately simple: an 8x8 bitmap font and flat shapes. The real UI arrives in Phase 6.

use crate::drivers::framebuffer::{Framebuffer, Rgb};

pub const VOID: Rgb = Rgb::hex(0x070709);
pub const SMOKE: Rgb = Rgb::hex(0x0C0B0E);
pub const GOLD: Rgb = Rgb::hex(0xC8A96E);
pub const GOLD_DIM: Rgb = Rgb::hex(0x6E5B3A);
pub const CRIMSON: Rgb = Rgb::hex(0x8F1D21);
pub const IVORY: Rgb = Rgb::hex(0xE6DEC8);
pub const ASH: Rgb = Rgb::hex(0x8A8478);

/// Double gold frame with diamond-tipped corners.
fn frame(fb: &mut Framebuffer, m: usize, outer: Rgb, inner: Rgb) {
    let (w, h) = (fb.width, fb.height);
    fb.hline(m, m, w - 2 * m, outer);
    fb.hline(m, h - m - 1, w - 2 * m, outer);
    fb.vline(m, m, h - 2 * m, outer);
    fb.vline(w - m - 1, m, h - 2 * m, outer);
    let i = m + 6;
    fb.hline(i, i, w - 2 * i, inner);
    fb.hline(i, h - i - 1, w - 2 * i, inner);
    fb.vline(i, i, h - 2 * i, inner);
    fb.vline(w - i - 1, i, h - 2 * i, inner);
    // corner ornaments
    let arm = 28;
    for &(cx, cy, dx, dy) in &[(m, m, 1i64, 1i64), (w - m - 1, m, -1, 1), (m, h - m - 1, 1, -1), (w - m - 1, h - m - 1, -1, -1)] {
        for k in 0..arm {
            let x = (cx as i64 + dx * k as i64) as usize;
            let y = (cy as i64 + dy * k as i64) as usize;
            fb.put(x, cy.wrapping_add_signed(-dy as isize * 3), GOLD);
            fb.put(cx.wrapping_add_signed(-dx as isize * 3), y, GOLD);
        }
        fb.diamond(cx.wrapping_add_signed(dx as isize * 10), cy.wrapping_add_signed(dy as isize * 10), 3, GOLD);
    }
}

fn divider(fb: &mut Framebuffer, y: usize, width: usize, bg: Rgb) {
    let x = fb.width.saturating_sub(width) / 2;
    fb.fade_line(x, y, width, GOLD, bg);
    fb.diamond(fb.width / 2, y, 4, GOLD);
    fb.diamond(fb.width / 2, y, 2, bg);
}

pub struct BootFacts<'a> {
    pub bootloader: &'a str,
    pub display: &'a str,
    pub memory: &'a str,
    pub protection: &'a str,
    pub timer: &'a str,
}

pub fn boot_screen(fb: &mut Framebuffer, facts: &BootFacts) {
    let (w, h) = (fb.width, fb.height);
    fb.gradient(SMOKE, VOID);
    let m = (h / 14).max(16);
    frame(fb, m, GOLD_DIM, Rgb::hex(0x2A241A));

    let big = (h / 110).clamp(3, 10);
    let title_y = h * 26 / 100;
    fb.draw_text_centered(title_y, "SIGIL", big, IVORY);
    let sub_y = title_y + 8 * big + 18;
    fb.draw_text_centered(sub_y, "THE KEEP STANDS", 2, GOLD);
    divider(fb, sub_y + 40, (w / 3).max(200), VOID);

    let rows = [
        ("bootloader", facts.bootloader),
        ("display", facts.display),
        ("memory", facts.memory),
        ("protection", facts.protection),
        ("timer", facts.timer),
    ];
    let line_h = 30;
    let label_w = Framebuffer::text_width("protection  ", 2);
    let block_w = label_w + Framebuffer::text_width("466 MiB free of 512 MiB", 2);
    let x0 = w.saturating_sub(block_w) / 2;
    let mut y = sub_y + 80;
    for (label, value) in rows {
        fb.draw_text(x0, y, label, 2, ASH);
        fb.draw_text(x0 + label_w, y, value, 2, IVORY);
        y += line_h;
    }

    fb.draw_text_centered(h - m - 46, "phase 2 - kernel foundations - idle", 2, ASH);
}

pub fn panic_screen(fb: &mut Framebuffer, location: &str, message: &str) {
    let (w, h) = (fb.width, fb.height);
    let deep = Rgb::hex(0x2A0709);
    fb.gradient(CRIMSON, deep);
    let m = (h / 14).max(16);
    frame(fb, m, IVORY.mix(CRIMSON, 128), Rgb::hex(0x5A1216));

    let big = (h / 160).clamp(3, 7);
    let y0 = h * 24 / 100;
    fb.draw_text_centered(y0, "KERNEL PANIC", big, IVORY);
    let dy = y0 + 8 * big + 26;
    let x = w.saturating_sub((w / 3).max(200)) / 2;
    fb.fade_line(x, dy, (w / 3).max(200), IVORY, deep);

    fb.draw_text_centered(dy + 30, location, 2, GOLD);

    // wrap the message inside the frame
    let max_chars = (w - 2 * m - 80) / 16;
    let mut y = dy + 80;
    let bytes = message.as_bytes();
    let mut start = 0;
    while start < bytes.len() && y < h - m - 90 {
        let end = (start + max_chars).min(bytes.len());
        let line = core::str::from_utf8(&bytes[start..end]).unwrap_or("?");
        fb.draw_text_centered(y, line, 2, IVORY);
        start = end;
        y += 26;
    }

    fb.draw_text_centered(h - m - 46, "the machine has been halted safely - see serial log", 2, IVORY.mix(deep, 90));
}
