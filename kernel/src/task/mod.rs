//! Kernel threads and the scheduler (Phase 3a: cooperative, single core).
//!
//! - Every thread has its own guarded 64 KiB stack (`stack.rs`) and a saved stack pointer.
//! - The scheduler is round-robin: `yield_now` puts the current thread at the back of the
//!   ready queue and switches to the front one.
//! - The thread that booted the kernel becomes thread 0 ("boot"); it keeps the 64 KiB
//!   boot stack set up by `mm::vmm`.
//! - The idle thread (thread 1) is never queued; it runs only when nothing else can, and
//!   halts the CPU until the next interrupt.
//! - A thread that finishes becomes a zombie. Its stack cannot be freed while it is still
//!   running on it, so the next scheduler call made from another thread frees it.
//!
//! Phase 3b adds preemption (the timer forces a switch) and sleeping.

mod stack;
mod switch;

use crate::sync::IrqMutex;
use alloc::boxed::Box;
use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;
use stack::KernelStack;
use x86_64::instructions::interrupts;

pub use stack::live as live_stacks;

pub type ThreadId = u64;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Ready,
    Running,
    Dead,
}

struct Thread {
    #[allow(dead_code)] // shown in diagnostics from Phase 3b on
    name: &'static str,
    state: State,
    /// Saved stack pointer while not running. Boxed `Thread`s never move, so
    /// `switch_context` may write here after the scheduler lock is released.
    rsp: u64,
    /// `None` only for the boot thread, which runs on the boot stack. Never read: owning it
    /// is the point — the stack is unmapped exactly when the dead thread is freed.
    #[allow(dead_code)]
    stack: Option<KernelStack>,
    /// What to run; taken by `thread_start` the first time the thread runs.
    entry: Option<Box<dyn FnOnce() + Send>>,
}

struct Scheduler {
    threads: BTreeMap<ThreadId, Box<Thread>>,
    ready: VecDeque<ThreadId>,
    zombies: Vec<ThreadId>,
    current: ThreadId,
    idle: ThreadId,
    next_id: ThreadId,
}

static SCHED: IrqMutex<Option<Scheduler>> = IrqMutex::new(None);

const BOOT_ID: ThreadId = 0;

/// Turn the running code into thread 0 and create the idle thread.
pub fn init() {
    let mut threads = BTreeMap::new();
    threads.insert(BOOT_ID, Box::new(Thread {
        name: "boot",
        state: State::Running,
        rsp: 0,
        stack: None,
        entry: None,
    }));
    let mut s = Scheduler {
        threads,
        ready: VecDeque::new(),
        zombies: Vec::new(),
        current: BOOT_ID,
        idle: 0,
        next_id: 1,
    };
    let idle = s.create("idle", Box::new(idle_loop)).expect("no memory for the idle thread");
    s.idle = idle;
    *SCHED.lock() = Some(s);
}

impl Scheduler {
    fn create(&mut self, name: &'static str, entry: Box<dyn FnOnce() + Send>) -> Option<ThreadId> {
        let stack = KernelStack::new()?;
        // SAFETY: the stack was just mapped for this thread alone; its top is 16-byte aligned.
        let rsp = unsafe { switch::prepare_stack(stack.top(), thread_start) };
        let id = self.next_id;
        self.next_id += 1;
        self.threads.insert(id, Box::new(Thread {
            name,
            state: State::Ready,
            rsp,
            stack: Some(stack),
            entry: Some(entry),
        }));
        Some(id)
    }
}

/// Start a new kernel thread running `f`. It runs once the current thread yields.
pub fn spawn<F: FnOnce() + Send + 'static>(name: &'static str, f: F) -> Option<ThreadId> {
    let mut g = SCHED.lock();
    let s = g.as_mut().expect("scheduler not initialised");
    let id = s.create(name, Box::new(f))?;
    s.ready.push_back(id);
    Some(id)
}

/// Let the next ready thread run; returns when this thread's turn comes round again.
pub fn yield_now() {
    schedule(false);
}

/// End the current thread. Its stack is freed later, from another thread.
pub fn exit() -> ! {
    schedule(true);
    unreachable!("a dead thread was resumed");
}

/// Wait (by yielding) until thread `id` has finished.
pub fn join(id: ThreadId) {
    while is_alive(id) {
        yield_now();
    }
}

pub fn is_alive(id: ThreadId) -> bool {
    let g = SCHED.lock();
    let s = g.as_ref().expect("scheduler not initialised");
    s.threads.get(&id).is_some_and(|t| t.state != State::Dead)
}

/// Threads that exist, including zombies not yet freed.
pub fn thread_count() -> usize {
    SCHED.lock().as_ref().map_or(0, |s| s.threads.len())
}

pub fn current_id() -> ThreadId {
    SCHED.lock().as_ref().map_or(BOOT_ID, |s| s.current)
}

fn schedule(exiting: bool) {
    let were_enabled = interrupts::are_enabled();
    interrupts::disable();

    // Everything is decided under the lock; the switch itself happens after releasing it
    // (interrupts stay off throughout, so nothing else can run in between).
    let mut freed: Vec<Box<Thread>> = Vec::new();
    let switch = {
        let mut g = SCHED.lock();
        let s = g.as_mut().expect("scheduler not initialised");

        // 1) free threads that died earlier — none of them is running now
        for id in s.zombies.drain(..) {
            if let Some(t) = s.threads.remove(&id) {
                freed.push(t);
            }
        }

        // 2) put the current thread back in line, or bury it
        let cur = s.current;
        if exiting {
            assert!(cur != BOOT_ID && cur != s.idle, "the boot/idle thread cannot exit");
            s.threads.get_mut(&cur).unwrap().state = State::Dead;
            s.zombies.push(cur);
        } else if cur != s.idle {
            s.threads.get_mut(&cur).unwrap().state = State::Ready;
            s.ready.push_back(cur);
        }

        // 3) pick who runs next
        let next = s.ready.pop_front().unwrap_or(s.idle);
        s.threads.get_mut(&next).unwrap().state = State::Running;
        if next == cur {
            None
        } else {
            s.current = next;
            let save: *mut u64 = &mut s.threads.get_mut(&cur).unwrap().rsp;
            let load = s.threads[&next].rsp;
            Some((save, load))
        }
    };
    drop(freed); // unmaps stacks; done outside the scheduler lock

    if let Some((save, load)) = switch {
        // SAFETY: `save` points into a boxed Thread that stays in the table until it is
        // reaped, which only happens after this thread is switched away from. `load` was
        // saved by `switch_context` or built by `prepare_stack`, and that thread's stack is
        // still mapped (zombies are only reaped when they are not current). Interrupts are off.
        unsafe { switch::switch_context(save, load) };
    }

    if were_enabled {
        interrupts::enable();
    }
}

/// First code every new thread runs (entered by `ret` from `switch_context`).
extern "C" fn thread_start() -> ! {
    let entry = {
        let mut g = SCHED.lock();
        let s = g.as_mut().expect("scheduler not initialised");
        let cur = s.current;
        s.threads.get_mut(&cur).unwrap().entry.take().expect("thread started twice")
    };
    // We arrived from `schedule`, which had interrupts off; a new thread runs with them on.
    interrupts::enable();
    entry();
    exit()
}

fn idle_loop() {
    loop {
        interrupts::enable_and_hlt(); // sleep until the next interrupt...
        yield_now(); //                  ...then see whether anyone became ready
    }
}

pub use stack::layout_ok as stack_layout_ok;
