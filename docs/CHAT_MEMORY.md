# Chat memory — project kickoff session (2026-11-22)

A record of the first working session between the owner and Claude Code: what was asked,
what was decided, what was built, and what comes next. Read this first when resuming work.

---

## 1. What the owner wants

- A **real, independent operating system built from scratch** — own kernel with direct hardware
  access (ring 0), not a Linux distribution, not a web/desktop mock-up.
- Long-term ambition: support as much hardware as possible and offer equivalents of the
  everyday features of Windows 11.
- A fully customised UI/UX with **three home screens** (locked — see §3).
- Inspiration: medieval history and the games *Elden Ring*, *The Witcher 3: Wild Hunt* and
  *Red Dead Redemption 2* (mood references only; all shipped art must be original).
- Reference projects the owner shared: SerenityOS, Redox, Asterinas, ToaruOS, Theseus, seL4,
  Haiku, KolibriOS, aeneas, OSDev Wiki/Forum, r/osdev, phil-opp's "Writing an OS in Rust",
  xv6 book, UEFI/ACPI/PCI/USB/VirtIO/NVMe specs, Limine, ACPICA, smoltcp.
- Two planning documents from the owner shaped the plan: a technical stack specification
  (Limine, Clay/PlutoVG/stb_truetype, QOI, uACPI, design tokens, test harness) and an
  independent-OS research & build plan (Rust `no_std`, phased roadmap, safety rules).

## 2. The owner's working rules (always follow)

1. **Discuss everything before implementing.** Propose each milestone, explain options in plain
   language, wait for a clear yes.
2. **Test safely and visibly.** Every run logged; results reported honestly, failures included.
3. **"Don't mess up my PC."** Never touch the host's disks, EFI partition, boot settings or
   firmware. Windows settings/features are changed by the owner, not Claude.
4. **Push every commit to GitHub:** https://github.com/kristoholick/Hydrogen-OS
5. The owner later allowed using this laptop as a real-hardware test machine — still never
   flash firmware or write the internal SSD; USB-stick boots only, prepared by the owner.

Communication style that works: casual, short explanations, tables for comparisons, a clear
recommendation plus the reasons, and one question at the end.

## 3. 🔒 Locked UI requirement — three home screens

| Screen | Owner's description |
|---|---|
| **Left — THEATRE** | Full-Bleed Cinematic Console UI (PS5 / media-first): full-screen atmospheric artwork, whisper-quiet top status info, bold card rows, zero cluttered file trees or tiny buttons |
| **Center — AETHER** (default) | Detached-Chrome Spatial UI (visionOS style): navigation and controls detach into floating pills orbiting the content, with physical depth and lighting — no heavy window frames |
| **Right — THE KEEP** | Medieval history + Elden Ring / Witcher 3 / RDR2 look: ink-brush panels, tarnished-gold frames, parchment maps, radial wheel, inventory grid, skill-tree settings, wax-seal notifications |

The owner explicitly asked that these stay as specified. Never merge, drop or restyle one into
another without the owner's OK. Full text: `docs/DESIGN_SYSTEM.md` ("LOCKED" section).

## 4. Decisions made (full log in `docs/DECISIONS.md`)

| Topic | Decision |
|---|---|
| OS type | Independent kernel (no Linux) |
| Kernel language | Rust `no_std` + minimal x86-64 assembly |
| Architecture | Modular monolithic first; drivers behind traits so they can move to userspace later |
| Bootloader | Limine **11.2.1** (pinned by commit + SHA-256). The spec's "12.x" doesn't exist yet |
| Boot | UEFI only (no legacy BIOS) |
| Target CPU | x86-64 first |
| CPU table formats | **Option A:** `x86_64` crate 0.15.5 for GDT/IDT/TSS/page-table formats only; all policy code is ours; replaceable later (owner chose after a full A-vs-B comparison) |
| UI libraries (Phase 6) | taffy (layout), tiny-skia (vector), fontdue (fonts), QOI (images) |
| Fonts | Cinzel, EB Garamond, JetBrains Mono (all OFL) |
| Art | Original only; game screenshots are mood references |
| Name | Working codename **SIGIL**; final name TBD (repo is called Hydrogen-OS — note OnePlus ships "HydrogenOS") |

## 5. Environment (set up during this session)

