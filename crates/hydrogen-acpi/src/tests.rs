use super::*;

fn fix_sum(b: &mut [u8], at: usize) {
    b[at] = 0;
    let s = b.iter().fold(0u8, |a, &x| a.wrapping_add(x));
    b[at] = 0u8.wrapping_sub(s);
}

fn rsdp(revision: u8, rsdt: u32, xsdt: u64) -> Vec<u8> {
    let mut b = vec![0u8; 36];
    b[..8].copy_from_slice(b"RSD PTR ");
    b[15] = revision;
    b[16..20].copy_from_slice(&rsdt.to_le_bytes());
    b[20..24].copy_from_slice(&36u32.to_le_bytes());
    b[24..32].copy_from_slice(&xsdt.to_le_bytes());
    fix_sum(&mut b[..20], 8);
    fix_sum(&mut b, 32);
    b
}

fn make(sig: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut b = vec![0u8; 36];
    b[..4].copy_from_slice(sig);
    b[4..8].copy_from_slice(&(36 + body.len() as u32).to_le_bytes());
    b.extend_from_slice(body);
    fix_sum(&mut b, 9);
    b
}

#[test]
fn root_pointer() {
    assert_eq!(parse_rsdp(&rsdp(2, 0x1000, 0x2000)), Ok(Root::Xsdt(0x2000)));
    assert_eq!(parse_rsdp(&rsdp(2, 0x1000, 0)), Ok(Root::Rsdt(0x1000)));
    assert_eq!(parse_rsdp(&rsdp(0, 0x1000, 0x2000)), Ok(Root::Rsdt(0x1000)));
    assert_eq!(parse_rsdp(&rsdp(0, 0x1000, 0)[..20]), Ok(Root::Rsdt(0x1000)));
}

#[test]
fn bad_root_pointers() {
    let good = rsdp(2, 0x1000, 0x2000);
    for n in 0..36 {
        assert!(parse_rsdp(&good[..n]).is_err(), "accepted {n} bytes");
    }
    for at in 0..36 {
        let mut b = good.clone();
        b[at] ^= 0x10;
        assert!(parse_rsdp(&b).is_err(), "accepted damage at {at}");
    }
    let old = rsdp(0, 0x1000, 0);
    for at in 0..20 {
        let mut b = old.clone();
        b[at] ^= 0x10;
        assert!(parse_rsdp(&b[..20]).is_err(), "accepted damage at {at} (ACPI 1.0)");
    }
    let mut b = good.clone();
    b[20] = 40;
    fix_sum(&mut b, 32);
    assert_eq!(parse_rsdp(&b), Err(Error::BadLength));
}

#[test]
fn tables() {
    let t = make(b"FACP", &[1, 2, 3]);
    assert_eq!(header(&t), Ok(Header { signature: *b"FACP", length: 39 }));
    assert_eq!(table(&t, b"FACP"), Ok(&t[..]));
    assert_eq!(table(&t, b"MCFG"), Err(Error::BadSignature));
    assert_eq!(table(&t[..38], b"FACP"), Err(Error::Truncated));
    let mut long = t.clone();
    long.extend_from_slice(&[9; 50]); // whatever follows in memory is not part of the table
    assert_eq!(table(&long, b"FACP"), Ok(&t[..]));
    for at in 8..39 {
        let mut b = t.clone();
        b[at] ^= 1;
        assert_eq!(table(&b, b"FACP"), Err(Error::BadChecksum), "damage at {at}");
    }
    for bad in [0u32, 35, MAX_TABLE as u32 + 1, u32::MAX] {
        let mut b = t.clone();
        b[4..8].copy_from_slice(&bad.to_le_bytes());
        assert_eq!(header(&b), Err(Error::BadLength), "{bad}");
    }
}

#[test]
fn table_lists() {
    let mut body = Vec::new();
    for a in [0x1111_0000u64, 0x2_2222_0000, 0x3333_0000] {
        body.extend_from_slice(&a.to_le_bytes());
    }
    let x = make(b"XSDT", &body);
    assert_eq!(entries(&x, true).collect::<Vec<_>>(), [0x1111_0000, 0x2_2222_0000, 0x3333_0000]);
    let r = make(b"RSDT", &[0x78, 0x56, 0x34, 0x12, 1, 0, 0, 0, 0xff]); // last byte: half an entry
    assert_eq!(entries(&r, false).collect::<Vec<_>>(), [0x1234_5678, 1]);
    assert_eq!(entries(&[], true).count(), 0);
}

