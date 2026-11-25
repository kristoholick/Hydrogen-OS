# Phase 4 plan — Hardware for Hydrogen (PROPOSED 2026-11-25, revised for the target PC, waiting for the owner's yes)

**Goal (from ROADMAP.md):** find the devices in the machine and talk to the first ones.
**Accept:** device list printed with vendor/device IDs; reads a sector from a VirtIO disk image.

In plain words: today Hydrogen can compute and print, but it is blind and deaf. After Phase 4 it
knows which devices are plugged in, hears the keyboard and mouse, and can read a (virtual) disk.

## 1. Steps (each = its own test, commit and push)

| Step | What gets built | New test | Pass means |
|---|---|---|---|
| **4.1 Device list** | Read the ACPI tables (own small parser, host unit-tested, untrusted input), find and list all PCIe devices | `make test-pci` | The list matches the devices QEMU was given |
| **4.2 Log on the screen + steady clock** | The kernel log also on the screen (real PCs have no serial window); time kept from the CPU's cycle counter, so lost ticks don't matter | `make test-screen-log`, `make test-clock` | Screen shows the last log lines; the clock stays right although ticks are dropped on purpose |
| **4.3 USB keyboard** | xHCI USB controller, USB HID keyboard (the target PC has no other keyboard) | `make test-keyboard` | The test types keys through QEMU; the kernel logs exactly those |
| **4.4 USB mouse** | USB HID mouse | `make test-mouse` | Movements and clicks sent by QEMU arrive with the right values |
| **4.5 Disks, read-only** | VirtIO disk and NVMe (the target PC's disk type), bounds-checked; **no write code at all** | `make test-disk` | Reads known data from test images; damaged or out-of-range requests return errors |
| **4.6 Disk, writing** | VirtIO scratch image only, in a separate QEMU-only build | `make test-disk-write` | Data written is read back after a reboot; all other builds contain no write code |
| **4.7 Round-up** | Device and driver overview, `phase 4 boot complete`, full suite, deliberate breakages | `make test-all` | Everything from Phases 1–4 passes |

## 2. Choices — with my recommendation

| # | Question | Recommendation |
|---|---|---|
| A | ACPI: own small table reader, or the uACPI library (C code) now? | **Own reader now.** Only tables are needed. uACPI (power button, batteries, laptop devices) comes with Phase 8. |
| B | VirtIO: write our own drivers, or use the `virtio-drivers` crate? | **Our own**, like the ELF loader: small and security-critical. |
| C | The roadmap also lists VirtIO network, graphics and AHCI here | **Move each to the phase that uses it:** network → 7, graphics → 6, AHCI → 8. They can't be properly tested earlier. |
| D | Other kinds of devices (owner's goal) | New CPU-specific code stays in `arch/`; PCIe, VirtIO and disk code stay CPU-independent, so an ARM port can reuse them. |
| E | A second virtual machine (VirtualBox) | See section 3. |

## 3. VirtualBox
Possible in principle, not tried yet. Hydrogen can't be *installed* there: it has no disk driver and no
installer. It would boot from the ISO each time, like a live CD.

- **Good for:** a second opinion. Bugs that QEMU hides often show up on a different virtual machine.
- **Not a replacement for QEMU:** the automatic tests depend on QEMU.
- **Needs the owner:** installing VirtualBox adds drivers and a network adapter to Windows, so the owner
  does it, and `docs/SAFETY.md` (QEMU only) must be changed with the owner's OK.
- **Safe settings:** no hard disk, no network, no USB, no shared folders; only the ISO and a serial log file.
- **Expect:** slow, because WSL2 already uses the CPU's virtualization.
- **Best moment:** after 4.3, when there is a keyboard to try.

## 4. Safety
- Disks are image files in `build/`, never real ones. Nothing here touches Windows' disks or boot settings.
- Real-hardware builds (`bare-metal-ro`) contain no disk write code.

## 5. Not in Phase 4
Files and folders (Phase 5), network (Phase 7), sound (Phase 8), other USB devices (Phase 8), graphics
acceleration (long term), installing to a real disk (Phase 8, own plan and OK).
