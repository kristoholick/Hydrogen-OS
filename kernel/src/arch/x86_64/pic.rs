//! The legacy 8259 interrupt controllers. We use the Local APIC instead, so the PICs are
//! remapped away from the CPU-exception vectors (in case a stray IRQ fires) and masked.

use super::io::outb;

const PIC1_CMD: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_CMD: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

fn io_wait() {
    // SAFETY: port 0x80 is the POST diagnostic port; writing it is a harmless ~1 µs delay.
    unsafe { outb(0x80, 0) };
}

pub fn disable() {
    // SAFETY: ports 0x20/0x21/0xA0/0xA1 belong to the 8259 pair, owned by this module.
    // ICW1–ICW4 remap IRQs to vectors 0xE0–0xEF (clear of CPU exceptions and of our own
    // vectors), then OCW1 masks every line.
    unsafe {
        outb(PIC1_CMD, 0x11); io_wait(); // ICW1: init, expect ICW4
        outb(PIC2_CMD, 0x11); io_wait();
        outb(PIC1_DATA, 0xE0); io_wait(); // ICW2: master vectors 0xE0–0xE7
        outb(PIC2_DATA, 0xE8); io_wait(); // ICW2: slave vectors 0xE8–0xEF
        outb(PIC1_DATA, 4); io_wait(); //    ICW3: slave on IRQ2
        outb(PIC2_DATA, 2); io_wait();
        outb(PIC1_DATA, 1); io_wait(); //    ICW4: 8086 mode
        outb(PIC2_DATA, 1); io_wait();
        outb(PIC1_DATA, 0xFF); // mask all
        outb(PIC2_DATA, 0xFF);
    }
}
