#!/usr/bin/env bash
# Headless boot test.
# Boots an ISO in QEMU with no window, waits until the kernel reports it is done (or panics,
# or the time limit hits), takes a screenshot, stops QEMU and checks the serial log.
# Safety: QEMU only gets the ISO file and a copy of the UEFI vars; output goes to $OUT only.
#
# usage: tools/test-boot.sh <iso> <out-dir> <expected serial text>
set -uo pipefail

ISO=$1
OUT=$2
EXPECT=$3
LIMIT=${TIMEOUT:-20}

ACCEL="-accel tcg -cpu max"
[ -r /dev/kvm ] && [ -w /dev/kvm ] && ACCEL="-accel kvm -cpu host"

rm -rf "$OUT" && mkdir -p "$OUT"
cp /usr/share/OVMF/OVMF_VARS_4M.fd "$OUT/ovmf-vars.fd"
SOCK=$(mktemp -u /tmp/sigil-qemu-XXXXXX.sock)   # unix sockets cannot live on /mnt/c

echo "[test] booting $ISO ($ACCEL), limit ${LIMIT}s"
timeout $((LIMIT + 10)) qemu-system-x86_64 $ACCEL -M q35 -m 512M -no-reboot \
  -drive if=pflash,unit=0,format=raw,file=/usr/share/OVMF/OVMF_CODE_4M.fd,readonly=on \
  -drive if=pflash,unit=1,format=raw,file="$OUT/ovmf-vars.fd" \
  -cdrom "$ISO" \
  -serial file:"$OUT/serial.log" \
  -display none \
  -monitor unix:"$SOCK",server,nowait \
  -d cpu_reset,guest_errors -D "$OUT/qemu.log" &
QPID=$!

# wait for the kernel to finish, panic, or the time limit
status="timeout"
for ((i = 0; i < LIMIT * 10; i++)); do
  if grep -qE "boot complete|PANIC" "$OUT/serial.log" 2>/dev/null; then status="kernel-finished"; break; fi
  if ! kill -0 $QPID 2>/dev/null; then status="qemu-exited"; break; fi
  sleep 0.1
done
sleep 0.5   # let the last pixels land

if kill -0 $QPID 2>/dev/null; then
  echo "screendump $OUT/frame.ppm" | socat - UNIX-CONNECT:"$SOCK" >/dev/null 2>&1
  sleep 1
  echo "quit" | socat - UNIX-CONNECT:"$SOCK" >/dev/null 2>&1
fi
wait $QPID 2>/dev/null
rm -f "$SOCK"

[ -f "$OUT/frame.ppm" ] && python3 "$(dirname "$0")/ppm2png.py" "$OUT/frame.ppm" "$OUT/frame.png" && rm -f "$OUT/frame.ppm"

echo "[test] stop reason: $status"
echo "----- serial.log -----"
tr -d '\r' < "$OUT/serial.log" 2>/dev/null || echo "(no serial output)"
echo "----------------------"
[ -s "$OUT/qemu.log" ] && { echo "----- qemu.log (resets / guest errors) -----"; head -20 "$OUT/qemu.log"; }

if grep -q "$EXPECT" "$OUT/serial.log" 2>/dev/null; then
  echo "[test] PASS: found \"$EXPECT\"  (screenshot: $OUT/frame.png)"
  exit 0
else
  echo "[test] FAIL: \"$EXPECT\" not found in serial log"
  exit 1
fi
