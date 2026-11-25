//! Local APIC: the per-core interrupt controller and its built-in timer.
//!
//! Supports both access modes: xAPIC (memory-mapped registers) and x2APIC (MSRs), because
//! real laptop firmware sometimes hands over in x2APIC mode.

use super::idt::{SPURIOUS_VECTOR, TIMER_VECTOR};
use super::io::{inb, outb};
use crate::mm;
use spin::Once;
use x86_64::registers::model_specific::Msr;

const IA32_APIC_BASE: u32 = 0x1B;
const APIC_BASE_ENABLE: u64 = 1 << 11;
const APIC_BASE_X2APIC: u64 = 1 << 10;

// register offsets (xAPIC MMIO); x2APIC MSR = 0x800 + offset / 16
const REG_ID: u32 = 0x20;
const REG_TPR: u32 = 0x80;
const REG_EOI: u32 = 0xB0;
const REG_SVR: u32 = 0xF0;
const REG_LVT_TIMER: u32 = 0x320;
const REG_TIMER_INIT: u32 = 0x380;
const REG_TIMER_CURRENT: u32 = 0x390;
const REG_TIMER_DIVIDE: u32 = 0x3E0;

const LVT_MASKED: u32 = 1 << 16;
const LVT_PERIODIC: u32 = 1 << 17;

enum Mode {
    XApic { base: u64 },
    X2Apic,
}

struct Apic {
    mode: Mode,
}

static APIC: Once<Apic> = Once::new();

impl Apic {
    fn read(&self, reg: u32) -> u32 {
        match self.mode {
            // SAFETY: `base` is the LAPIC register page mapped uncached by `init`; `reg` is a
            // valid register offset, and LAPIC registers are 32-bit aligned.
            Mode::XApic { base } => unsafe { ((base + reg as u64) as *const u32).read_volatile() },
            // SAFETY: x2APIC mode is enabled, so MSR 0x800 + reg/16 is the same register.
            Mode::X2Apic => unsafe { Msr::new(0x800 + (reg >> 4)).read() as u32 },
        }
    }

    fn write(&self, reg: u32, value: u32) {
        match self.mode {
            // SAFETY: as in `read`.
            Mode::XApic { base } => unsafe { ((base + reg as u64) as *mut u32).write_volatile(value) },
            // SAFETY: as in `read`.
            Mode::X2Apic => unsafe { Msr::new(0x800 + (reg >> 4)).write(value as u64) },
        }
    }
}

pub struct ApicInfo {
    pub x2apic: bool,
    pub phys_base: u64,
    pub id: u32,
}

/// Enable this core's Local APIC with all its local interrupts masked.
pub fn init() -> ApicInfo {
    let mut msr = Msr::new(IA32_APIC_BASE);
    // SAFETY: IA32_APIC_BASE exists on every x86-64 CPU; reading it has no side effects.
    let mut value = unsafe { msr.read() };
    if value & APIC_BASE_ENABLE == 0 {
        value |= APIC_BASE_ENABLE;
        // SAFETY: setting the global-enable bit only turns on this core's APIC.
        unsafe { msr.write(value) };
    }
    let phys_base = value & 0x000F_FFFF_FFFF_F000;
    let x2apic = value & APIC_BASE_X2APIC != 0;

    let mode = if x2apic {
        Mode::X2Apic
    } else {
        Mode::XApic { base: mm::vmm::map_mmio(phys_base, 4096).expect("cannot map the local APIC") }
    };
    let apic = APIC.call_once(|| Apic { mode });

    apic.write(REG_TPR, 0); // accept every priority
    apic.write(REG_LVT_TIMER, LVT_MASKED);
    apic.write(REG_SVR, 0x100 | SPURIOUS_VECTOR as u32); // software-enable + spurious vector
    let id = if x2apic { apic.read(REG_ID) } else { apic.read(REG_ID) >> 24 };
    ApicInfo { x2apic, phys_base, id }
}

/// Tell the APIC the current interrupt is handled.
pub fn eoi() {
    if let Some(a) = APIC.get() {
        a.write(REG_EOI, 0);
    }
}

/// Measure the APIC timer against the PIT (a fixed 1.193182 MHz clock), then start it in
/// periodic mode at `hz`. Returns the measured APIC timer ticks per millisecond.
pub fn start_timer(hz: u32) -> Option<u32> {
    let apic = APIC.get()?;
    apic.write(REG_TIMER_DIVIDE, 0x3); // divide bus clock by 16

    const PIT_HZ: u32 = 1_193_182;
    const CAL_MS: u32 = 10;
    let pit_count = (PIT_HZ * CAL_MS / 1000) as u16;

    let elapsed;
    // SAFETY: ports 0x42/0x43 (PIT channel 2) and 0x61 (gate/speaker control) are legacy
    // timer ports, used only here and only during this one-shot calibration.
    unsafe {
        let ctrl = inb(0x61);
        outb(0x61, (ctrl & !0x02) | 0x01); // gate on, speaker off
        outb(0x43, 0b1011_0000); //            channel 2, lo/hi byte, mode 0 (one-shot)
        outb(0x42, pit_count as u8);
        outb(0x42, (pit_count >> 8) as u8); // counting starts here
        apic.write(REG_TIMER_INIT, u32::MAX);
        let mut guard = 0u64;
        while inb(0x61) & 0x20 == 0 {
            guard += 1;
            if guard > 200_000_000 {
                apic.write(REG_LVT_TIMER, LVT_MASKED);
                return None; // PIT never fired: no way to calibrate
            }
        }
        elapsed = u32::MAX - apic.read(REG_TIMER_CURRENT);
        outb(0x61, inb(0x61) & !0x01); // gate off
    }

    let per_ms = elapsed / CAL_MS;
    let initial = per_ms * 1000 / hz;
    apic.write(REG_LVT_TIMER, TIMER_VECTOR as u32 | LVT_PERIODIC);
    apic.write(REG_TIMER_INIT, initial);
    Some(per_ms)
}
