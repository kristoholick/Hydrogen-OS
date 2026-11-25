//! Reaches PCI configuration space and keeps the list of devices. The walk itself is in
//! `hydrogen_pci`.
//!
//! Normally through PCI Express (memory-mapped, one MiB per bus, mapped when first used); if the
//! firmware does not say where that is, through the old port mechanism of the CPU.

use super::acpi::Facts;
use crate::arch::x86_64::pci_ports;
use crate::kprintln;
use crate::mm::vmm;
use crate::sync::IrqMutex;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use hydrogen_acpi::Ecam;
use hydrogen_pci::{self as pci, Address, Config, Device};

const BUS_SIZE: u64 = 1 << 20;

enum Space {
    /// With the virtual address of each bus, 0 = not mapped yet.
    Express(Ecam, [AtomicU64; 256]),
    Ports,
}

impl Space {
    fn bus(&self, bus: u8) -> Option<u64> {
        let Space::Express(e, mapped) = self else { return None };
        if !(e.first_bus..=e.last_bus).contains(&bus) {
            return None;
        }
        let slot = &mapped[bus as usize];
        match slot.load(Ordering::Relaxed) {
            0 => {
                let v = vmm::map_mmio(e.base + bus as u64 * BUS_SIZE, BUS_SIZE)?;
                slot.store(v, Ordering::Relaxed);
                Some(v)
            }
            v => Some(v),
        }
    }
}

impl Config for Space {
    fn read(&self, at: Address, offset: u16) -> u32 {
        debug_assert!(at.device < 32 && at.function < 8 && offset < 4096 && offset % 4 == 0);
        match self {
            Space::Express(..) => {
                let Some(bus) = self.bus(at.bus) else { return u32::MAX };
                let register = bus + ((at.device as u64) << 15 | (at.function as u64) << 12 | offset as u64);
                // SAFETY: inside the configuration space of `at.bus`, which is mapped uncached.
                // Reading configuration registers has no side effects that matter here; devices
                // that do not exist read as all ones.
                unsafe { (register as *const u32).read_volatile() }
            }
            Space::Ports if offset < 256 => pci_ports::read(at.bus, at.device, at.function, offset as u8),
            Space::Ports => u32::MAX,
        }
    }
}

static DEVICES: IrqMutex<Vec<Device>> = IrqMutex::new(Vec::new());

/// Find all devices and log them.
pub fn init(facts: &Facts) {
    let space = match facts.ecam.iter().find(|e| e.segment == 0) {
        Some(e) => Space::Express(*e, [const { AtomicU64::new(0) }; 256]),
        None => Space::Ports,
    };
    let (first, last) = match &space {
        Space::Express(e, _) => {
            kprintln!("[hydrogen] pci: express configuration space at phys {:#x}, buses {}-{}", e.base, e.first_bus, e.last_bus);
            (e.first_bus, e.last_bus)
        }
        Space::Ports => {
            kprintln!("[hydrogen] pci: no express configuration space, using ports");
            (0, 255)
        }
    };
    let mut list = Vec::new();
    pci::scan(&space, first, last, |d| list.push(d));
    list.sort_by_key(|d| d.at);
    for d in &list {
        let msix = pci::capabilities(&space, d.at).any(|(id, _)| id == pci::MSI_X);
        kprintln!("[hydrogen] pci {} {:04x}:{:04x} {}{}", d.at, d.vendor, d.id, d.kind(), if msix { ", MSI-X" } else { "" });
    }
    kprintln!("[hydrogen] pci: {} devices", list.len());
    *DEVICES.lock() = list;
}

#[allow(dead_code)] // drivers pick their devices from here, starting with step 4.3
pub fn devices() -> Vec<Device> {
    DEVICES.lock().clone()
}
