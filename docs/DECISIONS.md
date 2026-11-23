# Decision log

| # | Date | Decision | Why |
|---|---|---|---|
| 1 | 2026-11-22 | Independent kernel, not a Linux distro | Owner wants a real OS from scratch |
| 2 | 2026-11-22 | Kernel in Rust `no_std` + minimal asm | Memory safety for a large driver surface; Redox/Asterinas/Theseus prove it |
| 3 | 2026-11-22 | Limine + UEFI, x86-64 first | Focus on the kernel, not the bootloader |
| 4 | 2026-11-22 | Modular monolithic, drivers behind traits | Easy start; can move drivers to userspace later |
| 5 | 2026-11-22 | UI: taffy + tiny-skia + fontdue + QOI | Rust equivalents of Clay/PlutoVG/stb_truetype |
| 6 | 2026-11-22 | Three realms on one app model | Apps written once, rendered three ways |
| 7 | 2026-11-22 | Keep art = owner's references: medieval history + Elden Ring / Witcher 3 / RDR2 look | Owner's direction. Original assets only |
| 8 | 2026-11-22 | Design Lab in the browser before OS UI code | Find the style in days, not months |
| 9 | 2026-11-22 | Working codename "SIGIL" | Placeholder until the owner picks a name |
| 10 | 2026-11-22 | Code lives at github.com/kristoholick/Hydrogen-OS; push after every commit | Owner's request |
| 11 | 2026-11-22 | Limine **11.2.1** (commit `deba356b`), verified by commit + SHA-256 in the Makefile | Latest release; the spec's "12.x" does not exist yet |
| 12 | 2026-11-22 | `limine` crate 0.6.5 (base revision 6), Rust `nightly-2026-11-22` | Current versions; crate needs nightly |
| 13 | 2026-11-22 | UEFI-only boot ISO (no legacy BIOS) | Target PCs are UEFI; simpler, fewer moving parts |
| 14 | 2026-11-22 | `font8x8` bitmap font for early text | Placeholder until fontdue + real fonts in Phase 6 |
| 15 | 2026-11-22 | 🔒 Three home screens locked: left PS5-style Theatre, center visionOS-style Aether, right medieval Keep | Owner's explicit requirement; changes need the owner's OK |
| 16 | 2026-11-22 | Option A: `x86_64` crate 0.15.5 for CPU table formats only (GDT/IDT/TSS/page-table entries, registers) | Fewer silent-reset bugs, faster progress; contained in `arch/x86_64/`, replaceable later. All policy code is ours |
| 17 | 2026-11-22 | Memory algorithms in a separate `sigil-mm` crate (renamed `hydrogen-mm` in step 3.0) | Unit-testable on the host |
| 18 | 2026-11-22 | Owner's laptop may be used as a real-hardware test machine | Owner's permission. Still never: firmware flashing, writing the internal SSD |
| 19 | 2026-11-23 | **OS name: Hydrogen** (replaces codename SIGIL) | Owner's choice. Note: OnePlus once shipped an Android skin called "HydrogenOS" — fine for a personal project, worth a check before any public release |
| 20 | 2026-11-23 | Long-term goal: equivalents of **all** everyday Windows 11 features | Owner's request. Built in phase order (see ARCHITECTURE.md §8); running Windows `.exe` files stays out of scope |

## Open questions for the owner
- Visual choices from the Design Lab
