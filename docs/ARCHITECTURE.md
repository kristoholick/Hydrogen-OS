# Architecture

## 1. Layers
```
┌──────────────────────────────────────────────────────────────┐
│ REALMS (shells)   Theatre (left) · Aether (center) · Keep (right) │
│ APPS              written once, rendered by any realm         │
├──────────────────────────────────────────────────────────────┤
│ USERSPACE         compositor · libsigil-ui · init · services  │
│                   shell · network manager · (later) drivers   │
├────────────────────────── syscalls / IPC ────────────────────┤
│ KERNEL (ring 0)   scheduler · processes · IPC · VMM/PMM/heap  │
│                   VFS · net (smoltcp) · driver framework      │
├──────────────────────────────────────────────────────────────┤
│ HAL               x86_64: GDT/IDT/APIC/HPET/TSC · paging      │
│                   PCIe · ACPI (uACPI) · MMIO · DMA            │
├──────────────────────────────────────────────────────────────┤
│ HARDWARE          QEMU virtual devices first, then real PCs   │
└──────────────────────────────────────────────────────────────┘
```

## 2. Kernel
- **Language:** Rust `no_std`, target `x86_64-unknown-none`, nightly pinned via `rust-toolchain.toml`.
- **Style:** modular monolithic. Each driver sits behind a trait (`BlockDevice`, `NetDevice`,
  `InputDevice`, `Display`) so it can later move to userspace (Redox model) without rewriting.
- **Boot:** UEFI → Limine → kernel ELF in the higher half. Limine hands over memory map,
  framebuffer (GOP), RSDP, SMP cores, and modules (initrd, fonts, wallpapers).
- **Memory:** bitmap/buddy physical allocator → 4-level paging → slab heap (`GlobalAlloc`).
- **Unsafe policy:** all `unsafe` lives in `arch/` and `drivers/*/hw.rs`; each block has a
  `// SAFETY:` comment.

## 3. Directory plan (created as phases need it, not all at once)
```
kernel/src/{arch/x86_64, mm, interrupts, sched, proc, syscall, ipc, drivers, fs, net, main.rs}
userspace/{libsigil-ui, compositor, init, shell, apps/*}
assets/{fonts, wallpapers, sigils, sfx}
tools/{build, run-qemu, test-visual-boot}
design-lab/   (browser prototype; never shipped in the OS)
```

## 4. Graphics and compositor (userspace)
- Draws to the GOP linear framebuffer (later VirtIO-GPU).
- CPU compositor with **dirty-rectangle** tracking and SIMD (SSE2/AVX2) blits.
- **Live blur is too expensive on CPU.** Each wallpaper is pre-baked into three buffers
  at load: sharp, smoked-glass (blurred + darkened), and monochrome (for the Keep's pause/ink look).
  Glass panels sample the pre-blurred buffer — looks like live blur over a static backdrop.
- Libraries: **taffy** (flex/grid layout), **tiny-skia** (anti-aliased vector), **fontdue**
  (glyph raster + cache), **QOI** decoder.
- Target: 60 FPS at 1920×1080 on the host laptop's CPU class.

## 5. App model — the key idea
Apps are **not** written three times. An app publishes:
- a **content tree** (lists, grids, media, text, forms)
- a set of **actions** (open, delete, share, …) with semantic roles
- optional **hero art** (used by Theatre) and a **sigil** (icon, used by the Keep)

Each realm is a renderer for that same description:
| | Theatre | Aether | Keep |
|---|---|---|---|
| Chrome | none; quiet top status line | detached floating pills | ink-brush panel + gold hairline frame |
| Actions | button prompts at bottom (gamepad glyphs) | pill toolbar orbiting the window | radial wheel + footer prompts |
| Lists | big card rows | glass list | parchment/ledger list |

## 6. Unified input action bus
Keyboard, mouse, touchpad and gamepad → one stream of actions:
`NAV_UP/DOWN/LEFT/RIGHT, CONFIRM, BACK, REALM_PREV, REALM_NEXT, HOLD_RADIAL, INSPECT, MENU`.
Pointer events pass through for mouse-first realms (Aether).

## 7. Hardware strategy
1. QEMU virtual devices (VirtIO blk/net/gpu/input, PS/2, AHCI, NVMe, xHCI, e1000, HDA).
2. Owner's laptop (see HARDWARE_MATRIX.md). Hard parts: I2C-HID touchpad, Intel Wi-Fi, UHD 620 acceleration.
3. Common devices on other PCs.
4. Long term: a LinuxKPI-style compatibility shim (as FreeBSD does) to reuse Linux GPU/Wi-Fi drivers.

## 8. Windows 11 feature parity
Owner's goal (2026-11-23): **equivalents of all everyday Windows 11 features.** We build our own
versions; running native Windows `.exe` software is out of scope (Wine-scale project).
Order follows the roadmap — each feature needs the layers under it first:

| Windows 11 feature | Hydrogen equivalent | Phase |
|---|---|---|
| Multitasking, Task Manager | scheduler + processes; **Astrolabe** (system monitor) | 3 → 6 |
| Device Manager, drivers, plug-and-play | PCIe/ACPI device tree, driver framework | 4, 8 |
| File Explorer, search, zip | VFS + **The Archive** (map in the Keep, lists elsewhere) | 5 → 6 |
| Terminal / PowerShell | **The Forge** + shell | 5 |
| Desktop, Start menu, taskbar, Snap layouts, virtual desktops | the three realms (Theatre / Aether / Keep), radial wheel, Satchel | 6 |
| Notifications, Action Center, quick settings | wax-seal letters (Keep), pills (Aether), status line (Theatre) | 6 |
| Settings app | **Codex** (skill tree in the Keep) | 6 |
| Lock screen, sign-in, user accounts | session manager + per-user home | 6, 9 |
| Wi-Fi, Ethernet, Bluetooth | network stack (smoltcp); Wi-Fi/BT drivers are long-term | 7, 8+ |
| Web browser (Edge) | port of an existing engine (later decision) | 7+ |
| Sound, volume mixer | HDA audio + mixer; **The Bard** | 8 |
| USB, printers, game controllers | xHCI + HID; printing much later | 8+ |
| Photos, Media Player, Clock, Notepad, Mail | Lanternlight, Moving Pictures, Hourglass, Chronicle, Ravenpost | 6+ |
| Microsoft Store | **The Market** (package manager) | 6+ |
| Windows Update | signed updates | 9 |
| Windows Security, firewall, BitLocker | permissions, sandboxing, firewall, disk encryption | 9 |
| Sleep / hibernate, battery | ACPI power management | 8+ |
| OneDrive, Copilot, Phone Link | cloud/AI features — much later, after networking | 9+ |

