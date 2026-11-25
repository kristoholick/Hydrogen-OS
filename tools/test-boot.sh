#!/usr/bin/env bash
# Headless boot test.
# Boots an ISO in QEMU with no window, waits until the kernel reports it is done (or panics,
# or the time limit hits), takes a screenshot, stops QEMU and checks the serial log.
# Safety: QEMU only gets the ISO file and a copy of the UEFI vars; output goes to $OUT only.
#
# usage: tools/test-boot.sh <iso> <out-dir> <expected text> [more expected text ...]
# Passes only if EVERY expected text appears in the serial log and the CPU never reset
# after power-on (a second reset would mean a triple fault).
set -uo pipefail

ISO=$1
OUT=$2
shift 2
EXPECTS=("$@")
LIMIT=${TIMEOUT:-40}
# Optional: MACHINE = QEMU machine type (default q35).
# Optional: QEMU_EXTRA = more QEMU arguments (devices), MONITOR = QEMU monitor commands to run
# once the kernel is done, separated by ";" (their output goes to $OUT/monitor.log).

ACCEL="-accel tcg -cpu max"
[ -r /dev/kvm ] && [ -w /dev/kvm ] && ACCEL="-accel kvm -cpu host"

rm -rf "$OUT" && mkdir -p "$OUT"
cp /usr/share/OVMF/OVMF_VARS_4M.fd "$OUT/ovmf-vars.fd"
# The log goes to /tmp and is copied afterwards: writing it to /mnt/c byte by byte is slow (0.4 to
# 1.5 ms per byte, depending on what else Windows is doing) and made results depend on that.
LOG=$(mktemp /tmp/hydrogen-serial-XXXXXX.log)
SOCK=$(mktemp -u /tmp/hydrogen-qemu-XXXXXX.sock)   # unix sockets cannot live on /mnt/c

echo "[test] booting $ISO ($ACCEL), limit ${LIMIT}s"
timeout $((LIMIT + 10)) qemu-system-x86_64 $ACCEL -M ${MACHINE:-q35} -m 512M -no-reboot \
  -drive if=pflash,unit=0,format=raw,file=/usr/share/OVMF/OVMF_CODE_4M.fd,readonly=on \
  -drive if=pflash,unit=1,format=raw,file="$OUT/ovmf-vars.fd" \
  -cdrom "$ISO" \
  -serial file:"$LOG" \
  -display none \
  -monitor unix:"$SOCK",server,nowait \
  -d cpu_reset,guest_errors -D "$OUT/qemu.log" ${QEMU_EXTRA:-} &
QPID=$!

# wait for the kernel to finish, panic, or the time limit
status="timeout"
for ((i = 0; i < LIMIT * 10; i++)); do
  if grep -qE "boot complete|PANIC" "$LOG" 2>/dev/null; then status="kernel-finished"; break; fi
  if ! kill -0 $QPID 2>/dev/null; then status="qemu-exited"; break; fi
  sleep 0.1
done
sleep 0.5   # let the last pixels land

if kill -0 $QPID 2>/dev/null; then
  : > "$OUT/monitor.log"
  IFS=';' read -ra commands <<< "${MONITOR:-}"
  for c in "${commands[@]}"; do (echo "$c"; sleep 0.3) | socat - UNIX-CONNECT:"$SOCK" >> "$OUT/monitor.log" 2>&1; done
  echo "screendump $OUT/frame.ppm" | socat - UNIX-CONNECT:"$SOCK" >/dev/null 2>&1
  sleep 1
  echo "quit" | socat - UNIX-CONNECT:"$SOCK" >/dev/null 2>&1
fi
wait $QPID 2>/dev/null
rm -f "$SOCK"
[ "$LOG" = "$OUT/serial.log" ] || { cp "$LOG" "$OUT/serial.log"; rm -f "$LOG"; }

[ -f "$OUT/frame.ppm" ] && python3 "$(dirname "$0")/ppm2png.py" "$OUT/frame.ppm" "$OUT/frame.png" && rm -f "$OUT/frame.ppm"

echo "[test] stop reason: $status"
echo "----- serial.log -----"
tr -d '\r' < "$OUT/serial.log" 2>/dev/null | grep -v '^\x1b' || echo "(no serial output)"
echo "----------------------"

fail=0
# QEMU logs the CPU state at every reset. Start-up resets show the power-on state
# (EIP=0000fff0, paging off). A reset caused by our code (triple fault) would show a running
# 64-bit CPU ("RIP=...") or some other instruction pointer.
bad_resets=$(grep -E '^(RIP|EIP)=' "$OUT/qemu.log" 2>/dev/null | grep -vc '^EIP=0000fff0' || true)
if [ "${bad_resets:-0}" -gt 0 ]; then
  echo "[test] FAIL: $bad_resets CPU reset(s) after code started running (triple fault)"
  fail=1
fi
if [ "$status" = "qemu-exited" ]; then
  echo "[test] FAIL: QEMU exited by itself (machine reset/shut down unexpectedly)"
  fail=1
fi
for e in "${EXPECTS[@]}"; do
  if grep -qF -- "$e" "$OUT/serial.log" 2>/dev/null; then
    echo "[test] ok:   \"$e\""
  else
    echo "[test] FAIL: \"$e\" not found in serial log"
    fail=1
  fi
done

if [ $fail -eq 0 ]; then
  echo "[test] PASS  (screenshot: $OUT/frame.png)"
else
  echo "[test] FAILED"
fi
exit $fail
