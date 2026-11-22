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

## Open questions for the owner
- Final OS name
- Top-10 Windows 11 features to rebuild
- Visual choices from the Design Lab
