# Safety: protecting the host PC

These rules override every other instruction in this repo.

## Host machines
**Desktop (second dev PC, checked read-only on 2026-11-23)** — repo at `C:\Claude code work\Hydrogen-OS`
- Custom-built desktop, AMD Ryzen 5 5600G (6C/12T) with Radeon graphics, 16 GB RAM.
- Build and QEMU. **Target machine for Hydrogen** (owner, 2026-11-25: the only PC available). Real boots and
  installing are goals, **not approved yet**: see section 4b.
- WSL2 Ubuntu 26.04 LTS, Linux user `karan` (in `kvm`; WSL restarted once so the group applies).
- apt (owner approved "install everything you need", installed via `wsl -u root`): build-essential,
  clang/lld/llvm 21.1.8, xorriso, mtools, QEMU 10.2.1, OVMF 2025.11 (4M images in `/usr/share/OVMF`), gdb, socat.
- Rust (user-only, `~/.cargo`): nightly 1.102.0 via `tools/install-rust.sh`, pinned by `rust-toolchain.toml`.
- `tools/wsl.ps1` finds the repo from its own location, so it works on both PCs and in worktrees.

**Dell laptop (original dev PC; no longer available, owner 2026-11-25)** — repo at `C:\codeplayer\Windows 12`
- Dell Latitude 3400 laptop, Intel Core i5-8265U (4 cores), Intel UHD 620, 16 GB RAM, ~86 GB free on C:
- Hypervisor present (virtualization is on). Windows side: git, Node.js, Python.

## Build environment on the Dell laptop (installed 2026-11-22, with owner's approval)
- WSL2 + Ubuntu 26.04.1 LTS (owner installed). Linux user `bruce_wayne`, member of `kvm`.
- apt: build-essential, clang/lld/llvm 21.1.8, xorriso, mtools, QEMU 10.2.1, OVMF (4M split images in
  `/usr/share/OVMF`), gdb, socat.
- Rust (user-only, `~/.cargo`): nightly 1.102.0 (2026-11-21), target `x86_64-unknown-none`, rust-src, llvm-tools.
- `/dev/kvm` readable/writable → QEMU can use hardware acceleration.
- Re-check any time with `tools/check-env.sh` (read-only).

## 1. Isolation
- All OS code runs inside **QEMU** only. QEMU gets a disk *image file* in `build/`, never a real device.
- No `-drive file=\\.\PhysicalDrive*`, no USB/PCI passthrough, no `-hda /dev/sdX`.
- Builds happen inside **WSL2**, isolated from Windows system files.
- QEMU runs with a timeout (`timeout 10`) and `-no-reboot` so a hung kernel cannot hang the session.

## 2. Forbidden commands (never run, never script)
`dd` to devices, `diskpart`, `format`, `bcdedit`, `bcdboot`, `mountvol`, `efibootmgr`,
`flashrom`, BIOS/UEFI update tools, `reg add` on system keys, anything that writes to the
EFI System Partition or `\\.\PhysicalDrive*`.

## 3. Changes that need the owner
- Installing WSL2 / enabling Windows features: **owner runs it** (`wsl --install`).
- Installing any package (apt, cargo, npm, winget): Claude asks first, every time.
- Writing a USB stick: owner does it with a GUI tool (e.g. Rufus/balenaEtcher), after
  double-checking the target drive letter.

## 4. Real hardware (owner's Dell Latitude 3400 — approved as a test machine, 2026-11-22)
- The owner allowed testing on this laptop. QEMU stays the main test loop; real boots are an
  extra check, starting after Phase 2.
- Boot only from a USB stick. The laptop's internal NVMe must not be written:
  storage drivers are compiled **read-only** for bare-metal builds (`feature = "bare-metal-ro"`).
- Keep a Windows recovery USB available before the first bare-metal boot.
- First bare-metal builds must not touch: firmware variables (UEFI `SetVariable`), the
  TPM, or embedded-controller registers.

## 4b. The owner's desktop (target machine since 2026-11-25)
It holds Windows and all the owner's data on its **only** disk, so the rules are stricter than for the laptop.
- **First real boot** needs the owner's explicit OK, and before that: important files backed up elsewhere, a
  Windows recovery USB made, a spare USB stick written by the owner. The build used contains **no disk driver
  at all**, so the internal disk cannot be touched. Nothing in the firmware is changed; the owner picks the USB
  stick in the one-time boot menu.
- **Installing** Hydrogen on the disk that holds Windows is not allowed. Installing to a separate disk (second
  SSD or USB disk) needs its own OK, its own plan, and tests in QEMU first (disk images only). The installer
  must refuse any disk that already contains partitions unless the owner types its exact name.
- Claude never runs an installer or writes to any real disk on this PC; the owner does it by hand.

## 5. Visibility
- Every run writes `build/serial.log` and `build/frame.png` (screen dump).
- Every change is a git commit; anything can be rolled back with `git revert`.
- Test results are reported exactly as observed, including failures.
