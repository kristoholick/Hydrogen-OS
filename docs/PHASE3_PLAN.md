# Phase 3 plan — Multitasking for Hydrogen (APPROVED by the owner 2026-11-23, with choices A–F as recommended)

**Goal (from ROADMAP.md):** threads, context switch, scheduler, ring-3 processes, `syscall`/`sysret`,
ELF loader.
**Accept:** two user processes print interleaved; a faulting process is killed without killing the kernel.

In plain words: today Hydrogen runs exactly one thing — the kernel. After Phase 3 it can run many
programs "at once" (taking turns 100 times a second), and those programs live in their own locked
rooms (ring 3, own memory). If one crashes, only that program dies; the OS keeps going.

---

## 1. Steps (each one = its own commit, its own automated test, pushed after it passes)

| Step | What gets built | New test | Pass means |
|---|---|---|---|
| **3.0 Rename to Hydrogen** | Boot screen, crash screen, serial log tags (`[sigil]` → `[hydrogen]`), crate names (`sigil-kernel` → `hydrogen-kernel`, `sigil-mm` → `hydrogen-mm`), ISO name, Limine menu entry, docs | existing `make test-all` with updated expected text | Everything still passes; screen and logs say **Hydrogen** |
| **3.1 Kernel threads** | Thread struct, per-thread 16 KiB kernel stack with guard page, context switch (save/restore registers, ~30 lines asm), `yield()` | `make test-threads` | Two kernel threads take turns printing `A1 B1 A2 B2 …` |
| **3.2 Preemption** | Timer interrupt switches threads every 10 ms (round-robin); `sleep(ms)`; idle thread (`hlt`); thread exit + cleanup | `make test-preempt` | A thread stuck in an endless loop does **not** stop the others; sleep wakes on time (±1 tick) |
| **3.3 Address spaces + ring 3** | One page table per process (kernel half shared, user half private); enter ring 3; TSS kernel-stack switch; `syscall`/`sysret` entry; first syscalls | `make test-user` | A tiny built-in user program calls `log("hello from ring 3")` and `exit(0)`; kernel prints its exit code |
| **3.4 ELF loader + boot modules** | Own small ELF64 parser (host unit-tested); user programs built in `userspace/`, shipped inside the ISO as Limine modules, loaded into fresh processes | `make test-elf` + unit tests | Two separate programs `hello-a`, `hello-b` load and run; output interleaves |
| **3.5 Fault isolation** | User faults kill only the process (page fault, divide-by-zero, bad instruction, GPF); syscalls check every user pointer | `make test-isolation` | A crashing program is reported and removed, the others finish, kernel reaches `phase 3 boot complete`; a syscall given a kernel address returns an error instead of crashing |

`make test-all` grows to include all of these, and every Phase 1–2 test must keep passing.

## 2. Choices — with my recommendation

| # | Question | Options | Recommendation |
|---|---|---|---|
| A | How do threads take turns? | (1) cooperative: each thread must call `yield` · (2) **preemptive**: the timer forces turns | **Preemptive round-robin, 10 ms slices.** Cooperative breaks as soon as one program loops forever. Priorities come later. |
| B | How many CPU cores? | (1) **one core now**, others later · (2) all cores now | **One core now.** Multi-core scheduling is a big bug source; locks are already interrupt-safe and will be made SMP-safe in a later step. Your i5 has 4 cores and the Ryzen has 6 — they stay idle for now. |
| C | How do programs get onto the machine? (no file system until Phase 5) | (1) **Limine boot modules** — files placed in the ISO, handed to the kernel at boot · (2) baked into the kernel binary | **Limine modules.** Programs stay separate files, like they will be on a real disk later. |
| D | ELF parser | (1) **write our own** (~200 lines, unit-tested) · (2) use a crate (`xmas-elf`) | **Our own.** It's small, it's security-critical (it reads untrusted files), and it matches "all policy code is ours" (decision #16). |
| E | What language are user programs in? | (1) **Rust `no_std`** + a tiny `libsigil` runtime we write · (2) C | **Rust**, same toolchain as the kernel. C support can come with a libc in Phase 5. |
| F | Extra hardware protection | SMEP (kernel can't run user code), SMAP (kernel can't touch user memory by accident), user-pointer checks | **All three.** Both of your PCs support SMEP/SMAP. The kernel turns them on when the CPU has them and logs a clear warning if it doesn't. |

### First syscalls (the "doors" from programs into the kernel)
| # | Name | Does |
|---|---|---|
| 0 | `log(ptr, len)` | write text to the serial log (and later the screen) |
| 1 | `exit(code)` | end this process |
| 2 | `yield()` | give up the rest of the time slice |
| 3 | `sleep(ms)` | wait |
| 4 | `getpid()` | own process number |
| 5 | `uptime_ms()` | milliseconds since boot |

Register convention: `rax` = number, `rdi rsi rdx r10 r8 r9` = arguments, `rax` = result
(negative = error). The numbers are ours, not Linux's.

## 3. Memory layout per process
```
0x0000_0000_0000_0000  unmapped (null-pointer page faults)
0x0000_0000_0040_0000  program code + data (from the ELF file)
0x0000_7fff_ffe0_0000  user stack (64 KiB) with a guard page below it
0x0000_8000_0000_0000  ── canonical hole ──
0xffff_8000_0000_0000  kernel half (same for every process, user can't touch it)
```

## 4. Not in Phase 3 (on purpose)
- Multiple cores (SMP), priorities, real-time scheduling
- `fork`, signals, pipes/IPC between programs, shared memory
- Files and a shell (Phase 5), keyboard input (Phase 4)
- FPU/SSE state switching: kernel and user programs are built without SSE for now, so there is
  nothing to save. Needed before Phase 6 (the compositor wants SIMD) — will get its own step.

## 5. Risks and how they're covered
| Risk | Guard |
|---|---|
| `sysret` with a bad return address is a known CPU security hole | Return address checked to be a canonical user address before every `sysret`; otherwise use the slower `iretq` |
| Wrong `swapgs` order → kernel runs with user's GS | Entry/exit asm reviewed against the Intel/AMD manuals; a test makes a syscall from inside an interrupt-heavy loop |
| Kernel stack overflow inside a thread | Each thread stack has a guard page; existing double-fault emergency stack still catches it |
| Leaking memory when processes exit | Test counts free frames before and after running 50 short programs — must be equal |
| Works on Intel but not AMD (or reverse) | Suite runs on both PCs (laptop + Ryzen desktop) before the phase is marked done |

## 6. New / changed files
```
kernel/src/sched/      threads, run queue, context switch
kernel/src/proc/       processes, address spaces, ELF loading
kernel/src/syscall/    syscall entry + table, user-pointer checks
crates/sigil-elf/      ELF64 parser (host unit tests)
userspace/libsigil/    tiny runtime: _start, syscall wrappers, log!
userspace/apps/        hello-a, hello-b, crash-test
boot/limine.conf       + module_path lines
Makefile               build userspace, add modules to ISO, new test targets
```
