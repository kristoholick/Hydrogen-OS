//! Checks for ring 3, system calls and private memory, using tiny programs built into the
//! kernel as machine code (position independent, so they can be copied anywhere).

use crate::mm::{heap, pmm};
use crate::proc::{self, ExitStatus};
use crate::{kprintln, sched, time};

macro_rules! program {
    ($name:ident, $start:literal, $end:literal) => {
        fn $name() -> &'static [u8] {
            unsafe extern "C" {
                #[link_name = $start]
                static START: u8;
                #[link_name = $end]
                static END: u8;
            }
            // SAFETY: both symbols delimit one read-only blob assembled below.
            unsafe {
                let start = &raw const START;
                core::slice::from_raw_parts(start, (&raw const END).offset_from(start) as usize)
            }
        }
    };
}

program!(hello_program, "user_hello", "user_hello_end");
program!(registers_program, "user_registers", "user_registers_end");
program!(storm_program, "user_storm", "user_storm_end");
program!(memory_program, "user_memory", "user_memory_end");

core::arch::global_asm!(
    ".pushsection .rodata.user_programs, \"a\"",
    // --- log("hello from ring 3"), exit(0)
    "user_hello:",
    "    mov eax, 0",
    "    lea rdi, [rip + 2f]",
    "    mov esi, 17",
    "    syscall",
    "    cmp rax, 17", // log returns the length
    "    jne 3f",
    "    mov ax, cs", // really in ring 3?
    "    and eax, 3",
    "    cmp eax, 3",
    "    jne 3f",
    "    mov eax, 1",
    "    xor edi, edi",
    "    syscall",
    "3:  mov eax, 1",
    "    mov edi, 1",
    "    syscall",
    "2:  .ascii \"hello from ring 3\"",
    "user_hello_end:",
    //
    // --- for 300 ms: every register except rax/rcx/r11 must survive system calls and forced
    //     switches; unknown calls must fail; exit(7) if all is well
    "user_registers:",
    "    mov eax, 5",
    "    syscall",
    "    mov r14, rax",
    "    add r14, 300", // r14 = end time
    "    mov eax, 4",
    "    syscall",
    "    mov r15, rax", // r15 = our pid: different in every copy
    "    push r15",
    "4:  lea rbx, [r15 + 1000]",
    "    lea rbp, [r15 + 2000]",
    "    lea r12, [r15 + 3000]",
    "    lea r13, [r15 + 4000]",
    "    lea rdi, [r15 + 5000]",
    "    lea rsi, [r15 + 6000]",
    "    lea rdx, [r15 + 7000]",
    "    lea r10, [r15 + 8000]",
    "    lea r8,  [r15 + 9000]",
    "    lea r9,  [r15 + 10000]",
    "    mov eax, 4", // getpid
    "    syscall",
    "    cmp rax, r15",
    "    jne 9f",
    "    lea r11, [r15 + 11000]",
    "    mov ecx, 40000000", // longer than a time slice, so the timer strikes in ring 3
    "5:  dec ecx",
    "    jnz 5b",
    "    lea rcx, [r15 + 11000]",
    "    cmp r11, rcx",
    "    jne 9f",
    "    mov eax, 2", // yield
    "    syscall",
    "    test rax, rax",
    "    jnz 9f",
    "    mov eax, 4", // a third call: two would undo a pair of swapped registers
    "    syscall",
    "    lea rcx, [r15 + 1000]",
    "    cmp rbx, rcx",
    "    jne 9f",
    "    lea rcx, [r15 + 2000]",
    "    cmp rbp, rcx",
    "    jne 9f",
    "    lea rcx, [r15 + 3000]",
    "    cmp r12, rcx",
    "    jne 9f",
    "    lea rcx, [r15 + 4000]",
    "    cmp r13, rcx",
    "    jne 9f",
    "    lea rcx, [r15 + 5000]",
    "    cmp rdi, rcx",
    "    jne 9f",
    "    lea rcx, [r15 + 6000]",
    "    cmp rsi, rcx",
    "    jne 9f",
    "    lea rcx, [r15 + 7000]",
    "    cmp rdx, rcx",
    "    jne 9f",
    "    lea rcx, [r15 + 8000]",
    "    cmp r10, rcx",
    "    jne 9f",
    "    lea rcx, [r15 + 9000]",
    "    cmp r8, rcx",
    "    jne 9f",
    "    lea rcx, [r15 + 10000]",
    "    cmp r9, rcx",
    "    jne 9f",
    "    cmp [rsp], r15",
    "    jne 9f",
    "    mov eax, 5",
    "    syscall",
    "    cmp rax, r14",
    "    jb 4b",
    "    mov eax, 99", // no such call
    "    syscall",
    "    cmp rax, -1",
    "    jne 9f",
    "    mov eax, 1",
    "    mov edi, 7",
    "    syscall",
    "9:  mov eax, 1",
    "    mov edi, 1",
    "    syscall",
    "user_registers_end:",
    //
    // --- nothing but system calls for 500 ms, so timer interrupts land all over the entry and
    //     exit code; exit(calls >= 10000)
    "user_storm:",
    "    mov eax, 5",
    "    syscall",
    "    lea r14, [rax + 500]",
    "    xor r13d, r13d",
    "7:  inc r13",
    "    mov eax, 5",
    "    syscall",
    "    cmp rax, r14",
    "    jb 7b",
    "    xor edi, edi",
    "    cmp r13, 10000",
    "    setae dil",
    "    mov eax, 1",
    "    syscall",
    "user_storm_end:",
    //
    // --- fresh memory must be all zero; then fill a stack page with our pid, sleep so others run,
    //     check it; sleep must not be early
    "user_memory:",
    "    lea rdi, [rsp - 61440]", // 15 of the 16 stack pages
    "    xor eax, eax",
    "    mov ecx, 7680",
    "    repe scasq",
    "    jne 8f",
    "    mov eax, 4",
    "    syscall",
    "    mov rbx, rax",
    "    mov r12d, 5",
    "6:  lea rdi, [rsp - 4096]",
    "    mov rax, rbx",
    "    mov ecx, 512",
    "    rep stosq",
    "    mov eax, 5",
    "    syscall",
    "    mov r13, rax",
    "    mov eax, 3", // sleep 20 ms
    "    mov edi, 20",
    "    syscall",
    "    mov eax, 5",
    "    syscall",
    "    sub rax, r13",
    "    cmp rax, 20",
    "    jb 8f",
    "    lea rdi, [rsp - 4096]",
    "    mov rax, rbx",
    "    mov ecx, 512",
    "    repe scasq",
    "    jne 8f",
    "    dec r12d",
    "    jnz 6b",
    "    mov eax, 1",
    "    mov edi, ebx", // exit(pid)
    "    syscall",
    "8:  mov eax, 1",
    "    mov edi, -1",
    "    syscall",
    "user_memory_end:",
    ".popsection",
);

