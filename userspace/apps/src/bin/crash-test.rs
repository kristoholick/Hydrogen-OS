//! Does one forbidden thing, chosen by the start argument (the kernel's self-test knows the list).
//! The kernel must end this program and carry on. Exit code 0 = it was NOT stopped.
#![no_std]
#![no_main]

use core::arch::asm;
use core::hint::black_box;

libhydrogen::entry!(main);

const KERNEL: u64 = 0xffff_ffff_8000_0000;
const NOT_CANONICAL: u64 = 0x8000_0000_0000;

fn read(addr: u64) {
    // SAFETY: none — the point is to be stopped here.
    unsafe { black_box((addr as *const u64).read_volatile()) };
}

fn write(addr: u64) {
    // SAFETY: none — the point is to be stopped here.
    unsafe { (addr as *mut u64).write_volatile(42) };
}

fn jump(addr: u64) {
    // SAFETY: none — the point is to be stopped here.
    unsafe { asm!("call {}", in(reg) addr) };
}

#[inline(never)]
#[allow(unconditional_recursion)]
fn overflow(n: u64) -> u64 {
    let pad = black_box([n; 64]);
    overflow(n + 1) + pad[1]
}

fn main() -> i32 {
    // SAFETY: none of this is safe, on purpose; the kernel has to cope with it.
    unsafe {
        match libhydrogen::arg() {
            1 => read(8),                       // null pointer
            2 => read(KERNEL),                  // kernel memory: read,
            3 => write(KERNEL + 0x10_0000),     //   write,
            4 => jump(KERNEL),                  //   execute
            5 => write(main as *const () as u64), // own code is read-only
            6 => {
                let code = black_box([0xc3c3_c3c3_c3c3_c3c3u64; 2]); // `ret`
                jump(code.as_ptr() as u64); // the stack is not executable
            }
            7 => asm!("xor edx, edx", "mov eax, 1", "xor ecx, ecx", "div ecx", out("rax") _, out("rdx") _, out("rcx") _),
            8 => asm!("ud2"),
            9 => asm!("cli"),
            10 => asm!("hlt"),
            11 => asm!("out 0x80, al", in("al") 0u8),
            12 => asm!("in al, 0x64", out("al") _),
            13 => asm!("int3"),
            14 => asm!("int 0x20"), // pretend to be the timer
            15 => asm!("int 0x8"),  // pretend to be a double fault
            16 => read(NOT_CANONICAL),
            17 => jump(NOT_CANONICAL),
            18 => {
                black_box(overflow(0));
            }
            19 => asm!(".byte 0x0f, 0x28, 0xc1"), // movaps xmm0, xmm1: SSE is off
            20 => asm!("fld1"),                   // x87 is off
            21 => asm!("pushfq", "or qword ptr [rsp], 0x100", "popfq", "nop", "nop"), // single-step
            22 => asm!(".byte 0xf1"),             // icebp
            23 => asm!("rdmsr", in("ecx") 0xc000_0080u32, out("eax") _, out("edx") _),
            24 => asm!("wrmsr", in("ecx") 0xc000_0082u32, in("eax") 0, in("edx") 0), // LSTAR
            25 => asm!("mov cr3, {}", in(reg) 0u64),
            26 => asm!("mov {}, cr0", out(reg) _),
            27 => asm!("mov dr7, {}", in(reg) 0u64),
            28 => asm!("sub rsp, 16", "sgdt [rsp]", "add rsp, 16"), // UMIP hides the kernel's tables
            29 => asm!("lgdt [{}]", in(reg) &[0u64; 2]),
            30 => asm!("swapgs"),
            31 => asm!("mov rsp, 8", "push rax", options(noreturn)),
            32 => asm!("mov rsp, {}", "push rax", in(reg) KERNEL + 0x1000, options(noreturn)),
            33 => asm!("mov rsp, {}", "push rax", in(reg) NOT_CANONICAL + 0x1000, options(noreturn)),
            34 => asm!("sysretq", in("rcx") 0, in("r11") 0),
            35 => asm!("push 0", "push 0x202", "push 0x08", "push 0", "push 0", "iretq", options(noreturn)), // fake return to ring 0
            36 => asm!("push 0x08", "lea rax, [rip + 2f]", "push rax", "retfq", "2:", out("rax") _), // ring 0 code segment
            37 => asm!("mov ss, {0:x}", in(reg) 0x10u16), // kernel data segment
            38 => asm!("ltr {0:x}", in(reg) 0x28u16),
            39 => asm!("invlpg [{}]", in(reg) KERNEL),
            // single-stepping across a system call must not stop the kernel, whatever the CPU does
            40 => asm!("pushfq", "or qword ptr [rsp], 0x100", "mov eax, 4", "popfq", "syscall", "nop",
                out("rax") _, out("rcx") _, out("r11") _),
            41 => asm!("pushfq", "or qword ptr [rsp], 0x100", "mov eax, 4", "mov edx, ss", "popfq", "mov ss, edx", "syscall", "nop",
                out("rax") _, out("rcx") _, out("r11") _, out("rdx") _),
            42 => read(0xffff_8000_0010_0000), // the kernel's view of all RAM
            43 => asm!("jmp {}", in(reg) 0x7fff_ffff_ffffu64, options(noreturn)), // last byte of user space
            44 => asm!("sysenter", options(noreturn)), // the other way into the kernel, never set up
            45 => asm!("sysexit", options(noreturn)),
            46 => asm!("int 0x80"),
            47 => asm!("rdpmc", in("ecx") 0, out("eax") _, out("edx") _),
            48 => asm!("xgetbv", in("ecx") 0, out("eax") _, out("edx") _),
            49 => asm!("lldt {0:x}", in(reg) 0x18u16),
            50 => asm!("clts"),
            51 => asm!("mov {}, cr2", out(reg) _),
            52 => asm!("mov cr8, {}", in(reg) 15u64),
            53 => asm!("lmsw {0:x}", in(reg) 0u16),
            54 => asm!("pushfq", "or qword ptr [rsp], 0x3000", "popfq", "cli"), // fake I/O privilege
            55 => asm!("int 0xff"),
            56 => asm!("mov rsp, {}", "int3", in(reg) KERNEL + 0x1000, options(noreturn)),
            57 => asm!("mov ax, 0x1b", "mov ss, ax", "mov ax, 0x07", "mov ds, ax", out("rax") _), // a local descriptor
            58 => asm!("mov ax, 0x43", "mov gs, ax", out("rax") _), // past the end of the descriptor table
            _ => return 99,
        }
    }
    0
}
