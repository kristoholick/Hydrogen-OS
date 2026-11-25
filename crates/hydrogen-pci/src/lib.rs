//! Finds PCI devices: walks the buses, follows bridges and reads what each device says about
//! itself. How configuration space is reached is the caller's business (`Config`), so the same
//! code runs on the host in tests and on any CPU.
//!
//! Devices are untrusted too: loops of bridges and capability lists are cut off.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

/// Where a device sits. `device` is 0-31, `function` 0-7.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Address {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
}

impl core::fmt::Display for Address {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{:02x}:{:02x}.{}", self.bus, self.device, self.function)
    }
}

/// Access to configuration space. `offset` is a multiple of 4 below 4096.
pub trait Config {
    /// All ones if there is no such device (or it cannot be reached).
    fn read(&self, at: Address, offset: u16) -> u32;
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Device {
    pub at: Address,
    pub vendor: u16,
    pub id: u16,
    pub class: u8,
    pub subclass: u8,
    pub interface: u8,
    pub revision: u8,
    /// Bus behind this device, if it is a bridge to another bus.
    pub bridge_to: Option<u8>,
}

impl Device {
    fn read(c: &impl Config, at: Address) -> Option<Self> {
        let id = c.read(at, 0);
        if id as u16 == 0xffff || id == 0 {
            return None;
        }
        let class = c.read(at, 8);
        let (major, sub) = ((class >> 24) as u8, (class >> 16) as u8);
        let layout = (c.read(at, 0xc) >> 16) as u8 & 0x7f;
        let bridge = major == 6 && sub == 4 && layout == 1;
        Some(Self {
            at,
            vendor: id as u16,
            id: (id >> 16) as u16,
            class: major,
            subclass: sub,
            interface: (class >> 8) as u8,
            revision: class as u8,
            bridge_to: bridge.then(|| (c.read(at, 0x18) >> 8) as u8),
        })
    }

    /// What kind of device this is, in words.
    pub fn kind(&self) -> &'static str {
        match (self.class, self.subclass, self.interface) {
            (1, 1, _) => "IDE disk controller",
            (1, 6, _) => "SATA disk controller",
            (1, 8, 2) => "NVMe disk",
            (1, 0, _) => "SCSI disk controller",
            (1, _, _) => "storage controller",
            (2, 0, _) => "Ethernet controller",
            (2, _, _) => "network controller",
            (3, _, _) => "display controller",
            (4, 1, _) => "audio device",
            (4, 3, _) => "HD audio device",
            (4, _, _) => "multimedia device",
            (5, _, _) => "memory controller",
            (6, 0, _) => "host bridge",
            (6, 1, _) => "ISA bridge",
            (6, 4, _) => "PCI bridge",
            (6, _, _) => "bridge",
            (7, _, _) => "communication controller",
            (8, _, _) => "system device",
            (9, _, _) => "input controller",
            (0xc, 3, 0) => "USB 1.1 controller (UHCI)",
            (0xc, 3, 0x10) => "USB 1.1 controller (OHCI)",
            (0xc, 3, 0x20) => "USB 2 controller (EHCI)",
            (0xc, 3, 0x30) => "USB 3 controller (xHCI)",
            (0xc, 3, _) => "USB controller",
            (0xc, 5, _) => "SMBus controller",
            (0xc, _, _) => "serial bus controller",
            (0xd, _, _) => "wireless controller",
            (0x10, _, _) => "encryption controller",
            (0x13, _, _) => "non-essential device",
            _ => "unknown device",
        }
    }
}

/// Call `found` for every device reachable from `first_bus`, following bridges up to `last_bus`.
/// Every bus is looked at once at most, whatever the bridges claim.
pub fn scan(c: &impl Config, first_bus: u8, last_bus: u8, mut found: impl FnMut(Device)) {
    // Buses still to visit and buses already queued, one bit each: no recursion, no allocation.
    let mut queued = [0u64; 4];
    let mut todo = [0u64; 4];
    let mut queue = |bus: u8, todo: &mut [u64; 4]| {
        let (word, bit) = (bus as usize / 64, 1u64 << (bus % 64));
        if (first_bus..=last_bus).contains(&bus) && queued[word] & bit == 0 {
            queued[word] |= bit;
            todo[word] |= bit;
        }
    };
    queue(first_bus, &mut todo);
    while let Some(word) = todo.iter().position(|w| *w != 0) {
        let bit = todo[word].trailing_zeros();
        todo[word] &= !(1 << bit);
        let bus = (word as u32 * 64 + bit) as u8;
        for device in 0..32 {
            let at = Address { bus, device, function: 0 };
            let Some(first) = Device::read(c, at) else { continue };
            let several = c.read(at, 0xc) >> 23 & 1 != 0;
            let mut one = |d: Device| {
                found(d);
                if let Some(next) = d.bridge_to {
                    queue(next, &mut todo);
                }
            };
            one(first);
            if several {
                for function in 1..8 {
                    if let Some(d) = Device::read(c, Address { function, ..at }) {
                        one(d);
                    }
                }
            }
        }
    }
}

/// Capability ids.
pub const MSI: u8 = 5;
pub const VENDOR: u8 = 9;
pub const MSI_X: u8 = 0x11;

/// The device's capabilities as (id, offset in configuration space).
pub fn capabilities<C: Config>(c: &C, at: Address) -> impl Iterator<Item = (u8, u16)> + '_ {
    let has_list = c.read(at, 4) >> 20 & 1 != 0;
    let mut next = if has_list { c.read(at, 0x34) as u8 & 0xfc } else { 0 };
    // 48 capabilities fill the whole 256 bytes: more means the list loops.
    let mut left = 48;
    core::iter::from_fn(move || {
        if next < 0x40 || left == 0 {
            return None;
        }
        left -= 1;
        let (offset, word) = (next as u16, c.read(at, next as u16));
        next = (word >> 8) as u8 & 0xfc;
        (word as u8 != 0xff).then_some((word as u8, offset))
    })
}

#[cfg(test)]
mod tests;
