use super::*;

const LIMITS: Limits = Limits { machine: X86_64, start: 0x1_0000, end: 0x7fff_ffff_f000, max_memory: 1 << 20 };

#[derive(Clone, Copy)]
struct Ph {
    kind: u32,
    flags: u32,
    offset: u64,
    addr: u64,
    file_size: u64,
    mem_size: u64,
}

const CODE: Ph = Ph { kind: 1, flags: 5, offset: 0x200, addr: 0x40_0000, file_size: 0x100, mem_size: 0x100 };
const DATA: Ph = Ph { kind: 1, flags: 6, offset: 0x300, addr: 0x40_1000, file_size: 0x40, mem_size: 0x2000 };

fn build(phs: &[Ph], entry: u64) -> Vec<u8> {
    let mut f = vec![0u8; 0x400];
    for (i, b) in f.iter_mut().enumerate().skip(0x200) {
        *b = i as u8;
    }
    f[..8].copy_from_slice(b"\x7fELF\x02\x01\x01\x00");
    let mut put = |at: usize, b: &[u8]| f[at..at + b.len()].copy_from_slice(b);
    put(16, &2u16.to_le_bytes());
    put(18, &62u16.to_le_bytes());
    put(20, &1u32.to_le_bytes());
    put(24, &entry.to_le_bytes());
    put(32, &64u64.to_le_bytes());
    put(52, &64u16.to_le_bytes());
    put(54, &56u16.to_le_bytes());
    put(56, &(phs.len() as u16).to_le_bytes());
    for (i, p) in phs.iter().enumerate() {
        let at = 64 + i * 56;
        put(at, &p.kind.to_le_bytes());
        put(at + 4, &p.flags.to_le_bytes());
        put(at + 8, &p.offset.to_le_bytes());
        put(at + 16, &p.addr.to_le_bytes());
        put(at + 32, &p.file_size.to_le_bytes());
        put(at + 40, &p.mem_size.to_le_bytes());
    }
    f
}

fn err(phs: &[Ph]) -> Error {
    Program::parse(&build(phs, 0x40_0010), &LIMITS).unwrap_err()
}

/// A good file with some bytes replaced.
fn patched(at: usize, b: &[u8]) -> Result<u64, Error> {
    let mut f = build(&[CODE, DATA], 0x40_0010);
    f[at..at + b.len()].copy_from_slice(b);
    Program::parse(&f, &LIMITS).map(|p| p.entry)
}

#[test]
fn good_file() {
    let f = build(&[CODE, DATA], 0x40_0010);
    let p = Program::parse(&f, &LIMITS).unwrap();
    assert_eq!(p.entry, 0x40_0010);
    let s: Vec<_> = p.segments().collect();
    assert_eq!(s.len(), 2);
    assert_eq!((s[0].addr, s[0].data, s[0].write, s[0].exec), (0x40_0000, &f[0x200..0x300], false, true));
    assert_eq!((s[1].addr, s[1].mem_size, s[1].data.len()), (0x40_1000, 0x2000, 0x40));
    assert!(s[1].write && !s[1].exec);
    assert_eq!(s[1].pages(), 0x40_1000..0x40_3000);
}

#[test]
fn harmless_headers_are_skipped() {
    let note = Ph { kind: 4, ..CODE };
    let stack = Ph { kind: 0x6474_e551, ..CODE };
    let empty = Ph { file_size: 0, mem_size: 0, ..DATA };
    let f = build(&[note, CODE, stack, empty, DATA], 0x40_0010);
    assert_eq!(Program::parse(&f, &LIMITS).unwrap().segments().count(), 2);
}

#[test]
fn wrong_kinds_of_file() {
    assert_eq!(Program::parse(b"", &LIMITS).unwrap_err(), Error::Truncated);
    assert_eq!(Program::parse(b"MZ\x90\0\x03\0\0\0", &LIMITS).unwrap_err(), Error::NotElf);
    assert_eq!(patched(4, &[1]), Err(Error::WrongFormat)); // 32-bit
    assert_eq!(patched(5, &[2]), Err(Error::WrongFormat)); // big-endian
    assert_eq!(patched(6, &[2]), Err(Error::WrongFormat));
    assert_eq!(patched(18, &183u16.to_le_bytes()), Err(Error::WrongMachine)); // ARM
    assert_eq!(patched(16, &3u16.to_le_bytes()), Err(Error::NotStatic)); // position independent
    assert_eq!(patched(20, &2u32.to_le_bytes()), Err(Error::BadHeader));
    assert_eq!(patched(52, &65u16.to_le_bytes()), Err(Error::BadHeader));
    assert_eq!(patched(54, &57u16.to_le_bytes()), Err(Error::BadHeader));
    assert_eq!(patched(56, &0u16.to_le_bytes()), Err(Error::BadHeader));
    assert_eq!(patched(56, &33u16.to_le_bytes()), Err(Error::BadHeader));
}

