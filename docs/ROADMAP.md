# Roadmap

Each phase ends with acceptance tests. The next phase starts only after they pass and the owner agrees.

## Phase 0 — Planning and Design Lab
- [x] Read-only host check
- [x] Planning docs, safety rules, decision log
- [x] Design Lab (browser prototype of the three realms)
- [ ] Owner picks visual direction in the Design Lab → frozen into DESIGN_SYSTEM.md tokens
- [ ] Owner gives top-10 Windows features
- [x] Owner installs WSL2 (`wsl --install`), then approves toolchain install

## Phase 1 — First boot  ✅ done 2026-11-22
Limine (pinned) + Rust kernel, COM1 serial, GOP framebuffer, panic handler, UEFI ISO, QEMU+OVMF,
headless test script with screen dump.
**Accept:** `make test` boots in QEMU, `serial.log` contains the startup banner, `frame.png` shows a
filled framebuffer with text; a deliberate panic prints file/line to serial.

Results (QEMU 10.2.1 + KVM, OVMF, Limine 11.2.1):
- [x] `make test` PASS — serial shows `SIGIL kernel online` … `phase 1 boot complete`
- [x] Boot screen rendered at 1280×800×32 — `docs/screenshots/phase1-boot.png`
- [x] `make test-panic` PASS — `PANIC at kernel/src/main.rs:100:5` on serial + crimson screen — `docs/screenshots/phase1-panic.png`
- [x] Only `build/` and `target/` written (checked with `git status`); temp QEMU socket removed

## Phase 2 — Kernel foundations  ✅ done 2026-11-22
GDT/TSS (3 emergency stacks), IDT (all 32 exceptions), legacy PIC masked, Local APIC (xAPIC + x2APIC)
timer calibrated against the PIT, bitmap PMM, own 4-level page tables (2 MiB direct map, per-section
kernel permissions, NX + WP), 64 KiB kernel stack with guard page, size-class kernel heap.
CPU table formats via the `x86_64` crate 0.15.5 (decision #16); all policy code is ours.
**Accept:** page fault caught and reported; `Vec`/`Box` work; timer ticks logged; unit tests for allocators.

Results (`make test-all`, QEMU 10.2.1 + KVM):
- [x] `make unit` — 13/13 `sigil-mm` host tests pass (bitmap + heap)
- [x] `make test` — all boot stages logged; self-checks pmm/paging/heap ok; timer: **100 ticks per RTC second**
- [x] `make test-fault-pagefault` — `PAGE FAULT at 0xdeadb000 (CAUSED_BY_WRITE)` + crimson screen
- [x] `make test-fault-stack` — `DOUBLE FAULT … kernel stack overflow caught on emergency stack`, no reset
- [x] `make test-fault-int3` — breakpoint handled, kernel resumed to `phase 2 boot complete`
- [x] `make test-panic` — Phase 1 crash test still passes
- [x] Every run checked for triple-fault resets (none); only `build/` and `target/` written
- Screenshots: `docs/screenshots/phase2-boot.png`, `phase2-page-fault.png`, `phase2-stack-overflow.png`

## Phase 2.5 — First boot on real hardware (owner's Dell Latitude 3400)  ← postponed (no USB stick at hand)
Owner writes the ISO to a USB stick (Rufus) and disables Secure Boot. No disk drivers exist yet, so the
internal SSD cannot be touched. Check: boots, screen draws, timer check passes on real hardware.

## Phase 3 — Multitasking  ← in progress
Threads, context switch, scheduler, ring-3 processes, `syscall`/`sysret`, ELF loader.
**Accept:** two user processes print interleaved; a faulting process is killed without killing the kernel.

Steps (single CPU core first):
- [x] **3a — kernel threads + context switch** (2026-11-23): guarded 64 KiB thread stacks, naked-asm
  `switch_context`, cooperative round-robin `yield_now`/`exit`/`join`, idle thread, zombie reaping.
  - [x] `make test` — `selftest threads: ok (ABABABABAB …; 64 more threads ran; stacks freed; guard pages in place)`
  - [x] `make test-fault-thread-stack` — `DOUBLE FAULT … thread stack overflow caught on emergency stack`, no reset
  - [x] all Phase 1–2 tests still pass (`make test-all`, cloud container, QEMU 8.2 TCG — no KVM there)
  - Screenshots: `docs/screenshots/phase3a-boot.png`, `phase3a-thread-stack-overflow.png`
- [ ] **3b — preemption + sleep**: the timer forces a switch; `sleep(ms)`; idle thread halts the CPU
- [ ] **3c — ring 3**: per-process page tables, user mode, TSS `rsp0`
- [ ] **3d — `syscall`/`sysret`**: `write`, `exit`, `yield`
- [ ] **3e — ELF loader**: load two separate programs (→ phase acceptance test)

## Phase 4 — Hardware
PCIe enumeration, uACPI, VirtIO (blk, net, input, gpu), PS/2 keyboard/mouse, AHCI.
**Accept:** device list printed with vendor/device IDs; reads a sector from a VirtIO disk image.

## Phase 5 — Usable system
VFS, FAT32 + RamFS/initrd, shell, basic libc-like runtime.
**Accept:** shell can `ls`, `cat`, run a program from disk.

## Phase 6 — Compositor and the three realms
Pre-baked backdrops, dirty rects, taffy/tiny-skia/fontdue, app model, Theatre/Aether/Keep, radial wheel.
**Accept:** 60 FPS realm switching at 1080p in QEMU; same app renders in all three realms; each
screen matches the LOCKED spec in DESIGN_SYSTEM.md (left PS5-style Theatre, center visionOS-style
Aether, right medieval Keep) — owner signs off on screenshots of all three.

## Phase 7 — Networking
VirtIO-net → e1000, smoltcp, DHCP, DNS, sockets.
**Accept:** DHCP lease + DNS lookup + HTTP GET in QEMU.

## Phase 8 — Hardware expansion and first bare-metal boot
NVMe (read-only first), xHCI + USB HID + gamepads, HDA audio, laptop touchpad, USB-stick boot.

## Phase 9 — Security and maturity
Permissions, sandboxing, signed updates, Secure Boot, encryption, crash recovery.
