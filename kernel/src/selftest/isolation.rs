//! Checks that a program that misbehaves hurts only itself.

use super::elf::{file, start};
use crate::mm::{heap, pmm};
use crate::proc::{self, ExitStatus};
use crate::{kprintln, sched};
use alloc::vec::Vec;

/// What `crash-test` does for each argument (see its source), and what must stop it. `ANY` where
/// CPUs (or QEMU's emulation) differ in which exception they raise.
const ANY: &str = "";
const PF: &str = "PAGE FAULT";
const GP: &str = "GENERAL PROTECTION FAULT";
const UD: &str = "INVALID OPCODE";
const DB: &str = "DEBUG";
const CRASHES: [&str; 58] = [
    PF, PF, PF, PF, PF, PF, "DIVIDE ERROR", UD, GP, GP, // 1-10
    GP, GP, GP, GP, GP, GP, ANY, PF, UD, "DEVICE NOT AVAILABLE", // 11-20
    DB, DB, GP, GP, GP, GP, GP, GP, GP, GP, // 21-30
    PF, PF, ANY, ANY, ANY, GP, GP, GP, GP, ANY, // 31-40
    ANY, PF, ANY, ANY, ANY, GP, GP, UD, GP, GP, // 41-50
    GP, GP, GP, GP, GP, GP, GP, GP, // 51-58
];

pub fn run() {
    crashes();
    bad_arguments();
    no_leaks();
}

fn crash(kind: usize) -> ExitStatus {
    let pid = proc::spawn_elf("crash-test", file("crash-test"), kind as u64).expect("cannot start crash-test");
    proc::wait(pid, 5000)
}

/// Every kind of crash, each next to innocent programs, which must finish undisturbed.
fn crashes() {
    let mut bystanders: Vec<_> = [start("hello-a"), start("hello-b")].into();
    let wrong: Vec<_> = CRASHES
        .iter()
        .enumerate()
        .map(|(i, &want)| {
            bystanders.push(start("memory-check"));
            (i + 1, want, crash(i + 1))
        })
        .filter(|&(_, want, got)| !matches!(got, ExitStatus::Killed(why) if want == ANY || why == want))
        .collect();
    assert!(wrong.is_empty(), "crash-test (kind, expected, got): {wrong:?}");
    for pid in bystanders {
        assert_eq!(proc::wait(pid, 5000), ExitStatus::Exited(0), "an innocent program was hurt");
    }
    kprintln!("[hydrogen] selftest isolation: ok ({} kinds of crash, each ended only its own program)", CRASHES.len());
}

fn bad_arguments() {
    let pids = [start("abuse-test"), start("abuse-test")];
    for pid in pids {
        assert_eq!(proc::wait(pid, 10_000), ExitStatus::Exited(0), "abuse-test got a wrong answer");
    }
    kprintln!("[hydrogen] selftest isolation: ok (bad system call arguments come back as errors)");
}

fn no_leaks() {
    let snapshot = || {
        let h = heap::stats();
        (pmm::stats().free_frames, h.live, h.bytes_in_use, sched::thread_count())
    };
    let round = || (1..=CRASHES.len()).for_each(|kind| assert!(matches!(crash(kind), ExitStatus::Killed(_))));
    let before = snapshot();
    round();
    assert_eq!(snapshot(), before, "leak: (free frames, heap blocks, heap bytes, threads) changed");
    kprintln!("[hydrogen] selftest isolation: ok (crashed programs leaked no memory)");
}