#[test]
fn dynamic_programs_refused() {
    for kind in [2, 3, 7] {
        assert_eq!(err(&[CODE, Ph { kind, ..DATA }]), Error::NotStatic);
    }
}

#[test]
fn truncated_everywhere() {
    let f = build(&[CODE, DATA], 0x40_0010);
    for n in 0..0x340 {
        assert!(Program::parse(&f[..n], &LIMITS).is_err(), "accepted the first {n} bytes");
    }
    assert!(Program::parse(&f[..0x340], &LIMITS).is_ok());
}

#[test]
fn header_table_out_of_range() {
    for off in [0x400u64, 0x3ff, u64::MAX, u64::MAX - 56, 1 << 63] {
        assert_eq!(patched(32, &off.to_le_bytes()), Err(Error::Truncated), "{off:#x}");
    }
}

#[test]
fn segment_contents_out_of_range() {
    for offset in [0x400u64, 0x301, u64::MAX, u64::MAX - 0x100] {
        assert_eq!(err(&[Ph { offset, ..CODE }]), Error::Truncated, "{offset:#x}");
    }
    assert!(Program::parse(&build(&[Ph { offset: 0x300, ..CODE }], 0x40_0010), &LIMITS).is_ok());
    assert_eq!(err(&[Ph { file_size: 0x101, ..CODE }]), Error::BadHeader); // larger than memory
    assert_eq!(err(&[Ph { file_size: u64::MAX, mem_size: u64::MAX, ..CODE }]), Error::Truncated);
}

#[test]
fn addresses_out_of_range() {
    for addr in [0, 0xffff, 0x7fff_ffff_f000, 0x7fff_ffff_ef01, 0xffff_8000_0000_0000, u64::MAX, u64::MAX - 0xff] {
        assert_eq!(err(&[Ph { addr, ..CODE }]), Error::BadAddress, "{addr:#x}");
    }
    assert_eq!(err(&[CODE, Ph { mem_size: u64::MAX, ..DATA }]), Error::BadAddress);
    assert_eq!(err(&[CODE, Ph { mem_size: 1 << 20, ..DATA }]), Error::BadAddress); // too much memory
    let last = Ph { addr: 0x7fff_ffff_ef00, ..CODE };
    assert!(Program::parse(&build(&[last], 0x7fff_ffff_ef10), &LIMITS).is_ok());
}

#[test]
fn write_and_exec_refused() {
    assert_eq!(err(&[Ph { flags: 7, ..CODE }]), Error::WriteAndExec);
    assert_eq!(err(&[CODE, Ph { flags: 3, ..DATA }]), Error::WriteAndExec);
}

#[test]
fn overlapping_segments_refused() {
    assert_eq!(err(&[CODE, CODE]), Error::Overlap);
    assert_eq!(err(&[CODE, Ph { addr: 0x40_0800, ..DATA }]), Error::Overlap); // same page
    assert_eq!(err(&[DATA, CODE]), Error::Overlap); // not ascending
}

#[test]
fn bad_entry_points() {
    for entry in [0, 0x3f_ffff, 0x40_0100, 0x40_1000, u64::MAX] {
        assert_eq!(Program::parse(&build(&[CODE, DATA], entry), &LIMITS).unwrap_err(), Error::BadEntry, "{entry:#x}");
    }
    assert_eq!(err(&[Ph { file_size: 0, ..CODE }]), Error::BadEntry); // zero-filled memory is not code
}

/// Random damage must never panic, and whatever is still accepted must obey the rules.
#[test]
fn random_damage() {
    let good = build(&[CODE, DATA, Ph { addr: 0x40_5000, ..DATA }], 0x40_0010);
    let mut x = 0x9e37_79b9_7f4a_7c15u64;
    let mut rnd = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    let mut accepted = 0;
    for _ in 0..300_000 {
        let mut f = good.clone();
        for _ in 0..1 + rnd() % 6 {
            let at = (rnd() % 232) as usize; // the headers
            f[at] = if rnd() % 3 == 0 { [0, 0x7f, 0x80, 0xff][(rnd() % 4) as usize] } else { rnd() as u8 };
        }
        f.truncate(if rnd() % 8 == 0 { (rnd() % 0x400) as usize } else { 0x400 });
        let Ok(p) = Program::parse(&f, &LIMITS) else { continue };
        accepted += 1;
        let (mut last, mut total, mut entry_ok) = (0, 0, false);
        for s in p.segments() {
            let r = s.pages();
            assert!(s.addr >= LIMITS.start && s.addr + s.mem_size <= LIMITS.end);
            assert!(r.start >= last && s.data.len() as u64 <= s.mem_size && !(s.write && s.exec));
            last = r.end;
            total += r.end - r.start;
            entry_ok |= s.exec && p.entry >= s.addr && p.entry < s.addr + s.data.len() as u64;
        }
        assert!(total <= LIMITS.max_memory && entry_ok);
    }
    assert!(accepted > 1000, "only {accepted} accepted: test proves little");
}
