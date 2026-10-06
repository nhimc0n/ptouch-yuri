#!/usr/bin/env python3
"""Decode a Brother PT raster job (as sent to the printer) into an annotated listing.

Usage:
  parse_stream.py job.bin [--png out.png] [--lines] [--json]
  parse_stream.py --hex "1b 40 1b 69 61 01 ..."

Anything unrecognised is reported as UNKNOWN with surrounding bytes and the parser
stops there (it cannot know the command length) — record those bytes in
references/raster-protocol.md.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from ptouch_common import MEDIA_WIDTHS, packbits_decode  # noqa: E402


def flags(v: int, table: dict[int, str]) -> str:
    names = [n for b, n in table.items() if v & b]
    return ", ".join(names) if names else "none"


ZTYPES = {0x00: "laminated/non-laminated tape", 0x09: "laminated, hi-res/draft (P900 doc note)",
          0x11: "heat-shrink 2:1", 0x13: "FLe", 0x17: "heat-shrink 3:1", 0xFF: "incompatible"}
ESC_I_K = {0x01: "draft", 0x04: "half-cut", 0x08: "no-chain/cut-at-end",
           0x10: "special tape", 0x40: "high-res", 0x80: "keep expansion buffer"}
ESC_I_M = {0x40: "auto-cut", 0x80: "mirror"}
ESC_I_Z_FLAGS = {0x02: "type valid", 0x04: "width valid", 0x08: "length valid",
                 0x40: "quality", 0x80: "recover"}


def parse(data: bytes):
    cmds, lines = [], []        # lines: decompressed raster (bytes) per raster line
    compression = 0
    i, n = 0, len(data)

    def add(name, start, end, detail=""):
        cmds.append({"offset": start, "bytes": data[start:end].hex(" ") if end - start <= 16
                     else data[start:start + 16].hex(" ") + f" …(+{end - start - 16})",
                     "cmd": name, "detail": detail})

    while i < n:
        b = data[i]
        if b == 0x00:
            j = i
            while j < n and data[j] == 0:
                j += 1
            add("invalidate", i, j, f"{j - i} × 00")
            i = j
        elif b == 0x1B and i + 1 < n and data[i + 1] == 0x40:
            add("ESC @ initialise", i, i + 2); i += 2
        elif b == 0x1B and i + 2 < n and data[i + 1] == 0x69:
            c = data[i + 2]
            if c == 0x53:
                add("ESC i S status request", i, i + 3); i += 3
            elif c == 0x61:
                m = data[i + 3]
                add("ESC i a mode", i, i + 4, {0: "ESC/P", 1: "raster", 3: "P-touch Template"}.get(m, f"{m:#04x} ?")); i += 4
            elif c == 0x7A:
                p = data[i + 3:i + 13]
                lines_n = int.from_bytes(p[4:8], "little")
                add("ESC i z print info", i, i + 13,
                    f"flags={p[0]:#04x} [{flags(p[0], ESC_I_Z_FLAGS)}] "
                    f"type={p[1]:#04x} ({ZTYPES.get(p[1], 'UNKNOWN')}) "
                    f"width={p[2]} ({MEDIA_WIDTHS.get(p[2], '?')}) length={p[3]} "
                    f"raster_lines={lines_n} page={ {0: 'first', 1: 'middle', 2: 'last'}.get(p[8], p[8]) } b9={p[9]}")
                i += 13
            elif c == 0x4D:
                add("ESC i M various mode", i, i + 4, f"{data[i+3]:#04x} [{flags(data[i+3], ESC_I_M)}]"); i += 4
            elif c == 0x4B:
                add("ESC i K advanced", i, i + 4, f"{data[i+3]:#04x} [{flags(data[i+3], ESC_I_K)}]"); i += 4
            elif c == 0x41:
                add("ESC i A cut every", i, i + 4, f"n={data[i+3]}"); i += 4
            elif c == 0x55:
                add("ESC i U job tag (P-touch Editor)", i, i + 18,
                    f"payload={data[i+3:i+18].hex(' ')} job_no={data[i+14]}  [E850-seen, meaning unverified]"); i += 18
            elif c == 0x6b:
                add("ESC i k (unknown, sent by P-touch Editor)", i, i + 6,
                    f"payload={data[i+3:i+6].hex(' ')}  [E850-seen, meaning unverified]"); i += 6
            elif c == 0x64:
                add("ESC i d margin", i, i + 5, f"{data[i+3] | (data[i+4] << 8)} dots"); i += 5
            elif c == 0x21:
                add("ESC i ! auto status notify", i, i + 4, "notify" if data[i + 3] == 0 else "off"); i += 4
            elif c == 0x58 and i + 3 < n and data[i + 3] == 0x47:
                add("ESC i X G extended info query", i, i + 4); i += 4
            else:
                add("UNKNOWN ESC i", i, min(n, i + 16), f"command byte {c:#04x} ('{chr(c) if 32 <= c < 127 else '?'}') — length unknown, stopping")
                return cmds, lines, compression, False
        elif b == 0x4D:
            compression = data[i + 1]
            add("M compression", i, i + 2, {0: "none", 2: "PackBits"}.get(compression, f"{compression:#04x} ?")); i += 2
        elif b in (0x47, 0x67):   # doc text says 47h ('G'); its command table says 67h ('g')
            ln = data[i + 1] | (data[i + 2] << 8)
            payload = data[i + 3:i + 3 + ln]
            raw = packbits_decode(payload) if compression == 2 else payload
            lines.append(raw)
            i += 3 + ln
        elif b == 0x5A:
            lines.append(None)     # zero line; width filled in later
            i += 1
        elif b in (0x0C, 0x1A):
            add("print" if b == 0x0C else "print + feed (last page)", i, i + 1,
                f"after {len(lines)} raster lines total"); i += 1
        else:
            add("UNKNOWN", i, min(n, i + 16), "unrecognised byte — stopping")
            return cmds, lines, compression, False
    return cmds, lines, compression, True


def summarise(lines):
    widths = sorted({len(l) for l in lines if l is not None})
    w = widths[-1] if widths else 0
    full = [l if l is not None else bytes(w) for l in lines]
    first = last = None
    for l in full:
        for byte_i, v in enumerate(l):
            if v:
                for bit in range(8):
                    if v & (0x80 >> bit):
                        p = byte_i * 8 + bit
                        first = p if first is None else min(first, p)
                        last = p if last is None else max(last, p)
    h = hashlib.sha256(b"".join(full)).hexdigest()[:16]
    return {"raster_lines": len(lines), "zero_lines": sum(1 for l in lines if l is None),
            "bytes_per_line": widths, "head_pins": w * 8,
            "ink_span_pins": None if first is None else [first, last],
            "ink_height_pins": None if first is None else last - first + 1,
            "raster_sha256_16": h}, full


def to_png(full, path):
    try:
        from PIL import Image
    except ImportError:
        sys.exit("Pillow needed for --png (pip install pillow)")
    if not full:
        sys.exit("no raster lines")
    w_pins = len(full[0]) * 8
    # x = raster line index (feed direction), y = head pin
    img = Image.new("1", (len(full), w_pins), 1)
    px = img.load()
    for x, l in enumerate(full):
        for byte_i, v in enumerate(l):
            for bit in range(8):
                if v & (0x80 >> bit):
                    px[x, byte_i * 8 + bit] = 0
    img.save(path)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("file", nargs="?")
    ap.add_argument("--hex", help="parse hex string instead of a file")
    ap.add_argument("--png", help="write raster preview (x=feed, y=head pin)")
    ap.add_argument("--lines", action="store_true", help="dump each raster line as hex")
    ap.add_argument("--json", action="store_true", help="machine-readable output for diffing")
    a = ap.parse_args()
    if a.hex:
        data = bytes.fromhex(a.hex.replace(":", " "))
    elif a.file:
        data = Path(a.file).read_bytes()
    else:
        ap.error("give a file or --hex")

    cmds, lines, comp, complete = parse(data)
    summary, full = summarise(lines)
    summary["complete_parse"] = complete

    if a.json:
        # offsets differ between encoders; keep only what must match
        print(json.dumps({"commands": [(c["cmd"], c["detail"]) for c in cmds],
                          "summary": summary}, indent=1, ensure_ascii=False))
    else:
        for c in cmds:
            print(f"{c['offset']:08x}  {c['cmd']:<32} {c['detail']}")
            print(f"          {c['bytes']}")
        print("\nSUMMARY")
        for k, v in summary.items():
            print(f"  {k:<18} {v}")
        if not complete:
            print("\n  ! parse stopped at an UNKNOWN command — record it in raster-protocol.md")
        if a.lines:
            print("\nRASTER LINES")
            for idx, l in enumerate(full):
                print(f"  {idx:5d} {l.hex()}")
    if a.png:
        to_png(full, a.png)
        print(f"preview written to {a.png}", file=sys.stderr)


if __name__ == "__main__":
    main()
