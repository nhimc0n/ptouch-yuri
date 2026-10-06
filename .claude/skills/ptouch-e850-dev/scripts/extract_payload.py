#!/usr/bin/env python3
"""Extract raw printer traffic from a Wireshark capture (needs `tshark` on PATH).

  extract_payload.py cap.pcapng -o job.bin                  # host -> printer, TCP 9100
  extract_payload.py cap.pcapng -o replies.bin --from-printer
  extract_payload.py cap.pcapng -o job.bin --usb            # USBPcap, bulk OUT
  extract_payload.py cap.pcapng -o job.bin --port 9100 --stream 2

If several TCP streams exist (P-touch Editor opens one per job plus status polls), the
script lists them and uses the largest unless --stream is given.
"""

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
from collections import defaultdict


def tshark(args):
    r = subprocess.run(["tshark", *args], capture_output=True, text=True)
    if r.returncode != 0:
        sys.exit(r.stderr.strip())
    return r.stdout.splitlines()


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("capture")
    ap.add_argument("-o", "--out", required=True)
    ap.add_argument("--port", type=int, default=9100)
    ap.add_argument("--from-printer", action="store_true")
    ap.add_argument("--usb", action="store_true")
    ap.add_argument("--stream", type=int)
    a = ap.parse_args()
    if not shutil.which("tshark"):
        sys.exit("tshark not found (brew install wireshark). Alternative: Follow TCP Stream → Raw → Save as.")

    if a.usb:
        # USBPcap: endpoint direction bit 0x80 = IN (device->host)
        dir_filter = "usb.endpoint_address.direction == 1" if a.from_printer else "usb.endpoint_address.direction == 0"
        rows = tshark(["-r", a.capture, "-Y", f"usb.capdata && {dir_filter}", "-T", "fields", "-e", "usb.capdata"])
        data = b"".join(bytes.fromhex(r.replace(":", "")) for r in rows if r)
    else:
        side = "tcp.srcport" if a.from_printer else "tcp.dstport"
        rows = tshark(["-r", a.capture, "-Y", f"{side} == {a.port} && tcp.len > 0",
                       "-T", "fields", "-e", "tcp.stream", "-e", "tcp.payload"])
        streams = defaultdict(bytearray)
        for r in rows:
            sid, _, payload = r.partition("\t")
            if payload:
                streams[int(sid)] += bytes.fromhex(payload.replace(":", ""))
        if not streams:
            sys.exit("no matching payload")
        for sid, buf in sorted(streams.items()):
            print(f"stream {sid}: {len(buf)} bytes, starts {bytes(buf[:8]).hex(' ')}", file=sys.stderr)
        sid = a.stream if a.stream is not None else max(streams, key=lambda s: len(streams[s]))
        data = bytes(streams[sid])
        print(f"using stream {sid}", file=sys.stderr)

    with open(a.out, "wb") as f:
        f.write(data)
    print(f"wrote {len(data)} bytes to {a.out}", file=sys.stderr)


if __name__ == "__main__":
    main()
