#!/usr/bin/env bash
# usage: tools/check-interleave.sh <serial.log>
# Passes if the lines of hello-a and hello-b are complete, in order, and change hands at least 4 times.
set -uo pipefail
seq=$(tr -d '\r' < "$1" | grep -aoE '^\[hello-[ab] [0-9]+\] [AB][0-9]$' | awk '{print $3}' | tr '\n' ' ')
a=$(echo $seq | tr ' ' '\n' | grep '^A' | tr -d '\n')
b=$(echo $seq | tr ' ' '\n' | grep '^B' | tr -d '\n')
turns=$(echo $seq | tr ' ' '\n' | cut -c1 | uniq | wc -l)
echo "[test] order of hello-a and hello-b: $seq"
if [ "$a" = "A1A2A3A4A5" ] && [ "$b" = "B1B2B3B4B5" ] && [ "$turns" -ge 5 ]; then
  echo "[test] ok:   output interleaved ($((turns - 1)) changes)"
else
  echo "[test] FAIL: output not complete, not in order or not interleaved"
  exit 1
fi