pub fn run() {
    hello();
    registers();
    storm();
    private_memory();
    no_leaks();
}

fn hello() {
    let pid = proc::spawn_flat("hello", hello_program()).expect("cannot start program");
    assert_eq!(proc::wait(pid, 2000), ExitStatus::Exited(0), "hello failed");
    kprintln!("[hydrogen] selftest user: ok (hello ran in ring 3, exit code 0)");
}

/// Three copies at once, plus a kernel thread that never yields: forced switches everywhere.
fn registers() {
    use core::sync::atomic::{AtomicBool, Ordering};
    static STOP: AtomicBool = AtomicBool::new(false);
    let spinner = sched::spawn("spin", || {
        while !STOP.load(Ordering::Relaxed) {
            core::hint::spin_loop();
        }
    });
    let before = time::user_ticks();
    let pids: [_; 3] = core::array::from_fn(|_| proc::spawn_flat("registers", registers_program()).expect("cannot start"));
    for pid in pids {
        assert_eq!(proc::wait(pid, 5000), ExitStatus::Exited(7), "a register changed, or a call answered wrongly");
    }
    STOP.store(true, Ordering::Relaxed);
    super::join(&[spinner]);
    let struck = time::user_ticks() - before;
    assert!(struck >= 5, "the timer hit ring 3 only {struck} times: test proved nothing");
    kprintln!("[hydrogen] selftest user: ok (registers intact across system calls; timer struck ring 3 {} times)", struck);
}

fn storm() {
    let pid = proc::spawn_flat("storm", storm_program()).expect("cannot start program");
    assert_eq!(proc::wait(pid, 5000), ExitStatus::Exited(1), "system call storm failed");
    kprintln!("[hydrogen] selftest user: ok (500 ms of nothing but system calls)");
}

/// Four copies write different values to the same addresses.
fn private_memory() {
    let pids: [_; 4] = core::array::from_fn(|_| proc::spawn_flat("memory", memory_program()).expect("cannot start"));
    for pid in pids {
        assert_eq!(proc::wait(pid, 5000), ExitStatus::Exited(pid as i32), "memory of two programs mixed up, not zeroed, or sleep ended early");
    }
    kprintln!("[hydrogen] selftest user: ok (4 programs used the same addresses, each saw only its own data, zeroed at start)");
}

/// 50 programs leave no trace.
fn no_leaks() {
    let ten = || {
        let pids: [_; 10] = core::array::from_fn(|_| proc::spawn_flat("short", memory_program()).expect("cannot start"));
        for pid in pids {
            assert_eq!(proc::wait(pid, 5000), ExitStatus::Exited(pid as i32));
        }
    };
    let snapshot = || {
        let h = heap::stats();
        (pmm::stats().free_frames, h.live, h.bytes_in_use, sched::thread_count())
    };
    ten(); // warm-up: lists kept for reuse reach their full size
    let before = snapshot();
    (0..5).for_each(|_| ten());
    assert_eq!(snapshot(), before, "leak: (free frames, heap blocks, heap bytes, threads) changed");
    kprintln!("[hydrogen] selftest user: ok (50 programs started and finished, no memory leaked)");
}