- **Host:** Dell Latitude 3400 laptop — Intel Core i5-8265U, Intel UHD 620, 16 GB RAM, Windows 11.
- **WSL2 + Ubuntu 26.04.1 LTS**, installed by the owner (needed a restart and Virtual Machine
  Platform; Linux password prompt shows no characters while typing — that's normal).
- **Installed in Ubuntu (with approval):** build-essential, clang/lld/llvm 21, xorriso, mtools,
  QEMU 10.2.1, OVMF, gdb, socat; Rust `nightly-2026-11-22` with `x86_64-unknown-none`,
  rust-src, llvm-tools. `/dev/kvm` works, so QEMU runs with hardware acceleration.
- Run Linux commands from PowerShell with `.\tools\wsl.ps1 <command>`.

## 6. What was built

### Phase 0 — Planning + Design Lab ✅
- Planning docs: ARCHITECTURE, ROADMAP, DESIGN_SYSTEM, SAFETY, HARDWARE_MATRIX, DECISIONS, CLAUDE.md.
- `design-lab/` — browser prototype of all three home screens with procedural original art,
  radial wheel, parchment file map, skill-tree settings, wax-seal letters and a style tweaker
  (press `T`, then "Copy tokens" to send choices back).

### Phase 1 — First boot ✅
- Rust kernel boots via Limine in QEMU; COM1 serial log; framebuffer boot screen in the Keep
  palette; crimson kernel-panic screen; Makefile + headless QEMU test with screenshots.

### Phase 2 — Kernel foundations ✅
- GDT/TSS with 3 emergency stacks; IDT handles all 32 CPU exceptions.
- Bitmap physical memory manager; own 4-level page tables (2 MiB direct map, per-section kernel
  permissions, NX + write-protect); 64 KiB kernel stack with guard page.
- Size-class kernel heap (`Vec`, `Box`, `String`, `BTreeMap`); interrupt-safe locks.
- Legacy PIC masked; Local APIC timer (xAPIC/x2APIC) at 100 Hz, calibrated against the PIT and
  verified against the RTC (exactly 100 ticks per second in QEMU).
- `crates/sigil-mm` with 13 host unit tests; deliberate fault tests (page fault, stack overflow,
  breakpoint); test script detects triple-fault resets.
- `make test-all` passes. Screenshots in `docs/screenshots/`.

### Lessons from debugging
- QEMU always logs **two CPU resets at power-on** (EIP=0000fff0, paging off) — harmless. A real
  triple fault shows a 64-bit `RIP=` state; the test script checks for exactly that.
- Memory self-tests must reserve their own heap space *before* measuring free frames, because
  heap growth takes frames from the PMM too.
- Unix sockets can't live on `/mnt/c`; QEMU's monitor socket goes in WSL's `/tmp`.

## 6b. Session 2 (2026-11-23) — second dev PC
- The owner is now also working on a **Ryzen 5 5600G desktop** (repo at `C:\Claude code work\Hydrogen-OS`).
  It is a build/QEMU machine only — not approved for real-hardware boots (that is still the Dell).
- Installed its toolchain (owner: "install everything you need"); `tools/wsl.ps1` no longer hardcodes
  the laptop path. Details in `docs/SAFETY.md`.
- `make test-all` passes there with KVM on AMD. The very first boot after a WSL restart once stalled
  inside OVMF before loading the DVD (kernel never started); not reproduced in 9 later runs.
- Captured the Design Lab's three home screens into `docs/screenshots/design-lab-*.png`.
- Lesson: from Git Bash, `/mnt/...` paths passed to `wsl` get rewritten — call `wsl` from PowerShell.

## 6c. Session 2, continued (2026-11-23/24) — Phase 3 started
- Owner approved `docs/PHASE3_PLAN.md` (choices A–F as recommended) and the name **Hydrogen**.
- **3.0 done:** everything renamed SIGIL → Hydrogen (commit `3c7dc5f`).
- **3.1 done:** kernel threads in `kernel/src/sched/` — thread 0 is the boot path; new threads
  get 16 KiB stacks in slots at `0xffff_fd00_0000_0000` (guard page each); context switch is
  ~15 lines of asm; switching is still cooperative (timer takes over in 3.2). Thread stacks are
  not freed yet (3.2 adds the reaper). `make test-all` = 13 unit tests + 7 QEMU boots, all pass.
- Lesson: passing `$?`/`|`/quotes through `tools/wsl.ps1` gets mangled by PowerShell; put
  multi-part commands in a script (e.g. `build/run-test-all.sh`, not committed) and run that.

## 7. What's next

1. **Phase 2.5 — first boot on the real laptop.** Owner writes the ISO to a USB stick (Rufus) and
   disables Secure Boot. No disk drivers exist yet, so the SSD can't be touched.
2. **Phase 3 — multitasking:** threads, context switching, scheduler, ring-3 processes,
   `syscall`/`sysret`, ELF loader. Plan it with the owner first.
3. Later: hardware (PCIe, ACPI, VirtIO, input, storage) → file system + shell → **compositor and
   the three home screens (Phase 6)** → networking → USB/audio/NVMe → security.

## 8. Still open (ask the owner)

_Answered 2026-11-23: name = **Hydrogen**; Windows features = **all** (map in ARCHITECTURE.md §8);
chose Phase 3 over the laptop USB boot for now. Phase 3 plan proposed in `docs/PHASE3_PLAN.md`._

- Visual choices from the Design Lab tweaker (panel style, header font, metal accent, selection
  colour, backdrop mood) to freeze into the design tokens.
- Real-hardware USB boot on the Dell laptop (Phase 2.5) — still to do when on the laptop.
