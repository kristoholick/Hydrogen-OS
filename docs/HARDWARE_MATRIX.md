# Hardware compatibility matrix

Levels: **Detected** (IDs read) → **Initialized** → **Working** (tested end-to-end) → **Accelerated**.
Nothing is marked above "Not started" until it has been tested.

## Virtual (QEMU)
| Device | Status |
|---|---|
| UEFI GOP framebuffer | **Working** — 1280×800×32 via Limine, QEMU std VGA + OVMF (Phase 1, 2026-11-22) |
| COM1 serial | **Working** — 16550 output, 38400 8N1 (Phase 1, 2026-11-22) |
| Local APIC + timer | **Working** — xAPIC mode, periodic 100 Hz, PIT-calibrated, RTC-verified (Phase 2) |
| 8259 PIC | **Working** — remapped and masked (Phase 2) |
| PIT channel 2 | **Working** — one-shot calibration only (Phase 2) |
| CPU protections NX, SMEP, SMAP, UMIP | **Working** — KVM on Ryzen 5 5600G and QEMU TCG; SMAP/SMEP proven by fault tests (Phase 3.5) |
| CMOS RTC | **Working** — seconds register read (Phase 2) |
| PS/2 keyboard / mouse | Not started |
| VirtIO blk / net / input / gpu | Not started |
| AHCI / NVMe (emulated) | Not started |
| xHCI USB | Not started |
| e1000 | Not started |
| Intel HDA | Not started |

## Target PC — owner's desktop (from read-only check, 2026-11-25)
Nothing here has been tested on the real machine yet.
| Component | Known | Status | Notes |
|---|---|---|---|
| Board / firmware | ASRock B450 Pro4 R2.0, AMI UEFI P5.00, Secure Boot off | Not started | Limine boots UEFI |
| CPU | AMD Ryzen 5 5600G (6C/12T) | Not started | Phase 3 suite passes on it under KVM |
| GPU | AMD Radeon (integrated), 1920x1080 | Not started | firmware framebuffer first; acceleration is long-term |
| RAM | 16 GB | Not started | |
| Disk | Crucial P3 500 GB, NVMe — the only disk, holds Windows | Not started | **read-only driver only**; see SAFETY.md 4b |
| SATA | AHCI controllers (AMD + ASMedia), nothing attached | Not started | a second SSD could go here |
| USB | AMD xHCI controllers | Not started | needed first: keyboard and mouse are USB |
| Keyboard / mouse | USB | Not started | needs xHCI + USB HID |
| Serial | COM1 reported by the firmware | Not started | may need a header cable |
| Ethernet | Realtek RTL8168 family | Not started | realistic for Phase 7/8 |
| Wi-Fi / Bluetooth | AIC8800D80 (USB) | Not started | very hard: no public documentation known to us |
| Audio | Realtek ALC892 codec (HDA) | Not started | |

## Former laptop — Dell Latitude 3400 (from read-only check, 2026-11-22; no longer available)
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
