#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Validate native Linux/macOS binaries and exercise the window renderer."""

import argparse
from pathlib import Path
import struct
import subprocess


def check_architecture(path, target):
    data = path.read_bytes()[:64]
    if target.endswith("linux-gnu"):
        expected = 183 if target.startswith("aarch64") else 62
        if data[:6] != b"\x7fELF\x02\x01" or struct.unpack_from("<H", data, 18)[0] != expected:
            raise ValueError(f"{path}: expected 64-bit little-endian ELF for {target}")
    elif data[:4] != b"\xcf\xfa\xed\xfe" or struct.unpack_from("<I", data, 4)[0] != 0x0100000C:
        raise ValueError(f"{path}: expected ARM64 Mach-O")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True, choices=("x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu", "aarch64-apple-darwin"))
    parser.add_argument("--profile", required=True, choices=("debug", "release"))
    args = parser.parse_args()
    directory = Path("target") / args.target / args.profile
    for name in ("ptouch", "ptouch-gui"):
        check_architecture(directory / name, args.target)
    for option in ("--version", "--help"):
        subprocess.run([str(directory / "ptouch"), option], check=True, timeout=30)
    result = subprocess.run([str(directory / "ptouch-gui"), "--smoke-test"], check=True, timeout=60, capture_output=True, text=True)
    if "PTOUCH_GUI_SMOKE_OK" not in result.stdout:
        raise RuntimeError(f"GUI rendering check failed: {result.stdout}\n{result.stderr}")
    print("Binary architecture, CLI startup, and GUI rendering passed")
