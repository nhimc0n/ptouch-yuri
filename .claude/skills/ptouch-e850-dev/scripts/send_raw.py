#!/usr/bin/env python3
"""Talk to the printer over TCP 9100.

  send_raw.py 192.168.1.50                         # status query only (safe)
  send_raw.py 192.168.1.50 --print job.bin         # dry run: checks media vs job, sends nothing
  send_raw.py 192.168.1.50 --print job.bin --confirm

Before printing it reads live status and refuses if the job's ESC i z media type/width
does not match what is loaded, or if the printer reports any error. After sending it
listens for unsolicited status (printing completed / error).
"""

from __future__ import annotations

import argparse
import socket
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from ptouch_common import decode_status, find_status_blocks  # noqa: E402

QUERY = b"\x00" * 200 + b"\x1b\x40" + b"\x1b\x69\x53"


def read_blocks(sock, timeout, want=1):
    sock.settimeout(0.5)
    buf, end = bytearray(), time.time() + timeout
    while time.time() < end:
        try:
            chunk = sock.recv(4096)
            if not chunk:
                break
            buf += chunk
            if len(find_status_blocks(bytes(buf))) >= want:
                break
        except socket.timeout:
            continue
    return find_status_blocks(bytes(buf))


def job_media(job: bytes):
    i = job.find(b"\x1b\x69\x7a")
    if i < 0:
        return None
    p = job[i + 3:i + 13]
    return p[1], p[2]


def show(d):
    for k in ("model", "media_type", "media_width", "tape_color", "text_color",
              "errors", "extended_error", "status_type", "battery"):
        print(f"  {k:<14} {d[k]}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("host")
    ap.add_argument("--port", type=int, default=9100)
    ap.add_argument("--print", dest="job")
    ap.add_argument("--confirm", action="store_true", help="actually send the job")
    ap.add_argument("--timeout", type=float, default=5)
    a = ap.parse_args()

    with socket.create_connection((a.host, a.port), timeout=a.timeout) as s:
        s.sendall(QUERY)
        blocks = read_blocks(s, a.timeout)
        if not blocks:
            sys.exit("no status reply (printer busy, wrong port, or TCP status not supported)")
        st = blocks[0]
        d = decode_status(st)
        print("STATUS"); show(d)
        if not a.job:
            return

        job = Path(a.job).read_bytes()
        jm = job_media(job)
        if jm is None:
            sys.exit("job has no ESC i z — refusing")
        problems = []
        if d["errors"]:
            problems.append(f"printer reports errors: {d['errors']}")
        # ESC i z uses 0x00 for laminated/non-laminated tape; status reports 0x01/0x03
        loaded_type = 0x00 if st[11] in (0x01, 0x03) else st[11]
        if jm != (loaded_type, st[10]):
            problems.append(f"job declares ESC i z type={jm[0]:#04x} width={jm[1]} but loaded is "
                            f"status type={st[11]:#04x} width={st[10]}")
        if problems:
            sys.exit("REFUSING:\n  " + "\n  ".join(problems))
        if not a.confirm:
            print(f"\ndry run OK: {len(job)} bytes would be sent. Add --confirm to print.")
            return

        s.sendall(job)
        print(f"\nsent {len(job)} bytes; waiting for completion status…")
        for b in read_blocks(s, 30, want=8):
            dd = decode_status(b)
            print(f"  {dd['status_type']}: errors={dd['errors']} phase={dd['phase']}")
            if dd["status_type"] in ("printing completed", "error occurred"):
                break


if __name__ == "__main__":
    main()
