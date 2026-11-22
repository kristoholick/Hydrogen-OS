#!/usr/bin/env bash
# Read-only report of the WSL build environment. Changes nothing.
. /etc/os-release
echo "user:    $(whoami)"
echo "distro:  $PRETTY_NAME"
echo "kernel:  $(uname -r)"
echo "cpus:    $(nproc)"
echo "memory:  $(free -h | awk '/Mem:/ {print $2 " total, " $7 " available"}')"
echo "home fs: $(df -h ~ | awk 'NR==2 {print $4 " free of " $2}')"
echo "--- tools ---"
for t in gcc clang ld.lld make git curl xorriso mtools qemu-system-x86_64 rustup rustc cargo python3 socat; do
  printf '%-20s %s\n' "$t" "$(command -v "$t" || echo missing)"
done
echo "--- ovmf (UEFI firmware for QEMU) ---"
ls /usr/share/ovmf 2>/dev/null || echo "missing"
echo "--- kvm (hardware acceleration for QEMU) ---"
if [ -e /dev/kvm ]; then ls -l /dev/kvm; else echo "no /dev/kvm"; fi
