#!/usr/bin/env python3
"""Decode Brother PT 32-byte status replies.

Usage:
  decode_status.py "80 20 42 30 ..."        # one block as hex
  decode_status.py replies.bin              # every status block found in a file
  decode_status.py replies.bin --json
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from ptouch_common import decode_status, find_status_blocks  # noqa: E402


def main():
    args = [a for a in sys.argv[1:] if a != "--json"]
    as_json = "--json" in sys.argv
    if not args:
        sys.exit(__doc__)
    src = " ".join(args)
    p = Path(args[0])
    data = p.read_bytes() if len(args) == 1 and p.exists() else bytes.fromhex(src.replace(":", " "))
    blocks = find_status_blocks(data)
    if not blocks:
        sys.exit("no status block (80 20 42 …) found")
    decoded = [decode_status(b) for b in blocks]
    if as_json:
        print(json.dumps(decoded, indent=1, ensure_ascii=False))
        return
    for n, d in enumerate(decoded, 1):
        print(f"--- status block {n}")
        for k, v in d.items():
            print(f"  {k:<15} {v}")
        if "UNKNOWN" in d["model"]:
            print("  ! model code not in table — if this is the E850TKW, record it in raster-protocol.md")


if __name__ == "__main__":
    main()
