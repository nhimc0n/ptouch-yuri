#!/usr/bin/env python3
"""Reference encoder: PNG -> PT raster job (.bin). Golden source for the Rust encoder.

The image is in *design orientation*: width = label length (feed direction),
height = tape width. It is thresholded to 1 bit; height must equal the printable pins
for the media (or pass --fit to scale it).

  build_job.py --image label.png --media tze12 -o job.bin
  build_job.py --image label.png --media hse8.8 --cut half --copies 5 -o job.bin
  build_job.py --image label.png --media tze12 --left 199 --pins 150 -o job.bin   # override
  build_job.py --test-pattern rect --length-mm 30 --media tze12 -o rect.bin

Geometry defaults come from ptouch_common.HEAD_GEOMETRY (Brother PT-P900 raster
reference) and are UNVERIFIED for the E850TKW until marked otherwise — the script warns.
Page byte (ESC i z n9) follows the P900 doc: 0 first, 1 middle, 2 last; a single page
uses 0 unless --single-page-byte says otherwise (verify against a capture). Building a file never prints;
see send_raw.py for that.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from ptouch_common import (HEAD_BYTES_PER_LINE, HEAD_GEOMETRY, MEDIA_PRESETS,  # noqa: E402
                           MIN_MARGIN_DOTS, packbits_encode)

DPI = 360


def load_bitmap(args, pins):
    """Return list of columns; each column is a list[bool] of length `pins` (True = ink)."""
    if args.test_pattern:
        length = round(args.length_mm / 25.4 * DPI)
        if args.test_pattern == "rect":
            return [[True] * pins for _ in range(length)]
        if args.test_pattern == "F":  # asymmetric: vertical bar at start + bar along top + mid stub
            cols = []
            for x in range(length):
                col = [False] * pins
                for y in range(pins):
                    if x < length // 6 or (y < pins // 5) or (pins * 2 // 5 <= y < pins * 3 // 5 and x < length // 2):
                        col[y] = True
                cols.append(col)
            return cols
    try:
        from PIL import Image
    except ImportError:
        sys.exit("Pillow needed for --image (pip install pillow)")
    img = Image.open(args.image).convert("L")
    if img.height != pins:
        if not args.fit:
            sys.exit(f"image height {img.height} != printable pins {pins}; use --fit or resize")
        img = img.resize((round(img.width * pins / img.height), pins))
    px = img.load()
    return [[px[x, y] < args.threshold for y in range(pins)] for x in range(img.width)]


def column_to_line(col, left, bytes_per_line, flip):
    line = bytearray(bytes_per_line)
    seq = list(reversed(col)) if flip else col
    for y, ink in enumerate(seq):
        if ink:
            p = left + y
            line[p // 8] |= 0x80 >> (p % 8)
    return bytes(line)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    src = ap.add_mutually_exclusive_group(required=True)
    src.add_argument("--image")
    src.add_argument("--test-pattern", choices=["rect", "F"])
    ap.add_argument("--length-mm", type=float, default=30)
    ap.add_argument("--media", required=True, choices=sorted(MEDIA_PRESETS))
    ap.add_argument("--pins", type=int, help="override printable pins")
    ap.add_argument("--left", type=int, help="override left-margin pin offset")
    ap.add_argument("--bytes-per-line", type=int, default=HEAD_BYTES_PER_LINE)
    ap.add_argument("--margin-dots", type=int, default=MIN_MARGIN_DOTS, help="ESC i d feed margin")
    ap.add_argument("--single-page-byte", type=int, choices=[0, 2], default=2,
                    help="ESC i z page byte for a single page; P-touch Editor sends 2 [E850-verified]")
    ap.add_argument("--job-number", type=int, default=None,
                    help="emit ESC i U job tag with this job number (P-touch Editor does)")
    ap.add_argument("--cut", choices=["full", "half", "none"], default="full")
    ap.add_argument("--copies", type=int, default=1)
    ap.add_argument("--flip", action="store_true", help="reverse pin order (orientation test)")
    ap.add_argument("--no-compress", action="store_true")
    ap.add_argument("--fit", action="store_true")
    ap.add_argument("--threshold", type=int, default=128)
    ap.add_argument("-o", "--out", required=True)
    a = ap.parse_args()

    _status_type, ztype, wcode = MEDIA_PRESETS[a.media]
    left, pins, verified = HEAD_GEOMETRY[a.media]
    if a.left is not None:
        left = a.left
    if a.pins is not None:
        pins = a.pins
    if not verified and a.left is None and a.pins is None:
        print(f"WARNING: geometry for {a.media} is UNVERIFIED on E850TKW "
              "(PT-P900 raster reference values)", file=sys.stderr)
    if a.margin_dots < MIN_MARGIN_DOTS:
        sys.exit(f"margin below documented minimum {MIN_MARGIN_DOTS} dots")
    if left + pins > a.bytes_per_line * 8:
        sys.exit("band exceeds head width")

    cols = load_bitmap(a, pins)
    lines = [column_to_line(c, left, a.bytes_per_line, a.flip) for c in cols]

    out = bytearray(b"\x00" * 200 + b"\x1b\x40")   # P900 doc: 200-byte invalidate
    for page in range(a.copies):
        out += b"\x1b\x69\x61\x01"                     # raster mode (per page, as in doc)
        if a.job_number is not None:                    # [E850-verified present in Editor jobs; meaning unknown]
            out += b"\x1b\x69\x55\x4a\x00\x0c\x04\x42\x1a\xb3\xd4\x9c\x00\x00" + bytes([a.job_number & 0xFF, 0, 0, 0])
        if a.copies == 1:
            page_byte = a.single_page_byte
        else:
            page_byte = 0 if page == 0 else (2 if page == a.copies - 1 else 1)
        out += b"\x1b\x69\x7a" + bytes([0x84 if ztype == 0 else 0x86, ztype, wcode, 0]) + len(lines).to_bytes(4, "little") \
            + bytes([page_byte, 0])
        out += b"\x1b\x69\x4d" + bytes([0x40 if a.cut != "none" else 0x00])
        if a.cut != "none":
            out += b"\x1b\x69\x41\x01"
        k = 0x08  # feed/cut at end
        if a.cut == "half":
            k |= 0x04
        out += b"\x1b\x69\x4b" + bytes([k])
        out += b"\x1b\x69\x6b\x63\x01\x00"             # ESC i k: purpose unknown, in every Editor job
        out += b"\x1b\x69\x64" + a.margin_dots.to_bytes(2, "little")
        out += b"\x4d" + (b"\x00" if a.no_compress else b"\x02")
        for l in lines:
            if not any(l):
                out += b"\x5a"
                continue
            payload = l if a.no_compress else packbits_encode(l)
            out += b"\x47" + len(payload).to_bytes(2, "little") + payload
        out += b"\x1a" if page == a.copies - 1 else b"\x0c"

    Path(a.out).write_bytes(out)
    print(f"wrote {len(out)} bytes, {len(lines)} lines/page × {a.copies} to {a.out}", file=sys.stderr)


if __name__ == "__main__":
    main()
