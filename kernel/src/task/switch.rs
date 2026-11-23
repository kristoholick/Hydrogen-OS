//! The context switch: the only place the CPU moves from one thread's stack to another's.
//!
//! A suspended thread's whole state is its stack. On top of that stack sit the six
//! registers the System V ABI says a function must preserve (rbp, rbx, r12–r15) and a
//! return address. Every other register is either caller-saved (the compiler already
//! assumes `switch_context` clobbers it) or not used by the kernel (x86_64-unknown-none
//! builds without SSE/x87, so there is no floating-point state to save yet).

/// Number of 8-byte slots a fresh thread's initial frame occupies below its stack top:
/// six zeroed callee-saved registers, the entry address, and a fake return address.
pub const INITIAL_FRAME_SLOTS: u64 = 8;

/// Save the current thread's registers on its stack, store its stack pointer in
/// `*save_rsp`, then load `load_rsp` and resume whatever thread was suspended there.
///
/// # Safety
/// - `save_rsp` must be valid for a write and stay valid until this thread is resumed.
/// - `load_rsp` must be a stack pointer saved by `switch_context` (or built by
///   `prepare_stack`) whose stack is still mapped and not in use by anything else.
/// - Interrupts must be disabled, so no interrupt runs on a half-switched stack.
#[unsafe(naked)]
pub unsafe extern "C" fn switch_context(save_rsp: *mut u64, load_rsp: u64) {
    core::arch::naked_asm!(
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "mov [rdi], rsp", // old thread: remember where its registers are
        "mov rsp, rsi",   // new thread: switch to its stack
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",
        "ret",            // into the new thread, right after its own switch_context call
    )
}

/// Build the initial frame of a new thread so that the first `switch_context` into it
/// "returns" into `entry`, with the stack aligned as if `entry` had been called.
/// Returns the stack pointer to hand to `switch_context`.
///
/// # Safety
/// `top` must be the 16-byte-aligned, exclusive end of a mapped, writable stack of at
/// least `INITIAL_FRAME_SLOTS * 8` bytes that nothing else is using.
pub unsafe fn prepare_stack(top: u64, entry: extern "C" fn() -> !) -> u64 {
    debug_assert!(top % 16 == 0);
    let frame = (top - INITIAL_FRAME_SLOTS * 8) as *mut u64;
    // Layout, low to high: r15 r14 r13 r12 rbx rbp | entry | 0 (fake return address).
    // After the six pops and `ret`, rsp = top - 8: 8 mod 16, exactly what a called
    // function expects. rbp = 0 ends any future stack-trace walk here.
    // SAFETY: the caller guarantees these 8 slots below `top` are mapped and unused.
    unsafe {
        for i in 0..6 {
            frame.add(i).write(0);
        }
        frame.add(6).write(entry as usize as u64);
        frame.add(7).write(0);
    }
    frame as u64
}