fn ecam(base: u64, seg: u16, first: u8, last: u8) -> Vec<u8> {
    let mut e = base.to_le_bytes().to_vec();
    e.extend_from_slice(&seg.to_le_bytes());
    e.extend_from_slice(&[first, last, 0, 0, 0, 0]);
    e
}

#[test]
fn pcie_configuration_space() {
    let mut body = vec![0; 8];
    body.extend(ecam(0xe000_0000, 0, 0, 255));
    body.extend(ecam(0, 0, 0, 255)); // no address
    body.extend(ecam(0xe000_1000, 0, 0, 255)); // not aligned
    body.extend(ecam(0xf000_0000, 1, 9, 8)); // buses the wrong way round
    body.extend(ecam(u64::MAX << 20, 0, 0, 255)); // wraps around
    body.extend(ecam(1 << 52, 0, 0, 0)); // beyond any physical address
    body.extend(ecam(0x1_0000_0000, 2, 64, 127));
    body.extend([1, 2, 3]); // half an entry
    let t = make(b"MCFG", &body);
    assert_eq!(
        mcfg(&t).collect::<Vec<_>>(),
        [
            Ecam { base: 0xe000_0000, segment: 0, first_bus: 0, last_bus: 255 },
            Ecam { base: 0x1_0000_0000, segment: 2, first_bus: 64, last_bus: 127 },
        ]
    );
    assert_eq!(mcfg(&t[..40]).count(), 0);
}

#[test]
fn cpus_and_interrupt_controllers() {
    let mut body = vec![0; 8];
    body.extend([0, 8, 0, 0, 1, 0, 0, 0]); // CPU, usable
    body.extend([0, 8, 1, 1, 0, 0, 0, 0]); // CPU, switched off
    body.extend([1, 12, 0, 0, 0, 0, 0xc0, 0xfe, 0, 0, 0, 0]); // I/O APIC
    body.extend([9, 16, 0, 0, 5, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]); // x2APIC CPU, usable
    body.extend([200, 4, 0, 0]); // unknown kind: skipped
    body.extend([0, 8, 2, 2, 1, 0, 0, 0]);
    assert_eq!(madt(&make(b"APIC", &body)), Interrupts { cpus: 3, io_apics: 1 });
    body.extend([0, 0, 9, 9]); // length 0 must not loop forever
    body.extend([0, 8, 3, 3, 1, 0, 0, 0]);
    assert_eq!(madt(&make(b"APIC", &body)), Interrupts { cpus: 3, io_apics: 1 });
    assert_eq!(madt(&make(b"APIC", &[0, 0, 0, 0, 0, 0, 0, 0, 0, 99, 1])), Interrupts::default());
    assert_eq!(madt(&make(b"APIC", &[0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 0, 3])), Interrupts::default());
    assert_eq!(madt(&[]), Interrupts::default());
}

/// Random bytes with the right signatures: nothing may panic or run on forever.
#[test]
fn random_damage() {
    let mut x = 0x9e37_79b9_7f4a_7c15u64;
    let mut rnd = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    for _ in 0..100_000 {
        let mut body: Vec<u8> = (0..rnd() % 120).map(|_| if rnd() % 2 == 0 { (rnd() % 20) as u8 } else { rnd() as u8 }).collect();
        body.truncate((rnd() % 120) as usize);
        let t = make(b"APIC", &body);
        let i = madt(table(&t, b"APIC").unwrap());
        assert!(i.cpus + i.io_apics <= t.len());
        for e in mcfg(&t) {
            assert!(e.base % (1 << 20) == 0 && e.first_bus <= e.last_bus && e.base + (256 << 20) <= 1 << 52);
        }
        let _ = entries(&t, rnd() % 2 == 0).count();
        let n = (rnd() % t.len() as u64) as usize;
        let _ = (madt(&t[..n]), mcfg(&t[..n]).count(), header(&t[..n]), table(&t[..n], b"APIC"), parse_rsdp(&t[..n]));
    }
}
