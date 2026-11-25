use super::*;
use std::cell::Cell;
use std::collections::HashMap;

#[derive(Default)]
struct Fake {
    words: HashMap<(u8, u8, u8, u16), u32>,
    reads: Cell<usize>,
}

impl Config for Fake {
    fn read(&self, at: Address, offset: u16) -> u32 {
        assert!(at.device < 32 && at.function < 8 && offset % 4 == 0 && offset < 4096);
        self.reads.set(self.reads.get() + 1);
        let exists = self.words.contains_key(&(at.bus, at.device, at.function, 0));
        let missing = if exists { 0 } else { u32::MAX };
        *self.words.get(&(at.bus, at.device, at.function, offset)).unwrap_or(&missing)
    }
}

impl Fake {
    fn set(&mut self, at: (u8, u8, u8), offset: u16, v: u32) {
        self.words.insert((at.0, at.1, at.2, offset), v);
    }
    fn device(&mut self, at: (u8, u8, u8), vendor: u16, id: u16, class: u32, several: bool) {
        self.set(at, 0, vendor as u32 | (id as u32) << 16);
        self.set(at, 8, class << 8 | 7);
        self.set(at, 0xc, (several as u32) << 23);
    }
    fn bridge(&mut self, at: (u8, u8, u8), to: u8, several: bool) {
        self.device(at, 0x8086, 0x100, 0x06_04_00, several);
        self.set(at, 0xc, (several as u32) << 23 | 1 << 16);
        self.set(at, 0x18, (to as u32) << 8 | at.0 as u32 | 0xff << 16);
    }
    fn found(&self, first: u8, last: u8) -> Vec<String> {
        let mut v = Vec::new();
        scan(self, first, last, |d| v.push(format!("{} {:04x}:{:04x}", d.at, d.vendor, d.id)));
        v
    }
}

#[test]
fn nothing_there() {
    assert!(Fake::default().found(0, 255).is_empty());
}

#[test]
fn devices_and_functions() {
    let mut f = Fake::default();
    f.device((0, 0, 0), 0x8086, 0x29c0, 0x06_00_00, false);
    f.device((0, 0, 3), 1, 1, 0, false); // function 0 says it is alone: never looked at
    f.device((0, 31, 0), 0x8086, 0x2918, 0x06_01_00, true);
    f.device((0, 31, 2), 0x8086, 0x2922, 0x01_06_01, false);
    f.device((0, 31, 7), 0x8086, 0x2930, 0x0c_05_00, false);
    f.device((0, 5, 1), 2, 2, 0, false); // no function 0: does not exist
    f.device((1, 0, 0), 3, 3, 0, false); // bus 1 is behind no bridge
    assert_eq!(f.found(0, 255), ["00:00.0 8086:29c0", "00:1f.0 8086:2918", "00:1f.2 8086:2922", "00:1f.7 8086:2930"]);
}

#[test]
fn details() {
    let mut f = Fake::default();
    f.device((0, 4, 0), 0x1b36, 0x0010, 0x01_08_02, false);
    let mut v = Vec::new();
    scan(&f, 0, 0, |d| v.push(d));
    let at = Address { bus: 0, device: 4, function: 0 };
    assert_eq!(v, [Device { at, vendor: 0x1b36, id: 0x10, class: 1, subclass: 8, interface: 2, revision: 7, bridge_to: None }]);
    assert_eq!(v[0].kind(), "NVMe disk");
}

