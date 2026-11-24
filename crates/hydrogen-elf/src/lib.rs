//! ELF64 program files: check everything, then hand out the segments to load.
//!
//! The input is untrusted. No `unsafe`, no panics, no allocation: every number from the file is
//! range-checked before use, and anything unusual is refused rather than guessed at.
//! Only plain static executables are accepted (no dynamic linking, no thread-local storage).

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

/// Machine numbers (`Limits::machine`). The parser itself works for any 64-bit CPU.
pub const X86_64: u16 = 62;

const PAGE: u64 = 4096;
const HEADER_LEN: usize = 64;
const PH_LEN: usize = 56;
const MAX_HEADERS: usize = 32;

const ET_EXEC: u16 = 2;
const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const PT_INTERP: u32 = 3;
const PT_TLS: u32 = 7;
const PF_X: u32 = 1;
const PF_W: u32 = 2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    /// File shorter than its own headers say.
    Truncated,
    NotElf,
    /// Not 64-bit little-endian, version 1.
    WrongFormat,
    WrongMachine,
    /// Not a static executable.
    NotStatic,
    BadHeader,
    /// Outside the allowed addresses, or larger than allowed in total.
    BadAddress,
    /// Writable and executable at once.
    WriteAndExec,
    /// Segments overlap, share a page or are not in ascending order.
    Overlap,
    /// The entry point is not inside executable file contents.
    BadEntry,
}

/// What the caller is willing to load.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub machine: u16,
    /// Allowed addresses: `start..end`.
    pub start: u64,
    pub end: u64,
    /// Most memory all segments together may take, in bytes (whole pages counted).
    pub max_memory: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Segment<'a> {
    pub addr: u64,
    /// Size in memory; the part beyond `data` is zero.
    pub mem_size: u64,
    pub data: &'a [u8],
    pub write: bool,
    pub exec: bool,
}

impl Segment<'_> {
    /// Page-aligned range the segment occupies.
    pub fn pages(&self) -> core::ops::Range<u64> {
        self.addr & !(PAGE - 1)..(self.addr + self.mem_size).next_multiple_of(PAGE)
    }
}

/// A completely checked program file.
#[derive(Clone, Copy, Debug)]
pub struct Program<'a> {
    pub entry: u64,
    file: &'a [u8],
    ph_offset: usize,
    ph_count: usize,
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

impl<'a> Program<'a> {
    pub fn parse(file: &'a [u8], limits: &Limits) -> Result<Self, Error> {
        if bytes::<4>(file, 0)? != *b"\x7fELF" {
            return Err(Error::NotElf);
        }
        // 64-bit, little-endian, version 1
        if bytes::<3>(file, 4)? != [2, 1, 1] {
            return Err(Error::WrongFormat);
        }
        if u16_at(file, 18)? != limits.machine {
            return Err(Error::WrongMachine);
        }
        if u16_at(file, 16)? != ET_EXEC {
            return Err(Error::NotStatic);
        }
        let ph_count = u16_at(file, 56)? as usize;
        if u32_at(file, 20)? != 1
            || u16_at(file, 52)? as usize != HEADER_LEN
            || u16_at(file, 54)? as usize != PH_LEN
            || !(1..=MAX_HEADERS).contains(&ph_count)
        {
            return Err(Error::BadHeader);
        }
        let ph_offset = usize::try_from(u64_at(file, 32)?).map_err(|_| Error::Truncated)?;
        let ph_end = ph_offset.checked_add(ph_count * PH_LEN).ok_or(Error::Truncated)?;
        if ph_end > file.len() {
            return Err(Error::Truncated);
        }
        let p = Program { entry: u64_at(file, 24)?, file, ph_offset, ph_count };

        let mut next_free = limits.start;
        let mut memory = 0u64;
        let mut entry_ok = false;
        for i in 0..ph_count {
            let Some(s) = p.segment(i)? else { continue };
            let end = s.addr.checked_add(s.mem_size).ok_or(Error::BadAddress)?;
            if s.addr < limits.start || end > limits.end {
                return Err(Error::BadAddress);
            }
            if s.write && s.exec {
                return Err(Error::WriteAndExec);
            }
            let pages = s.addr & !(PAGE - 1)..end.checked_next_multiple_of(PAGE).ok_or(Error::BadAddress)?;
            if pages.start < next_free {
                return Err(Error::Overlap);
            }
            next_free = pages.end;
            memory = memory.checked_add(pages.end - pages.start).ok_or(Error::BadAddress)?;
            if memory > limits.max_memory {
                return Err(Error::BadAddress);
            }
            entry_ok |= s.exec && (s.addr..s.addr + s.data.len() as u64).contains(&p.entry);
        }
        if !entry_ok {
            return Err(Error::BadEntry);
        }
        Ok(p)
    }

    /// The `i`-th program header as a loadable segment; `None` if there is nothing to load.
    fn segment(&self, i: usize) -> Result<Option<Segment<'a>>, Error> {
        let at = self.ph_offset + i * PH_LEN;
        match u32_at(self.file, at)? {
            PT_LOAD => {}
            PT_DYNAMIC | PT_INTERP | PT_TLS => return Err(Error::NotStatic),
            _ => return Ok(None), // notes, stack hints, ...: nothing to load
        }
        let flags = u32_at(self.file, at + 4)?;
        let offset = u64_at(self.file, at + 8)?;
        let addr = u64_at(self.file, at + 16)?;
        let file_size = u64_at(self.file, at + 32)?;
        let mem_size = u64_at(self.file, at + 40)?;
        if file_size > mem_size {
            return Err(Error::BadHeader);
        }
        if mem_size == 0 {
            return Ok(None);
        }
        let end = offset.checked_add(file_size).ok_or(Error::Truncated)?;
        let range = usize::try_from(offset).map_err(|_| Error::Truncated)?
            ..usize::try_from(end).map_err(|_| Error::Truncated)?;
        let data = self.file.get(range).ok_or(Error::Truncated)?;
        Ok(Some(Segment { addr, mem_size, data, write: flags & PF_W != 0, exec: flags & PF_X != 0 }))
    }

    /// The segments to load, in ascending order, never sharing a page.
    pub fn segments(&self) -> impl Iterator<Item = Segment<'a>> + use<'a> {
        let p = *self;
        // `parse` has checked every header, so nothing is skipped because of an error here.
        (0..p.ph_count).filter_map(move |i| p.segment(i).ok().flatten())
    }
}

#[cfg(test)]
mod tests;
