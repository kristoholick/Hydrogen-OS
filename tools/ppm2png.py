#!/usr/bin/env python3
"""Convert QEMU's binary PPM (P6) screendump to PNG using only the standard library."""
import struct
import sys
import zlib


def read_ppm(path):
    with open(path, "rb") as f:
        data = f.read()
    tokens, pos = [], 0
    while len(tokens) < 4:
        while data[pos:pos + 1].isspace():
            pos += 1
        if data[pos:pos + 1] == b"#":
            pos = data.index(b"\n", pos) + 1
            continue
        end = pos
        while not data[end:end + 1].isspace():
            end += 1
        tokens.append(data[pos:end])
        pos = end
    pos += 1  # single whitespace after maxval
    if tokens[0] != b"P6" or int(tokens[3]) != 255:
        raise ValueError("expected 8-bit P6 PPM")
    w, h = int(tokens[1]), int(tokens[2])
    return w, h, data[pos:pos + w * h * 3]


def write_png(path, w, h, rgb):
    def chunk(tag, body):
        return struct.pack(">I", len(body)) + tag + body + struct.pack(">I", zlib.crc32(tag + body) & 0xFFFFFFFF)

    raw = b"".join(b"\x00" + rgb[y * w * 3:(y + 1) * w * 3] for y in range(h))
    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 6))
    png += chunk(b"IEND", b"")
    with open(path, "wb") as f:
        f.write(png)


if __name__ == "__main__":
    write_png(sys.argv[2], *read_ppm(sys.argv[1]))
