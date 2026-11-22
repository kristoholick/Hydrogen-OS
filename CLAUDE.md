# Project rules for Claude Code

This repo is an independent, from-scratch operating system (working codename **SIGIL**, final name TBD).
It is NOT a Linux distribution and must never embed the Linux kernel.

## The owner's three hard rules
1. **Discuss before implementing.** Propose each milestone and wait for a yes before writing its code.
2. **Test safely and visibly.** Every build/run is logged; every result reported honestly (no invented passes).
3. **Never harm the host PC.** See `docs/SAFETY.md` — those rules override everything else.

## Safety invariants (summary of docs/SAFETY.md)
- The OS runs ONLY inside QEMU, against disk-image files inside this repo. No disk/USB/PCI passthrough.
- Never run `dd`, `diskpart`, `bcdedit`, `mountvol`, `format`, firmware tools, or anything touching `\\.\PhysicalDrive*`, the EFI partition, or boot config.
- Never change Windows features/settings. Installing WSL2 or enabling Windows features is done by the owner, not Claude.
- Ask before installing any package or toolchain (host or WSL).
- Real-hardware tests: USB stick only, prepared by the owner, storage drivers built read-only.

## Tech decisions (see docs/DECISIONS.md)
- Kernel: Rust `no_std` (+ minimal x86-64 asm), target `x86_64-unknown-none`, modular monolithic.
- Boot: Limine (pinned version), UEFI. Emulator: QEMU + OVMF. Debug: serial log + GDB.
- UI stack: taffy (layout), tiny-skia (vector), fontdue (fonts), QOI (images).
- Host: WSL2 Ubuntu 26.04 (toolchain installed 2026-11-22; details in docs/SAFETY.md).
- Run Linux commands from Windows as: `wsl -d Ubuntu --cd "/mnt/c/codeplayer/Windows 12" -- bash -lc '...'`
  (source `~/.cargo/env` first for Rust).

## Working rules
- One milestone at a time; each has acceptance tests in `docs/ROADMAP.md`.
- Every `unsafe` block gets a `// SAFETY:` comment stating its preconditions.
- Never mark hardware "supported" in `docs/HARDWARE_MATRIX.md` without a real test.
- Art: original assets only. Game screenshots in the brief are mood references, never shipped.
- Commit after each verified step, then push to `origin` (github.com/kristoholick/Hydrogen-OS) — owner's request.
- Build/test from PowerShell: `.\tools\wsl.ps1 make test` (also `make test-panic`, `make run`, `make debug`).
- Limine is pinned in the Makefile (tag + commit + SHA-256). Never bump it silently.
