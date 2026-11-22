# Hardware compatibility matrix

Levels: **Detected** (IDs read) → **Initialized** → **Working** (tested end-to-end) → **Accelerated**.
Nothing is marked above "Not started" until it has been tested.

## Virtual (QEMU)
| Device | Status |
|---|---|
| UEFI GOP framebuffer | Not started |
| COM1 serial | Not started |
| PS/2 keyboard / mouse | Not started |
| VirtIO blk / net / input / gpu | Not started |
| AHCI / NVMe (emulated) | Not started |
| xHCI USB | Not started |
| e1000 | Not started |
| Intel HDA | Not started |

## Owner's laptop — Dell Latitude 3400 (from read-only check, 2026-11-22)
| Component | Known | Status | Notes |
|---|---|---|---|
| CPU | Intel Core i5-8265U (Whiskey Lake, 4C/8T) | Not started | x86-64, AVX2 available |
| GPU | Intel UHD 620 | Not started | GOP framebuffer first; acceleration is long-term |
| RAM | 16 GB | Not started | |
| Storage | TBD (likely NVMe or SATA SSD) | Not started | **read-only drivers only** on bare metal |
| Touchpad | TBD (likely I2C-HID) | Not started | hard: needs I2C + ACPI + HID |
| Keyboard | TBD (likely PS/2-compatible via EC) | Not started | |
| Wi-Fi | TBD | Not started | very hard: chipset firmware + 802.11 stack |
| Ethernet | TBD | Not started | |
| Audio | TBD (likely Intel HDA + Realtek codec) | Not started | |
