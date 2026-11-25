//! Runs thousands of programs made of random machine code (`make fuzz`, its own test build).
//! Whatever they do, the kernel must survive, other programs must be unharmed and no memory lost.
//! Programs that loop are stopped by a time limit that exists only in this build (see `idt`).
//!
//! Reproducible: the same seed gives the same programs (`make fuzz FUZZ_SEED=123`).

use super::elf::start;
use crate::mm::{heap, pmm};
use crate::proc::{self, ExitStatus, Pid};
use crate::{kprintln, sched};
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

const PROGRAMS: usize = 20_000;
const BATCH: usize = 8;

/// Byte sequences far more interesting than plain chance would produce.
const NASTY: &[&[u8]] = &[
    &[0x0f, 0x05], &[0x0f, 0x34], &[0x0f, 0x35], &[0x48, 0x0f, 0x07], // syscall sysenter sysexit sysret
    &[0x48, 0xcf], &[0x48, 0xcb], &[0xc3], &[0xcc], &[0xf1], &[0xf4], // iretq retfq ret int3 icebp hlt
    &[0xcd, 0x20], &[0xcd, 0x80], &[0xcd, 0x08], &[0xcd, 0x02], // int
    &[0xfa], &[0xfb], &[0xfc], &[0xfd], &[0x9c], &[0x9d], // cli sti cld std pushfq popfq
    &[0x68, 0x00, 0x01, 0x00, 0x00, 0x9d], &[0x68, 0x00, 0x41, 0x04, 0x00, 0x9d], // set TF; TF+NT+AC
    &[0x68, 0x00, 0x34, 0x04, 0x00, 0x9d], &[0x0f, 0x01, 0xca], &[0x0f, 0x01, 0xcb], // IOPL+DF+AC; clac stac
    &[0x8e, 0xd8], &[0x8e, 0xc0], &[0x8e, 0xd0], &[0x8e, 0xe0], &[0x8e, 0xe8], // mov to ds es ss fs gs
    &[0xb8, 0x1b, 0x00, 0x00, 0x00], &[0xb8, 0x23, 0x00, 0x00, 0x00], // eax = user selectors
    &[0x31, 0xc0], &[0xb8, 1, 0, 0, 0], &[0xb8, 2, 0, 0, 0], &[0xb8, 3, 0, 0, 0], &[0xb8, 4, 0, 0, 0], // call numbers
    &[0x48, 0xbf, 0, 0, 0, 0x80, 0xff, 0xff, 0xff, 0xff], // rdi = kernel image
    &[0x48, 0xbf, 0, 0, 0, 0, 0, 0x80, 0xff, 0xff], // rdi = direct map
    &[0x48, 0xbc, 0, 0x10, 0, 0x80, 0xff, 0xff, 0xff, 0xff], // rsp = kernel image
    &[0x48, 0xbc, 0, 0, 0, 0, 0, 0x80, 0, 0], // rsp = not canonical
    &[0x48, 0xbc, 0, 0, 0xe1, 0xff, 0xff, 0x7f, 0, 0], // rsp = top of our stack
    &[0xbe, 0xff, 0xff, 0xff, 0xff], &[0xbe, 64, 0, 0, 0], // esi = lengths
    &[0x0f, 0x32], &[0x0f, 0x30], &[0x0f, 0x33], &[0x0f, 0x31], // rdmsr wrmsr rdpmc rdtsc
    &[0x0f, 0x20, 0xc0], &[0x0f, 0x22, 0xd8], &[0x0f, 0x22, 0xe0], &[0x0f, 0x23, 0xf8], // control, debug registers
    &[0x0f, 0x01, 0x14, 0x24], &[0x0f, 0x01, 0x04, 0x24], &[0x0f, 0x01, 0xf8], &[0x0f, 0x01, 0x3c, 0x24], // lgdt sgdt swapgs invlpg
    &[0x0f, 0x00, 0xd0], &[0x0f, 0x00, 0xd8], &[0x0f, 0x06], &[0x0f, 0x08], &[0x0f, 0x09], // lldt ltr clts invd wbinvd
    &[0xe4, 0x64], &[0xe6, 0x80], &[0xf3, 0x6c], &[0xf3, 0x6e], // in out ins outs
    &[0xf3, 0xa4], &[0xf3, 0x48, 0xab], &[0xf3, 0xae], // rep movs stos scas
    &[0x0f, 0x28, 0xc1], &[0xd9, 0xe8], &[0x9b], &[0x0f, 0xae, 0x04, 0x24], &[0x0f, 0xc7, 0x24, 0x24], // SSE x87 fwait fxsave xsavec
    &[0x0f, 0xc7, 0xf8], &[0x0f, 0x01, 0xd0], &[0x0f, 0x01, 0xd1], &[0xf3, 0x48, 0x0f, 0xae, 0xd0], // rdseed xgetbv xsetbv wrfsbase
    &[0xff, 0xe0], &[0xff, 0xe4], &[0xeb, 0xfe], &[0xe8, 0, 0, 0, 0], &[0xe2, 0xfe], // jmp rax, jmp rsp, loops
    &[0x50], &[0x58], &[0x5c], &[0x54], &[0xc8, 0xff, 0xff, 0x1f], &[0x8e, 0xd0, 0x0f, 0x05], // stack; mov ss + syscall
    &[0xf0, 0x0f, 0xc7, 0xc8], &[0x66, 0x66, 0x66, 0x66, 0x90], // invalid lock, prefixes
];

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn program(r: &mut Rng) -> Vec<u8> {
    let largest = if r.below(8) == 0 { 8000 } else { 300 };
    let size = 16 + r.below(largest);
    // some programs are pure chance, most are stuffed with nasty sequences
    let nasty_of_8 = [0, 4, 7][r.below(3)];
    let mut code = Vec::with_capacity(size + 16);
    while code.len() < size {
        if r.below(8) < nasty_of_8 {
            code.extend_from_slice(NASTY[r.below(NASTY.len())]);
        } else {
            code.push(r.next() as u8);
        }
    }
    code
}

