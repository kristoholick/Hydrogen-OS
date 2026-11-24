//! Deliberate faults, each compiled in only by its own Cargo feature, to prove the kernel
//! catches errors instead of silently resetting. Never enabled in a normal build.

#[cfg(feature = "fault-pagefault")]
pub fn page_fault() {
    crate::kprintln!("[hydrogen] fault test: writing to unmapped address 0xdeadb000");
    // SAFETY: deliberately invalid — this write must trigger a page fault, which the IDT
    // handler reports. Nothing is actually written.
    unsafe { (0xdead_b000 as *mut u64).write_volatile(42) };
}

#[cfg(feature = "fault-stack")]
pub fn stack_overflow() {
    #[allow(unconditional_recursion)]
    #[inline(never)]
    fn dive(depth: u64) -> u64 {
        let frame = [depth; 32]; // 256 bytes per level
        core::hint::black_box(&frame);
        dive(depth + 1) + frame[0]
    }
    crate::kprintln!("[hydrogen] fault test: recursing until the 64 KiB stack hits its guard page");
    core::hint::black_box(dive(0));
}

/// Same as `stack_overflow`, but inside a kernel thread: its 16 KiB stack must hit its own
/// guard page (not run into another thread's memory).
#[cfg(feature = "fault-thread-stack")]
pub fn thread_stack_overflow() {
    #[allow(unconditional_recursion)]
    #[inline(never)]
    fn dive(depth: u64) -> u64 {
        let frame = [depth; 32];
        core::hint::black_box(&frame);
        dive(depth + 1) + frame[0]
    }
    crate::kprintln!("[hydrogen] fault test: a thread recursing until its 16 KiB stack hits its guard page");
    crate::sched::spawn("overflow", || {
        core::hint::black_box(dive(0));
    });
    crate::sched::yield_now();
}

#[cfg(feature = "fault-int3")]
pub fn breakpoint() {
    crate::kprintln!("[hydrogen] fault test: int3 breakpoint (recoverable)");
    x86_64::instructions::interrupts::int3();
    crate::kprintln!("[hydrogen] fault test: resumed after breakpoint");
}

/// A kernel bug that touches (or runs) program memory directly must be stopped by the CPU
/// (SMAP/SMEP). Maps a program page in a scratch address space, then misuses it.
#[cfg(any(feature = "fault-smap", feature = "fault-smep"))]
pub fn user_memory() {
    use crate::arch::x86_64::space::{Access, AddressSpace};
    crate::kprintln!("[hydrogen] fault test: the kernel touches program memory at 0x400000");
    let mut space = AddressSpace::new().unwrap();
    space.map(0x40_0000, Access { write: false, exec: true }).unwrap();
    space.fill(0x40_0000, &[0xc3]).unwrap(); // `ret`
    x86_64::instructions::interrupts::disable(); // stay in this address space
    space.activate();
    // SAFETY: deliberately wrong — must page-fault although the page is mapped and valid.
    unsafe {
        #[cfg(feature = "fault-smap")]
        core::hint::black_box((0x40_0000 as *const u8).read_volatile());
        #[cfg(feature = "fault-smep")]
        core::arch::asm!("call {}", in(reg) 0x40_0000u64);
    }
    crate::kprintln!("[hydrogen] fault test: NOT STOPPED");
}
