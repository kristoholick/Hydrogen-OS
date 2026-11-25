//! Reads the ACPI tables the firmware leaves in memory: which tables exist, where PCI Express
//! configuration space is (MCFG) and which CPUs and interrupt controllers there are (MADT).
//!
//! The bytes are untrusted (firmware has bugs): everything is length- and checksum-checked, no
//! `unsafe`, no panics, no allocation. Getting at the memory is the kernel's job.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

/// Every table starts with a header of this size.
pub const HEADER_LEN: usize = 36;
/// No sane table is larger; protects against reading gigabytes because of a broken length.
pub const MAX_TABLE: usize = 1 << 20;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    Truncated,
    BadSignature,
    BadChecksum,
    BadLength,
}

fn bytes<const N: usize>(b: &[u8], at: usize) -> Result<[u8; N], Error> {
    let end = at.checked_add(N).ok_or(Error::Truncated)?;
    b.get(at..end).and_then(|s| s.try_into().ok()).ok_or(Error::Truncated)
}
fn u16_at(b: &[u8], at: usize) -> Result<u16, Error> {
    bytes(b, at).map(u16::from_le_bytes)
}
fn u32_at(b: &[u8], at: usize) -> Result<u32, Error> {
    bytes(b, at).map(u32::from_le_bytes)
}
fn u64_at(b: &[u8], at: usize) -> Result<u64, Error> {
    bytes(b, at).map(u64::from_le_bytes)
}

fn sums_to_zero(b: &[u8]) -> bool {
    b.iter().fold(0u8, |a, &x| a.wrapping_add(x)) == 0
}

/// Physical address of the list of tables.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Root {
    /// 32-bit entries.
    Rsdt(u64),
    /// 64-bit entries.
    Xsdt(u64),
}

/// Parse the root pointer. `b` should be 36 bytes; 20 are enough for the old ACPI 1.0 form.
pub fn parse_rsdp(b: &[u8]) -> Result<Root, Error> {
    if bytes::<8>(b, 0)? != *b"RSD PTR " {
        return Err(Error::BadSignature);
    }
    if !sums_to_zero(b.get(..20).ok_or(Error::Truncated)?) {
        return Err(Error::BadChecksum);
    }
    let rsdt = u32_at(b, 16)? as u64;
    if b[15] < 2 {
        return Ok(Root::Rsdt(rsdt));
    }
    if u32_at(b, 20)? != HEADER_LEN as u32 {
        return Err(Error::BadLength);
    }
    if !sums_to_zero(b.get(..HEADER_LEN).ok_or(Error::Truncated)?) {
        return Err(Error::BadChecksum);
    }
    match u64_at(b, 24)? {
        0 => Ok(Root::Rsdt(rsdt)),
        x => Ok(Root::Xsdt(x)),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Header {
    pub signature: [u8; 4],
    /// Whole table, header included: between `HEADER_LEN` and `MAX_TABLE`.
    pub length: usize,
}

/// Read a table's header (first `HEADER_LEN` bytes).
pub fn header(b: &[u8]) -> Result<Header, Error> {
    let length = u32_at(b, 4)? as usize;
    if !(HEADER_LEN..=MAX_TABLE).contains(&length) {
        return Err(Error::BadLength);
    }
    Ok(Header { signature: bytes(b, 0)?, length })
}

/// Check a complete table: signature, length, checksum. Returns exactly its bytes.
pub fn table<'a>(b: &'a [u8], signature: &[u8; 4]) -> Result<&'a [u8], Error> {
    let h = header(b)?;
    if h.signature != *signature {
        return Err(Error::BadSignature);
    }
    let t = b.get(..h.length).ok_or(Error::Truncated)?;
    if !sums_to_zero(t) {
        return Err(Error::BadChecksum);
    }
    Ok(t)
}

/// Addresses of all tables listed in a checked RSDT (`wide` = false) or XSDT (`wide` = true).
pub fn entries(root_table: &[u8], wide: bool) -> impl Iterator<Item = u64> + '_ {
    let size = if wide { 8 } else { 4 };
    root_table.get(HEADER_LEN..).unwrap_or(&[]).chunks_exact(size).map(move |c| {
        let mut v = [0; 8];
        v[..size].copy_from_slice(c);
        u64::from_le_bytes(v)
    })
}

/// One stretch of PCI Express configuration space: 1 MiB per bus.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ecam {
    /// Physical address of bus 0's configuration space (even if `first_bus` is higher).
    pub base: u64,
    pub segment: u16,
    pub first_bus: u8,
    pub last_bus: u8,
}

/// Entries of a checked MCFG table; impossible ones are left out.
pub fn mcfg(t: &[u8]) -> impl Iterator<Item = Ecam> + '_ {
    t.get(HEADER_LEN + 8..).unwrap_or(&[]).chunks_exact(16).filter_map(|c| {
        let e = Ecam { base: u64_at(c, 0).ok()?, segment: u16_at(c, 8).ok()?, first_bus: c[10], last_bus: c[11] };
        let end = e.base.checked_add(256 << 20)?;
        (e.base != 0 && e.base % (1 << 20) == 0 && end <= 1 << 52 && e.first_bus <= e.last_bus).then_some(e)
    })
}

/// What the MADT table says about the machine.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Interrupts {
    /// Logical CPUs the firmware lists as usable.
    pub cpus: usize,
    pub io_apics: usize,
}

/// Summarise a checked MADT ("APIC") table. Stops at the first malformed entry.
pub fn madt(t: &[u8]) -> Interrupts {
    let mut r = Interrupts::default();
    let mut rest = t.get(HEADER_LEN + 8..).unwrap_or(&[]);
    while let [kind, len, ..] = *rest {
        let Some(e) = rest.get(..len as usize).filter(|e| e.len() >= 2) else { break };
        match kind {
            // bit 0: usable now
            0 if u32_at(e, 4).is_ok_and(|f| f & 1 != 0) => r.cpus += 1,
            9 if u32_at(e, 8).is_ok_and(|f| f & 1 != 0) => r.cpus += 1,
            1 if e.len() >= 12 => r.io_apics += 1,
            _ => {}
        }
        rest = &rest[len as usize..];
    }
    r
}

#[cfg(test)]
mod tests;
