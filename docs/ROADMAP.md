# Roadmap

Each phase ends with acceptance tests. The next phase starts only after they pass and the owner agrees.

## Phase 0 — Planning and Design Lab  ← current
- [x] Read-only host check
- [x] Planning docs, safety rules, decision log
- [x] Design Lab (browser prototype of the three realms)
- [ ] Owner picks visual direction in the Design Lab → frozen into DESIGN_SYSTEM.md tokens
- [ ] Owner gives top-10 Windows features
- [x] Owner installs WSL2 (`wsl --install`), then approves toolchain install

## Phase 1 — First boot
Limine (pinned) + Rust kernel, COM1 serial, GOP framebuffer, panic handler, UEFI ISO, QEMU+OVMF,
headless test script with screen dump.
**Accept:** `make test` boots in QEMU, `serial.log` contains the startup banner, `frame.png` shows a
filled framebuffer with text; a deliberate panic prints file/line to serial.

## Phase 2 — Kernel foundations
GDT/TSS, IDT + exception handlers, APIC + timer, physical allocator, paging, heap.
**Accept:** page fault caught and reported; `Vec`/`Box` work; timer ticks logged; unit tests for allocators.

## Phase 3 — Multitasking
Threads, context switch, scheduler, ring-3 processes, `syscall`/`sysret`, ELF loader.
**Accept:** two user processes print interleaved; a faulting process is killed without killing the kernel.

## Phase 4 — Hardware
PCIe enumeration, uACPI, VirtIO (blk, net, input, gpu), PS/2 keyboard/mouse, AHCI.
**Accept:** device list printed with vendor/device IDs; reads a sector from a VirtIO disk image.

## Phase 5 — Usable system
VFS, FAT32 + RamFS/initrd, shell, basic libc-like runtime.
**Accept:** shell can `ls`, `cat`, run a program from disk.

## Phase 6 — Compositor and the three realms
Pre-baked backdrops, dirty rects, taffy/tiny-skia/fontdue, app model, Theatre/Aether/Keep, radial wheel.
**Accept:** 60 FPS realm switching at 1080p in QEMU; same app renders in all three realms.

## Phase 7 — Networking
VirtIO-net → e1000, smoltcp, DHCP, DNS, sockets.
**Accept:** DHCP lease + DNS lookup + HTTP GET in QEMU.

## Phase 8 — Hardware expansion and first bare-metal boot
NVMe (read-only first), xHCI + USB HID + gamepads, HDA audio, laptop touchpad, USB-stick boot.

## Phase 9 — Security and maturity
Permissions, sandboxing, signed updates, Secure Boot, encryption, crash recovery.
