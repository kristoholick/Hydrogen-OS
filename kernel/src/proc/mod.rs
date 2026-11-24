//! User processes: a program running in ring 3 with its own memory.
//!
//! A process has exactly one thread, and its number (pid) is that thread's id. The thread owns
//! the process; when it has exited, the next thread to run frees both (see `sched::reap`).

use crate::arch::x86_64::space::{Access, AddressSpace, MapError};
use crate::arch::x86_64::syscall::enter_user;
use crate::fmtbuf::FmtBuf;
use crate::sync::IrqMutex;
use crate::{kprintln, sched, time};
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use hydrogen_mm::PAGE_SIZE;

pub type Pid = sched::ThreadId;
/// Fixed size, so a name can be copied out without touching the heap.
pub type Name = FmtBuf<32>;

const CODE_BASE: u64 = 0x40_0000;
const STACK_BASE: u64 = 0x7fff_ffe0_0000; // the page below stays unmapped: guard page
const STACK_SIZE: u64 = 64 * 1024;

pub struct Process {
    pub name: Name,
    pub space: AddressSpace,
    entry: u64,
    stack_top: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExitStatus {
    Exited(i32),
    Killed(&'static str),
}

/// How finished processes ended, until somebody `wait`s for them.
static EXITS: IrqMutex<BTreeMap<Pid, ExitStatus>> = IrqMutex::new(BTreeMap::new());

/// Start a program given as plain machine code (runs from its first byte).
pub fn spawn_flat(name: &str, code: &[u8]) -> Result<Pid, MapError> {
    let mut space = AddressSpace::new().ok_or(MapError::OutOfMemory)?;
    for v in (CODE_BASE..CODE_BASE + code.len() as u64).step_by(PAGE_SIZE) {
        space.map(v, Access { write: false, exec: true })?;
    }
    space.fill(CODE_BASE, code)?;
    start(name, space, CODE_BASE)
}

fn start(name: &str, mut space: AddressSpace, entry: u64) -> Result<Pid, MapError> {
    for v in (STACK_BASE..STACK_BASE + STACK_SIZE).step_by(PAGE_SIZE) {
        space.map(v, Access { write: true, exec: false })?;
    }
    let name = crate::fmtbuf!(32, "{}", name);
    let process = Process { name, space, entry, stack_top: STACK_BASE + STACK_SIZE };
    Ok(sched::spawn_process("user", first_run, Box::new(process)))
}

/// Kernel side of a new process's thread: go to ring 3 and never come back this way.
fn first_run() {
    let (entry, stack_top) = sched::with_process(|p| (p.entry, p.stack_top)).expect("not a process");
    enter_user(entry, stack_top)
}

pub fn current_name() -> Name {
    sched::with_process(|p| p.name).unwrap_or_default()
}

/// End the current process. Its memory is freed by the next thread that runs.
pub fn exit_current(code: i32) -> ! {
    finish(ExitStatus::Exited(code))
}

/// End the current process because it did something it must not.
pub fn kill_current(why: &'static str) -> ! {
    finish(ExitStatus::Killed(why))
}

/// Nothing that owns heap memory may be alive on the caller's stack: it is thrown away.
fn finish(status: ExitStatus) -> ! {
    let (name, pid) = (current_name(), sched::current_id());
    match status {
        ExitStatus::Exited(code) => {
            kprintln!("[hydrogen] process {} (pid {}) exited with code {}", name.as_str(), pid, code)
        }
        ExitStatus::Killed(why) => {
            kprintln!("[hydrogen] process {} (pid {}) KILLED: {}", name.as_str(), pid, why)
        }
    }
    EXITS.lock().insert(pid, status);
    sched::exit()
}

/// Sleep until the process is completely gone (memory freed too), then say how it ended.
/// Panics after `limit_ms`; meant for the boot-time checks.
pub fn wait(pid: Pid, limit_ms: u64) -> ExitStatus {
    let start = time::ticks();
    while sched::is_alive(pid) {
        assert!((time::ticks() - start) * 1000 / time::HZ as u64 <= limit_ms, "process {pid} did not end in time");
        sched::sleep_ms(10);
    }
    EXITS.lock().remove(&pid).expect("process ended without a status")
}
