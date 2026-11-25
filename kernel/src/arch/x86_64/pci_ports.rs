//! The old x86 way to PCI configuration space: an address port and a data port.

use super::io::{inl, outl};
use crate::sync::IrqMutex;

const ADDRESS: u16 = 0xcf8;
const DATA: u16 = 0xcfc;

static PORTS: IrqMutex<()> = IrqMutex::new(());

/// Read the 32-bit register at `offset` (a multiple of 4).
pub fn read(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    let address = 1 << 31 | (bus as u32) << 16 | (device as u32) << 11 | (function as u32) << 8 | (offset & 0xfc) as u32;
    let _guard = PORTS.lock(); // the two accesses belong together
    // SAFETY: the standard PCI configuration ports, used only here and under the lock.
    unsafe {
        outl(ADDRESS, address);
        inl(DATA)
    }
}
