//! Basic CPU control.

use core::sync::atomic::{AtomicBool, Ordering};
use x86_64::registers::control::{Cr0, Cr0Flags, Cr4, Cr4Flags};
use x86_64::registers::model_specific::{Efer, EferFlags};
use x86_64::registers::rflags::{self, RFlags};

/// Mask maskable interrupts on this core.
#[inline]
pub fn disable_interrupts() {
    x86_64::instructions::interrupts::disable();
}

/// Stop this core forever with interrupts masked. Used after a panic.
pub fn halt_forever() -> ! {
    loop {
        disable_interrupts();
        x86_64::instructions::hlt();
    }
}

/// Sleep between interrupts forever (timer keeps ticking). Used at the end of boot.
pub fn idle_forever() -> ! {
    loop {
        x86_64::instructions::interrupts::enable_and_hlt();
    }
}

/// Does the CPU support the no-execute page bit?
pub fn has_nx() -> bool {
    // CPUID is always available in 64-bit mode; leaf 0x8000_0001 exists on every x86-64 CPU.
    let r = core::arch::x86_64::__cpuid(0x8000_0001);
    r.edx & (1 << 20) != 0
}

/// Turn on the protections our page tables rely on:
/// NX (no-execute bit honoured) and WP (kernel cannot write to read-only pages).
pub fn enable_memory_protection() {
    // SAFETY: NX support was checked by the caller (`has_nx`); setting EFER.NXE only makes
    // the CPU honour bit 63 of page entries. CR0.WP only adds write checks for ring 0.
    unsafe {
        Efer::update(|f| f.insert(EferFlags::NO_EXECUTE_ENABLE));
        Cr0::update(|f| f.insert(Cr0Flags::WRITE_PROTECT));
    }
}

static SMAP_ON: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
pub struct UserGuards {
    pub smep: bool,
    pub smap: bool,
    pub umip: bool,
}

/// Turn on the CPU's protections against user programs, as far as it has them, and close doors
/// programs do not need. Must run while no user-accessible page is mapped.
pub fn enable_user_guards() -> UserGuards {
    let has_leaf_7 = core::arch::x86_64::__cpuid(0).eax >= 7;
    let (ebx, ecx) = if has_leaf_7 {
        let r = core::arch::x86_64::__cpuid_count(7, 0);
        (r.ebx, r.ecx)
    } else {
        (0, 0)
    };
    let g = UserGuards { smep: ebx & (1 << 7) != 0, smap: ebx & (1 << 20) != 0, umip: ecx & (1 << 2) != 0 };
    // SAFETY: SMEP/SMAP/UMIP are only set when CPUID reports them, and the kernel neither runs
    // nor touches user pages. The kernel is built without x87/SSE, so turning those off (until
    // their registers are saved per thread) affects only user programs: they fault instead of
    // silently sharing registers. The other cleared bits are features nothing here uses.
    unsafe {
        Cr4::update(|f| {
            f.set(Cr4Flags::SUPERVISOR_MODE_EXECUTION_PROTECTION, g.smep);
            f.set(Cr4Flags::SUPERVISOR_MODE_ACCESS_PREVENTION, g.smap);
            f.set(Cr4Flags::USER_MODE_INSTRUCTION_PREVENTION, g.umip);
            f.remove(Cr4Flags::OSFXSR | Cr4Flags::OSXMMEXCPT_ENABLE | Cr4Flags::OSXSAVE
                | Cr4Flags::FSGSBASE | Cr4Flags::PERFORMANCE_MONITOR_COUNTER);
        });
        Cr0::update(|f| f.insert(Cr0Flags::EMULATE_COPROCESSOR));
    }
    SMAP_ON.store(g.smap, Ordering::Relaxed);
    g
}

/// First thing in every interrupt that can arrive in ring 3: a program can set the flag that
/// suspends SMAP, and interrupts (unlike `syscall`) do not clear it.
#[inline]
pub fn close_smap() {
    if SMAP_ON.load(Ordering::Relaxed) {
        // SAFETY: the instruction exists when SMAP does; it only clears EFLAGS.AC.
        unsafe { core::arch::asm!("clac", options(nomem, nostack)) };
    }
}

/// Cheap check, on every entry from ring 3: no program may suspend SMAP or reverse the direction
/// of memory copies inside the kernel.
#[inline]
pub fn assert_flags_clean() {
    let flags = rflags::read();
    let ac = flags.contains(RFlags::ALIGNMENT_CHECK) && SMAP_ON.load(Ordering::Relaxed);
    assert!(!ac, "SMAP is suspended inside the kernel");
    assert!(!flags.contains(RFlags::DIRECTION_FLAG), "memory copies run backwards inside the kernel");
}