#[test]
fn behind_bridges() {
    let mut f = Fake::default();
    f.bridge((0, 1, 0), 1, true);
    f.bridge((0, 1, 1), 5, false);
    f.device((1, 0, 0), 0x10de, 1, 0x03_00_00, false);
    f.bridge((5, 2, 0), 200, false);
    f.device((200, 31, 0), 9, 9, 0x0c_03_30, false);
    assert_eq!(
        f.found(0, 255),
        ["00:01.0 8086:0100", "00:01.1 8086:0100", "01:00.0 10de:0001", "05:02.0 8086:0100", "c8:1f.0 0009:0009"]
    );
    assert_eq!(f.found(0, 100), ["00:01.0 8086:0100", "00:01.1 8086:0100", "01:00.0 10de:0001", "05:02.0 8086:0100"]);
    assert_eq!(f.found(5, 255), ["05:02.0 8086:0100", "c8:1f.0 0009:0009"]);
}

#[test]
fn bridge_loops_end() {
    let mut f = Fake::default();
    f.bridge((0, 1, 0), 0, false); // to itself
    f.bridge((0, 2, 0), 7, false);
    f.bridge((7, 0, 0), 0, false); // back up
    f.bridge((7, 1, 0), 7, false);
    f.bridge((0, 3, 0), 7, false); // a second way to bus 7
    assert_eq!(f.found(0, 255).len(), 5);
    // every bus behind every other: still each bus once
    let mut f = Fake::default();
    for bus in 0..=255 {
        for dev in 0..32 {
            f.bridge((bus, dev, 0), bus.wrapping_mul(31).wrapping_add(dev), false);
        }
    }
    assert_eq!(f.found(0, 255).len(), 256 * 32);
    assert!(f.reads.get() < 256 * 32 * 8);
}

#[test]
fn only_real_bridges_count() {
    let mut f = Fake::default();
    f.device((0, 1, 0), 1, 1, 0x06_04_00, false); // bridge class, ordinary layout
    f.set((0, 1, 0), 0x18, 3 << 8);
    f.device((0, 2, 0), 1, 2, 0x02_00_00, false); // bridge layout, network class
    f.set((0, 2, 0), 0xc, 1 << 16);
    f.set((0, 2, 0), 0x18, 3 << 8);
    f.device((3, 0, 0), 1, 3, 0, false);
    assert_eq!(f.found(0, 255).len(), 2);
}

#[test]
fn capability_lists() {
    let at = Address { bus: 0, device: 3, function: 0 };
    let mut f = Fake::default();
    f.device((0, 3, 0), 1, 1, 0, false);
    f.set((0, 3, 0), 0x34, 0x40);
    f.set((0, 3, 0), 0x40, MSI_X as u32 | 0x53 << 8); // low bits of the link are ignored
    f.set((0, 3, 0), 0x50, VENDOR as u32 | 0x60 << 8);
    f.set((0, 3, 0), 0x60, MSI as u32);
    assert_eq!(capabilities(&f, at).count(), 0); // status bit says: no list
    f.set((0, 3, 0), 4, 1 << 20);
    assert_eq!(capabilities(&f, at).collect::<Vec<_>>(), [(MSI_X, 0x40), (VENDOR, 0x50), (MSI, 0x60)]);
    f.set((0, 3, 0), 0x60, MSI as u32 | 0x40 << 8); // a loop
    assert_eq!(capabilities(&f, at).count(), 48);
    f.set((0, 3, 0), 0x60, MSI as u32 | 0x10 << 8); // into the standard header
    assert_eq!(capabilities(&f, at).count(), 3);
    f.set((0, 3, 0), 0x50, u32::MAX); // device vanished
    assert_eq!(capabilities(&f, at).count(), 1);
}

#[test]
fn every_class_has_a_name() {
    let mut f = Fake::default();
    f.device((0, 0, 0), 1, 1, 0, false);
    let mut d = None;
    scan(&f, 0, 0, |x| d = Some(x));
    let mut d = d.unwrap();
    for (c, s, i, name) in [(0xc, 3, 0x30, "USB 3 controller (xHCI)"), (2, 0, 0, "Ethernet controller"), (0xff, 0, 0, "unknown device")] {
        (d.class, d.subclass, d.interface) = (c, s, i);
        assert_eq!(d.kind(), name);
    }
}
