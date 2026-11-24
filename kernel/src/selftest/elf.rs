//! Checks for program files: loading, running side by side, refusing damaged files, no leaks.

use crate::boot;
use crate::mm::{heap, pmm};
use crate::proc::{self, ExitStatus, SpawnError};
use crate::{kprintln, sched};
use alloc::vec::Vec;

pub fn file(name: &str) -> &'static [u8] {
    boot::module(name).unwrap_or_else(|| panic!("program {name} is missing from the boot modules"))
}

pub fn start(name: &str) -> proc::Pid {
    proc::spawn_elf(name, file(name), 0).unwrap_or_else(|e| panic!("cannot start {name}: {e:?}"))
}

pub fn run() {
    side_by_side();
    memory();
    damaged_files();
    no_leaks();
}

fn side_by_side() {
    let (a, b) = (start("hello-a"), start("hello-b"));
    for pid in [a, b] {
        assert_eq!(proc::wait(pid, 5000), ExitStatus::Exited(0));
    }
    kprintln!("[hydrogen] selftest elf: ok (hello-a and hello-b ran side by side)");
}

fn memory() {
    let pids = [start("memory-check"), start("memory-check")];
    for pid in pids {
        assert_eq!(proc::wait(pid, 5000), ExitStatus::Exited(0), "memory-check found a problem");
    }
    kprintln!("[hydrogen] selftest elf: ok (memory-check, two copies)");
}

/// Damaged copies of a real program must be refused, or at least not harm the kernel.
fn damaged_files() {
    use hydrogen_elf::Error::*;
    let good = file("hello-a");
    let refused = |edit: &dyn Fn(&mut Vec<u8>)| {
        let mut f = good.to_vec();
        edit(&mut f);
        match proc::spawn_elf("damaged", &f, 0) {
            Err(SpawnError::File(e)) => e,
            other => panic!("damaged program not refused: {other:?}"),
        }
    };
    assert_eq!(refused(&|f| f.clear()), Truncated);
    assert_eq!(refused(&|f| f[0] = b'X'), NotElf);
    assert_eq!(refused(&|f| f.truncate(100)), Truncated);
    assert_eq!(refused(&|f| f[18] = 183), WrongMachine);
    assert_eq!(refused(&|f| f[31] = 0xff), BadEntry); // entry point in the kernel
    assert_eq!(refused(&|f| f[64 + 16 + 7] = 0xff), BadAddress); // first segment in the kernel
    assert_eq!(refused(&|f| f[64 + 4] = 7), WriteAndExec);
    kprintln!("[hydrogen] selftest elf: ok (damaged program files refused)");
}

fn no_leaks() {
    let ten = || {
        let pids: [_; 10] = core::array::from_fn(|_| start("memory-check"));
        for pid in pids {
            assert_eq!(proc::wait(pid, 5000), ExitStatus::Exited(0));
        }
        // refused programs must not leave half-built memory behind either
        let mut f = file("hello-a").to_vec();
        f[0] = 0;
        assert!(proc::spawn_elf("damaged", &f, 0).is_err());
    };
    let snapshot = || {
        let h = heap::stats();
        (pmm::stats().free_frames, h.live, h.bytes_in_use, sched::thread_count())
    };
    ten();
    let before = snapshot();
    (0..3).for_each(|_| ten());
    assert_eq!(snapshot(), before, "leak: (free frames, heap blocks, heap bytes, threads) changed");
    kprintln!("[hydrogen] selftest elf: ok (no memory leaked by 30 programs from files)");
}
