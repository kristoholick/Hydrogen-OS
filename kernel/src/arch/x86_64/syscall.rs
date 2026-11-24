//! The doors between ring 3 and ring 0: `syscall`/`sysret`, and the first jump into ring 3.
//!
//! The entry code needs the running thread's kernel stack before it has any register to spare,
//! so it uses two statics. That is sound because there is one core and interrupts are off until
//! the switch is done. Doing it this way also avoids `swapgs` and its ordering traps; per-core
//! data (and `swapgs`) comes with multi-core support.
//!
//! All user registers are saved on entry and restored on exit (except rax = result, and
//! rcx/r11, which the `syscall` instruction itself overwrites), so nothing of the kernel's leaks.

use super::gdt;
use super::space::USER_END;
use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::VirtAddr;
use x86_64::instructions::interrupts;
use x86_64::registers::model_specific::{Efer, EferFlags, LStar, SFMask, Star};
use x86_64::registers::rflags::RFlags;

static KERNEL_RSP: AtomicU64 = AtomicU64::new(0);
static USER_RSP: AtomicU64 = AtomicU64::new(0);

/// What `syscall_entry` pushes, lowest address first.
#[repr(C)]
#[allow(dead_code)] // the assembly reads and writes the fields Rust does not use
struct Frame {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    rbp: u64,
    rbx: u64,
    r9: u64,
    r8: u64,
    r10: u64,
    rdx: u64,
    rsi: u64,
    rdi: u64,
    rax: u64,
    rip: u64,
    rflags: u64,
    rsp: u64,
}

unsafe extern "C" {
    fn syscall_entry();
}

core::arch::global_asm!(
    ".global syscall_entry",
    "syscall_entry:", // interrupts off (SFMASK); rcx = user rip, r11 = user rflags
    "    mov [rip + {user_rsp}], rsp",
    "    mov rsp, [rip + {kernel_rsp}]",
    "    push qword ptr [rip + {user_rsp}]",
    "    push r11",
    "    push rcx",
    "    push rax",
    "    push rdi",
    "    push rsi",
    "    push rdx",
    "    push r10",
    "    push r8",
    "    push r9",
    "    push rbx",
    "    push rbp",
    "    push r12",
    "    push r13",
    "    push r14",
    "    push r15",
    "    mov rdi, rsp", // 16 pushes: rsp is 16-byte aligned again, as `call` wants it
    "    call {handler}",
    "    cli", // no interrupt may arrive on the user's stack pointer
    "    pop r15",
    "    pop r14",
    "    pop r13",
    "    pop r12",
    "    pop rbp",
    "    pop rbx",
    "    pop r9",
    "    pop r8",
    "    pop r10",
    "    pop rdx",
    "    pop rsi",
    "    pop rdi",
    "    pop rax",
    "    pop rcx",
    "    pop r11",
    "    pop rsp",
    "    sysretq",
    user_rsp = sym USER_RSP,
    kernel_rsp = sym KERNEL_RSP,
    handler = sym handler,
);

extern "C" fn handler(f: &mut Frame) {
    super::cpu::assert_flags_clean();
    interrupts::enable(); // we are on the thread's own kernel stack now
    f.rax = crate::syscall::dispatch(f.rax, [f.rdi, f.rsi, f.rdx, f.r10, f.r8, f.r9]) as u64;
    // `sysret` to a non-canonical address faults in ring 0 with the user's stack (a well-known
    // CPU trap). No page that high can be mapped, so this cannot happen; check anyway.
    if f.rip >= USER_END {
        crate::proc::kill_current("BAD RETURN ADDRESS", format_args!(" {:#x}", f.rip));
    }
}

/// Set up the `syscall` instruction. Needs the GDT.
pub fn init() {
    let s = gdt::selectors();
    Star::write(s.user_code, s.user_data, s.kernel_code, s.kernel_data).expect("GDT order does not fit sysret");
    LStar::write(VirtAddr::new(syscall_entry as *const () as u64));
    // Cleared on entry: interrupts off until we are on a safe stack; no single-stepping in the
    // kernel; string copies go forward; SMAP cannot be bypassed; no stray flags.
    SFMask::write(
        RFlags::INTERRUPT_FLAG
            | RFlags::TRAP_FLAG
            | RFlags::DIRECTION_FLAG
            | RFlags::ALIGNMENT_CHECK
            | RFlags::NESTED_TASK
            | RFlags::IOPL_LOW
            | RFlags::IOPL_HIGH,
    );
    // SAFETY: LSTAR, STAR and SFMASK are set up above, so the instruction is safe to allow.
    unsafe { Efer::update(|f| f.insert(EferFlags::SYSTEM_CALL_EXTENSIONS)) };
}

/// Where the CPU puts the stack when ring 3 enters the kernel (syscall or interrupt). Called on
/// every thread switch, interrupts off.
pub fn set_kernel_stack(top: u64) {
    KERNEL_RSP.store(top, Ordering::Relaxed);
    gdt::set_ring0_stack(top);
}

/// Jump into ring 3 for the first time: rdi = `arg`, every other register zero, interrupts on.
pub fn enter_user(rip: u64, rsp: u64, arg: u64) -> ! {
    assert!(rip < USER_END && rsp <= USER_END, "bad user entry point");
    let s = gdt::selectors();
    interrupts::disable(); // `iretq` turns them back on
    // SAFETY: builds the frame `iretq` expects from valid ring-3 selectors and checked canonical
    // addresses. The process's page tables are active and its kernel stack is set, so whatever
    // the program does can only come back through the kernel's own entry points.
    unsafe {
        core::arch::asm!(
            "push rax", // ss
            "push rcx", // rsp
            "push 0x202", // rflags: interrupts on, nothing else
            "push rdx", // cs
            "push rsi", // rip
            "xor eax, eax",
            "xor ebx, ebx",
            "xor ecx, ecx",
            "xor edx, edx",
            "xor esi, esi",
            "xor ebp, ebp",
            "xor r8d, r8d",
            "xor r9d, r9d",
            "xor r10d, r10d",
            "xor r11d, r11d",
            "xor r12d, r12d",
            "xor r13d, r13d",
            "xor r14d, r14d",
            "xor r15d, r15d",
            "iretq",
            in("rax") s.user_data.0 as u64,
            in("rcx") rsp,
            in("rdx") s.user_code.0 as u64,
            in("rsi") rip,
            in("rdi") arg,
            options(noreturn)
        )
    }
}
