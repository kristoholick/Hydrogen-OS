//! Deliberate faults, each compiled in only by its own Cargo feature, to prove the kernel
//! catches errors instead of silently resetting. Never enabled in a normal build.

#[cfg(feature = "fault-pagefault")]
pub fn page_fault() {
    crate::kprintln!("[sigil] fault test: writing to unmapped address 0xdeadb000");
    // SAFETY: deliberately invalid — this write must trigger a page fault, which the IDT
    // handler reports. Nothing is actually written.
    unsafe { (0xdead_b000 as *mut u64).write_volatile(42) };
}

/// Recurse forever, 256 bytes of stack per level, until a guard page stops it.
#[cfg(any(feature = "fault-stack", feature = "fault-thread-stack"))]
#[allow(unconditional_recursion)]
#[inline(never)]
fn dive(depth: u64) -> u64 {
    let frame = [depth; 32];
    core::hint::black_box(&frame);
    dive(depth + 1) + frame[0]
}

#[cfg(feature = "fault-stack")]
pub fn stack_overflow() {
    crate::kprintln!("[sigil] fault test: recursing until the 64 KiB stack hits its guard page");
    core::hint::black_box(dive(0));
}

#[cfg(feature = "fault-thread-stack")]
pub fn thread_stack_overflow() {
    crate::kprintln!("[sigil] fault test: a thread recurses until its own 64 KiB stack hits its guard page");
    let id = crate::task::spawn("overflow", || {
        core::hint::black_box(dive(0));
    })
    .expect("spawn failed");
    crate::task::join(id);
}

#[cfg(feature = "fault-int3")]
pub fn breakpoint() {
    crate::kprintln!("[sigil] fault test: int3 breakpoint (recoverable)");
    x86_64::instructions::interrupts::int3();
    crate::kprintln!("[sigil] fault test: resumed after breakpoint");
}