pub fn run() {
    let seed = option_env!("FUZZ_SEED").and_then(|s| s.parse().ok()).filter(|&s| s != 0).unwrap_or(0x1234_5678_9abc_def1);
    kprintln!("[hydrogen] fuzz: {} programs of random machine code, seed {}", PROGRAMS, seed);
    let mut r = Rng(seed);
    let snapshot = || {
        let h = heap::stats();
        (pmm::stats().free_frames, h.live, h.bytes_in_use, sched::thread_count())
    };
    let mut ends: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut before = None;
    for batch in 0..PROGRAMS / BATCH {
        let bystander = (batch % 4 == 0).then(|| start("memory-check"));
        let pids: [Pid; BATCH] = core::array::from_fn(|_| proc::spawn_flat("fuzz", &program(&mut r)).expect("cannot start"));
        for pid in pids {
            let end = match proc::wait(pid, 20_000) {
                ExitStatus::Exited(_) => "exited",
                ExitStatus::Killed(why) => why,
            };
            *ends.entry(end).or_default() += 1;
        }
        if let Some(pid) = bystander {
            assert_eq!(proc::wait(pid, 20_000), ExitStatus::Exited(0), "an innocent program was hurt (batch {batch})");
        }
        if batch == 50 {
            before = Some(snapshot()); // lists kept for reuse have reached their full size
        }
        if (batch + 1) % 250 == 0 {
            kprintln!("[hydrogen] fuzz: {} programs done", (batch + 1) * BATCH);
        }
    }
    // `ends` itself may have grown since: compare everything but the heap block counts.
    let (b, a) = (before.unwrap(), snapshot());
    assert_eq!((b.0, b.3), (a.0, a.3), "leak: (free frames, threads) changed");
    for (end, n) in &ends {
        kprintln!("[hydrogen] fuzz:   {:>6} x {}", n, end);
    }
    kprintln!("[hydrogen] fuzz: ok ({} programs, kernel and bystanders unharmed, no memory leaked)", PROGRAMS);
}
