//! Kernel threads and the scheduler (Phase 3.1 + 3.2): one core, round-robin, 10 ms slices.
//!
//! Every thread has its own 16 KiB kernel stack with an unmapped guard page below it (see
//! `stack`). The boot path becomes thread 0 ("kmain") and keeps the 64 KiB boot stack.
//! Thread 1 is the idle thread: it runs only when nobody else wants the CPU.
//!
//! Switching: `switch_context` pushes the callee-saved registers on the old stack, stores the
//! stack pointer in the old thread, loads the new thread's stack pointer and pops its registers.
//! Everything else is already on the old stack: caller-saved registers because the switch is an
//! ordinary function call, and — when the timer forces it — the whole interrupted state, saved
//! by the interrupt entry. No SSE/FPU state exists yet (kernel is built without SSE).
//!
//! For threads of user processes a switch also changes the page tables and the stack used for
//! entries from ring 3.
//!
//! Rules that keep this sound on one core:
//! - every switch happens with interrupts off, and the thread that comes back restores its own
//!   interrupt flag;
//! - `IrqMutex` masks interrupts, so a thread is never preempted while it holds a lock;
//! - `yield_now`/`sleep_ms` refuse to run with interrupts off, i.e. while a lock is held.
//!
//! Known limit: woken sleepers go to the front of the queue so they wake on time. A thread that
//! keeps sleeping briefly and then burning its whole slice could therefore crowd out others;
//! real priorities come later.

mod stack;

use crate::arch::x86_64::{gdt, space, syscall};
use crate::proc::Process;
use crate::sync::IrqMutex;
use crate::time;
use alloc::boxed::Box;
use alloc::collections::{BTreeMap, VecDeque};
use core::sync::atomic::{AtomicBool, Ordering};
use x86_64::instructions::interrupts;

pub use stack::is_guard as is_thread_stack_guard;

pub type ThreadId = u64;
pub const IDLE: ThreadId = 1;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    Ready,
    Running,
    /// Wakes at this timer tick.
    Sleeping(u64),
    Dead,
}

struct Thread {
    #[allow(dead_code)] // shown by diagnostics / the task manager later
    name: &'static str,
    state: State,
    /// Saved stack pointer while the thread is not running.
    rsp: u64,
    entry: Option<fn()>,
    /// Timer ticks that arrived while this thread was running.
    cpu_ticks: u64,
    /// Freed when the thread is reaped. `None` for thread 0, which runs on the boot stack.
    stack: Option<stack::ThreadStack>,
    /// The user program this thread runs, if any; freed together with the thread.
    process: Option<Box<Process>>,
}

struct Scheduler {
    /// Boxed so each thread's `rsp` keeps its address while the map changes.
    threads: BTreeMap<ThreadId, Box<Thread>>,
    /// Threads waiting for the CPU. Never contains the running or the idle thread.
    ready: VecDeque<ThreadId>,
    current: ThreadId,
    next_id: ThreadId,
    /// Exited thread whose stack is freed by whoever runs next (it cannot free its own).
    zombie: Option<ThreadId>,
}

static SCHED: IrqMutex<Option<Scheduler>> = IrqMutex::new(None);
static PREEMPT: AtomicBool = AtomicBool::new(false);

/// Turn the code that is running right now into thread 0 and create the idle thread.
pub fn init() {
    let boot = Box::new(Thread {
        name: "kmain", state: State::Running, rsp: 0, entry: None, cpu_ticks: 0, stack: None, process: None,
    });
    *SCHED.lock() = Some(Scheduler {
        threads: BTreeMap::from([(0, boot)]),
        ready: VecDeque::new(),
        current: 0,
        next_id: IDLE,
        zombie: None,
    });
    let id = add_thread("idle", idle, None, false);
    assert_eq!(id, IDLE);
}

fn idle() {
    crate::arch::idle_forever()
}

/// From now on the timer switches threads. Needs a running timer.
pub fn enable_preemption() {
    PREEMPT.store(true, Ordering::Relaxed);
}

/// Create a thread that will run `entry`; it joins the back of the queue.
pub fn spawn(name: &'static str, entry: fn()) -> ThreadId {
    add_thread(name, entry, None, true)
}

/// Like `spawn`, for a thread that belongs to a user process. `entry` runs in the kernel, with
/// the process's page tables active.
pub fn spawn_process(name: &'static str, entry: fn(), process: Box<Process>) -> ThreadId {
    add_thread(name, entry, Some(process), true)
}

fn add_thread(name: &'static str, entry: fn(), process: Option<Box<Process>>, queue: bool) -> ThreadId {
    let stack = stack::ThreadStack::new();
    // Initial stack, from the top down, so the first `switch_context` into it "returns" into
    // `thread_start` with the alignment a normal call would leave (rsp % 16 == 8 at entry):
    //   [top-8]            0               fake return address of thread_start (never used)
    //   [top-16]           thread_start    popped by `ret`
    //   [top-64 .. top-16] rbp, rbx, r12..r15 = 0, popped by switch_context
    let rsp = stack.top() - 64;
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
    let thread = Box::new(Thread {
        name, state: State::Ready, rsp, entry: Some(entry), cpu_ticks: 0, stack: Some(stack), process,
    });

    let mut g = SCHED.lock();
    let s = g.as_mut().expect("scheduler not initialised");
    let id = s.next_id;
    s.next_id += 1;
    s.threads.insert(id, thread);
    // Room for every thread at once, so the timer interrupt never has to grow the queue.
    s.ready.reserve(s.threads.len());
    if queue {
        s.ready.push_back(id);
    }
    id
}

