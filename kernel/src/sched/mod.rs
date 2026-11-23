//! Kernel threads (Phase 3.1): cooperative for now — a thread runs until it calls `yield_now`
//! or `exit`. The timer takes over the switching in step 3.2.
//!
//! Every thread has its own 16 KiB kernel stack with an unmapped guard page below it, in the
//! thread-stack region (see `mm::vmm::THREAD_STACKS`). The boot path becomes thread 0 ("kmain")
//! and keeps the 64 KiB boot stack.
//!
//! Switching: `switch_context` pushes the callee-saved registers on the old stack, stores the
//! stack pointer in the old thread, loads the new thread's stack pointer and pops its registers.
//! Everything else (caller-saved registers, RIP) is already on the stack because the switch is
//! an ordinary function call. No SSE/FPU state exists yet (kernel is built without SSE).

mod stack;

use crate::sync::IrqMutex;
use alloc::boxed::Box;
use alloc::vec::Vec;
use x86_64::instructions::interrupts;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Ready,
    Running,
    Dead,
}

/// A thread's id is its index in `Scheduler::threads`.
pub struct Thread {
    #[allow(dead_code)] // shown by diagnostics / the task manager later
    name: &'static str,
    state: State,
    /// Saved stack pointer while the thread is not running.
    rsp: u64,
    entry: Option<fn()>,
    /// `None` for thread 0, which runs on the boot stack.
    #[allow(dead_code)] // freed by the reaper in step 3.2
    stack: Option<stack::ThreadStack>,
}

struct Scheduler {
    /// Boxed so each thread's `rsp` field keeps its address while the Vec grows.
    threads: Vec<Box<Thread>>,
    current: usize,
}

static SCHED: IrqMutex<Option<Scheduler>> = IrqMutex::new(None);

/// Turn the code that is running right now into thread 0.
pub fn init() {
    let boot = Box::new(Thread { name: "kmain", state: State::Running, rsp: 0, entry: None, stack: None });
    *SCHED.lock() = Some(Scheduler { threads: alloc::vec![boot], current: 0 });
}

/// Create a thread that will run `entry` the next time the scheduler picks it.
pub fn spawn(name: &'static str, entry: fn()) -> usize {
    let stack = stack::ThreadStack::new();
    // Initial stack, from the top down, so the first `switch_context` into it "returns" into
    // `thread_start` with the alignment a normal call would leave (rsp % 16 == 8 at entry):
    //   [top-8]  0                 fake return address of thread_start (never used)
    //   [top-16] thread_start      popped by `ret`
    //   [top-64 .. top-24]         rbp, rbx, r12..r15 = 0, popped by switch_context
    let top = stack.top();
    let rsp = top - 64;
    // SAFETY: the 64 bytes below `top` lie inside the freshly mapped, writable stack and
    // nothing else references them yet.
    unsafe {
        let p = rsp as *mut u64;
        for i in 0..6 {
            p.add(i).write(0);
        }
        p.add(6).write(thread_start as extern "C" fn() -> ! as usize as u64);
        p.add(7).write(0);
    }
    let mut g = SCHED.lock();
    let s = g.as_mut().expect("scheduler not initialised");
    let id = s.threads.len();
    s.threads.push(Box::new(Thread { name, state: State::Ready, rsp, entry: Some(entry), stack: Some(stack) }));
    id
}

/// First code every new thread runs (reached by `ret` from `switch_context`).
extern "C" fn thread_start() -> ! {
    // We arrive with interrupts off (switches happen with them masked).
    interrupts::enable();
    let entry = {
        let g = SCHED.lock();
        let s = g.as_ref().unwrap();
        s.threads[s.current].entry.expect("thread without entry")
    };
    entry();
    exit()
}

/// Give the CPU to the next ready thread (round-robin). Returns when this thread is picked again.
/// If no other thread is ready, returns immediately.
pub fn yield_now() {
    interrupts::without_interrupts(|| switch_away(State::Ready));
}

/// End the current thread. Its stack is not reclaimed yet (step 3.2 adds the reaper).
pub fn exit() -> ! {
    interrupts::disable();
    switch_away(State::Dead);
    unreachable!("a dead thread was scheduled again");
}

/// Must run with interrupts off. `now` is the state the current thread is left in.
fn switch_away(now: State) {
    let (old_rsp, new_rsp) = {
        let mut g = SCHED.lock();
        let s = g.as_mut().expect("scheduler not initialised");
        let n = s.threads.len();
        let cur = s.current;
        let Some(next) = (1..=n).map(|k| (cur + k) % n).find(|&i| i != cur && s.threads[i].state == State::Ready) else {
            assert!(now != State::Dead, "last thread exited");
            return; // nobody else to run
        };
        s.threads[cur].state = now;
        s.threads[next].state = State::Running;
        s.current = next;
        (&mut s.threads[cur].rsp as *mut u64, s.threads[next].rsp)
        // lock released here; interrupts stay off until the switch is done
    };
    // SAFETY: `old_rsp` points into a boxed Thread that is never freed while it can be
    // switched back to; `new_rsp` was saved by `switch_context` (or built by `spawn`) for a
    // thread that is not running. Interrupts are off, so nothing runs between the two halves.
    unsafe { switch_context(old_rsp, new_rsp) }
}

pub fn current_id() -> usize {
    SCHED.lock().as_ref().map_or(0, |s| s.current)
}

pub fn state_of(id: usize) -> Option<State> {
    SCHED.lock().as_ref().and_then(|s| s.threads.get(id).map(|t| t.state))
}

unsafe extern "C" {
    /// Save callee-saved registers + rsp into `*old`, load `new` and restore its registers.
    fn switch_context(old: *mut u64, new: u64);
}

core::arch::global_asm!(
    ".global switch_context",
    "switch_context:",
    "    push rbp",
    "    push rbx",
    "    push r12",
    "    push r13",
    "    push r14",
    "    push r15",
    "    mov [rdi], rsp",
    "    mov rsp, rsi",
    "    pop r15",
    "    pop r14",
    "    pop r13",
    "    pop r12",
    "    pop rbx",
    "    pop rbp",
    "    ret",
);

pub use stack::is_guard as is_thread_stack_guard;
