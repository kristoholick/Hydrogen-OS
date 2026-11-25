#!/usr/bin/env python3
"""usage: tools/check-pci.py <serial.log> <monitor.log>

Compares the devices the kernel found with what QEMU itself says it has (`info pci`)."""
import re
import sys

log = open(sys.argv[1], errors="replace").read()
mon = open(sys.argv[2], errors="replace").read()

kernel = sorted(re.findall(r"\[hydrogen\] pci ([0-9a-f]{2}:[0-9a-f]{2}\.\d) ([0-9a-f]{4}:[0-9a-f]{4})", log))
qemu = sorted(
    (f"{int(b):02x}:{int(d):02x}.{f}", vid)
    for b, d, f, vid in re.findall(
        r"Bus\s+(\d+), device\s+(\d+), function (\d+):\s+[^\n]*?PCI (?:device|bridge) ([0-9a-f]{4}:[0-9a-f]{4})", mon)
)
for k in sorted(set(kernel) | set(qemu)):
    print("[test]  ", *k, "" if k in kernel and k in qemu else ("<- kernel only" if k in kernel else "<- QEMU only"))
if qemu and kernel == qemu:
    print(f"[test] ok:   kernel and QEMU agree on all {len(qemu)} devices")
else:
    print(f"[test] FAIL: kernel found {len(kernel)} devices, QEMU has {len(qemu)}, or they differ")
    sys.exit(1)
