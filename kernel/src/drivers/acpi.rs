//! Finds the firmware's ACPI tables. Parsing (and distrust) lives in `hydrogen_acpi`; this file
//! only decides which physical memory may be looked at.

use crate::boot;
use crate::kprintln;
use crate::mm::vmm;
use alloc::vec::Vec;
use hydrogen_acpi::{self as acpi, Ecam, HEADER_LEN, Interrupts, Root};

#[derive(Default)]
pub struct Facts {
    pub ecam: Vec<Ecam>,
    pub interrupts: Interrupts,
}

/// Copy `len` bytes of firmware memory. `None` if they are not inside one entry of the memory
/// map: a broken table must not make us read device registers.
fn read(phys: u64, len: usize) -> Option<Vec<u8>> {
    let end = phys.checked_add(len as u64)?;
    boot::memory_map().iter().find(|e| {
        e.type_ != limine::memmap::MEMMAP_BAD_MEMORY
            && e.type_ != limine::memmap::MEMMAP_FRAMEBUFFER
            && e.base <= phys
            && e.base.checked_add(e.length).is_some_and(|e_end| end <= e_end)
    })?;
    let virt = vmm::map_firmware(phys, len as u64)?;
    // SAFETY: `len` bytes at `virt` were just mapped readable; they are ordinary memory listed in
    // the memory map, which nothing else writes while we copy.
    Some(unsafe { core::slice::from_raw_parts(virt as *const u8, len) }.to_vec())
}

fn table(phys: u64, signature: &[u8; 4]) -> Option<Vec<u8>> {
    let h = acpi::header(&read(phys, HEADER_LEN)?).ok().filter(|h| h.signature == *signature)?;
    let t = read(phys, h.length)?;
    match acpi::table(&t, signature) {
        Ok(_) => Some(t),
        Err(e) => {
            kprintln!("[hydrogen] WARNING: acpi table {} at {:#x} ignored: {:?}", name(signature), phys, e);
            None
        }
    }
}

fn name(signature: &[u8; 4]) -> &str {
    core::str::from_utf8(signature).unwrap_or("????")
}

/// Read what the kernel needs. Missing or broken tables give empty facts, never a failure.
pub fn init() -> Facts {
    let mut facts = Facts::default();
    let Some(rsdp) = boot::rsdp_address() else {
        kprintln!("[hydrogen] WARNING: no ACPI tables from the firmware");
        return facts;
    };
    let root = read(rsdp, 36).or_else(|| read(rsdp, 20)).map(|b| acpi::parse_rsdp(&b));
    let (list, wide) = match root {
        Some(Ok(Root::Xsdt(a))) => (table(a, b"XSDT"), true),
        Some(Ok(Root::Rsdt(a))) => (table(a, b"RSDT"), false),
        other => {
            kprintln!("[hydrogen] WARNING: ACPI root pointer at {:#x} unusable: {:?}", rsdp, other);
            return facts;
        }
    };
    let Some(list) = list else {
        kprintln!("[hydrogen] WARNING: ACPI table list unusable");
        return facts;
    };
    let mut count = 0;
    for address in acpi::entries(&list, wide) {
        let Some(h) = read(address, HEADER_LEN).and_then(|b| acpi::header(&b).ok()) else { continue };
        count += 1;
        match &h.signature {
            b"MCFG" => facts.ecam.extend(table(address, b"MCFG").iter().flat_map(|t| acpi::mcfg(t).collect::<Vec<_>>())),
            b"APIC" => facts.interrupts = table(address, b"APIC").map(|t| acpi::madt(&t)).unwrap_or_default(),
            _ => {}
        }
    }
    kprintln!("[hydrogen] acpi: {} tables; {} CPUs, {} I/O interrupt controllers, {} PCIe segments",
        count, facts.interrupts.cpus, facts.interrupts.io_apics, facts.ecam.len());
    facts
}