/// First code every new thread runs (reached by `ret` from `switch_context`, interrupts off).
extern "C" fn thread_start() -> ! {
    reap();
    interrupts::enable();
    let entry = {
        let g = SCHED.lock();
        let s = g.as_ref().expect("scheduler not initialised");
        s.threads[&s.current].entry.expect("thread without entry")
    };
    entry();
    exit()
}

/// Let the other ready threads run first. Returns when this thread's turn comes again, or
/// immediately if nobody else is ready.
pub fn yield_now() {
    assert!(interrupts::are_enabled(), "yield with interrupts off (is a lock held?)");
    interrupts::without_interrupts(|| schedule(State::Ready));
}

/// Sleep for at least `ms` milliseconds; other threads run meanwhile. Returns how many timer
/// ticks later than planned the thread got the CPU back (0 = on time).
pub fn sleep_ms(ms: u64) -> u64 {
    assert!(interrupts::are_enabled(), "sleep with interrupts off (is a lock held?)");
    assert!(PREEMPT.load(Ordering::Relaxed), "sleep before preemption is on: nothing would wake us");
    let tick_ms = 1000 / time::HZ as u64;
    interrupts::without_interrupts(|| {
        // +1 because the current tick is already partly over: never wake early.
        let until = time::ticks().saturating_add(ms.div_ceil(tick_ms)).saturating_add(1);
        schedule(State::Sleeping(until));
        time::ticks() - until
    })
}

/// End the current thread. The next thread to run frees its stack.
pub fn exit() -> ! {
    interrupts::disable();
    schedule(State::Dead);
    unreachable!("a dead thread was scheduled again");
}

/// Called by the timer interrupt after it has acknowledged the interrupt (interrupts off).
pub fn on_tick() {
    if !PREEMPT.load(Ordering::Relaxed) {
        return;
    }
    {
        let mut g = SCHED.lock();
        let Some(s) = g.as_mut() else { return };
        let now = time::ticks();
        if let Some(t) = s.threads.get_mut(&s.current) {
            t.cpu_ticks += 1;
        }
        for (&id, t) in s.threads.iter_mut() {
            if let State::Sleeping(until) = t.state
                && until <= now
            {
                t.state = State::Ready;
                s.ready.push_front(id);
            }
        }
    }
    schedule(State::Ready);
}

/// Switch to the next thread, leaving the current one in state `leave`. Returns when the
/// current thread is picked again. Must run with interrupts off and no lock held.
fn schedule(leave: State) {
    let (old_rsp, new_rsp) = {
        let mut g = SCHED.lock();
        let s = g.as_mut().expect("scheduler not initialised");
        let cur = s.current;
        assert!(cur != IDLE || leave == State::Ready, "the idle thread must not sleep or exit");
        let next = match s.ready.pop_front() {
            Some(id) => id,
            None if leave == State::Ready => return, // nobody else wants the CPU: carry on
            None => IDLE,
        };
        match leave {
            State::Ready if cur != IDLE => s.ready.push_back(cur),
            State::Dead => s.zombie = Some(cur),
            _ => {}
        }
        s.current = next;
        let old = s.threads.get_mut(&cur).expect("current thread missing");
        old.state = leave;
        let old_rsp = &raw mut old.rsp;
        let new = s.threads.get_mut(&next).expect("next thread missing");
        new.state = State::Running;
        // Safe a moment before the stack switch: all page tables share the kernel half, and the
        // entry stack is only used when ring 3 is left.
        match &new.process {
            Some(p) => {
                p.space.activate();
                gdt::clear_data_segments();
            }
            None => space::activate_kernel(),
        }
        if let Some(stack) = &new.stack {
            syscall::set_kernel_stack(stack.top());
        }
        (old_rsp, new.rsp)
        // lock released here; interrupts stay off until the switch is done
    };
    // SAFETY: `old_rsp` points into a boxed Thread that is only freed by `reap`, i.e. after
    // this switch has finished; `new_rsp` was saved by `switch_context` (or built by
    // `add_thread`) for a thread that is not running. Interrupts are off and there is one
    // core, so nothing runs between releasing the lock and the switch.
    unsafe { switch_context(old_rsp, new_rsp) }
    reap();
}

/// Free the thread that just exited, if any. Runs first thing after every switch, interrupts
/// off, on the new thread's stack — so the dead thread's stack is no longer in use.
fn reap() {
    let dead = {
        let mut g = SCHED.lock();
        let s = g.as_mut().expect("scheduler not initialised");
        s.zombie.take().and_then(|id| s.threads.remove(&id))
    };
    drop(dead); // unmaps the stack; done after releasing the scheduler lock
}

/// Look at the process of the current thread (`None` for a kernel thread). Holds the scheduler
/// lock meanwhile, so `f` must be short and must not print.
pub fn with_process<R>(f: impl FnOnce(&Process) -> R) -> Option<R> {
    let g = SCHED.lock();
    let s = g.as_ref()?;
    s.threads.get(&s.current)?.process.as_deref().map(f)
}

pub fn current_id() -> ThreadId {
    SCHED.lock().as_ref().map_or(0, |s| s.current)
}

/// Has the thread not been reaped yet?
pub fn is_alive(id: ThreadId) -> bool {
    SCHED.lock().as_ref().is_some_and(|s| s.threads.contains_key(&id))
}

/// Timer ticks the thread has spent running; `None` once it is gone.
pub fn cpu_ticks(id: ThreadId) -> Option<u64> {
    SCHED.lock().as_ref().and_then(|s| s.threads.get(&id).map(|t| t.cpu_ticks))
}

pub fn thread_count() -> usize {
    SCHED.lock().as_ref().map_or(0, |s| s.threads.len())
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
