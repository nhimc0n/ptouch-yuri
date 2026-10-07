"""Shared tables and helpers for PT-E850TKW tooling (label engine only).

Tables mirror references/raster-protocol.md. Keep the two in sync: when a value is
verified on the E850TKW, update both and flip `verified` to True.
"""

from __future__ import annotations

# --------------------------------------------------------------------------- media

MEDIA_TYPES = {
    0x00: "no media",
    0x01: "laminated tape (TZe/HGe?)",
    0x03: "non-laminated tape",
    0x04: "fabric tape",
    0x11: "heat-shrink tube 2:1 (HSe)",
    0x13: "FLe tape",
    0x14: "flexible ID tape",
    0x15: "satin tape",
    0x17: "heat-shrink tube 3:1",
    0xFF: "incompatible media",
}

MEDIA_WIDTHS = {
    0: "none", 4: "3.5mm", 6: "6mm / HSe 5.8mm", 9: "9mm / HSe 8.8mm",
    12: "12mm / HSe 11.7mm", 18: "18mm / HSe 17.7mm", 21: "21mm FLe",
    24: "24mm / HSe 23.6mm", 36: "36mm",
}

TAPE_COLORS = {
    0x01: "white", 0x02: "other", 0x03: "clear", 0x04: "red", 0x05: "blue",
    0x06: "yellow", 0x07: "green", 0x08: "black", 0x09: "clear (white text)",
    0x20: "matte white", 0x21: "matte clear", 0x22: "matte silver",
    0x23: "satin gold", 0x24: "satin silver", 0x30: "blue (D)", 0x31: "red (D)",
    0x40: "fluorescent orange", 0x41: "fluorescent yellow",
    0x50: "berry pink (S)", 0x51: "light gray (S)", 0x52: "lime green (S)",
    0x60: "yellow (F)", 0x61: "pink (F)", 0x62: "blue (F)",
    0x70: "white (heat-shrink tube)", 0x90: "white (flex ID)", 0x91: "yellow (flex ID)",
    0xF0: "cleaning", 0xF1: "stencil", 0xFF: "incompatible",
}

TEXT_COLORS = {
    0x01: "white", 0x02: "other", 0x04: "red", 0x05: "blue", 0x08: "black",
    0x0A: "gold", 0x62: "blue (F)", 0xF0: "cleaning", 0xF1: "stencil", 0xFF: "incompatible",
}

# Media presets: name -> (status_media_type, esc_i_z_media_type, width_code)
# NOTE: the status reply reports laminated tape as 0x01, but the P900 doc says
# ESC i z n2 must be 0x00 for laminated/non-laminated tape. HSe is 0x11 in both.
MEDIA_PRESETS = {
    "tze3.5": (0x01, 0x00, 4), "tze6": (0x01, 0x00, 6), "tze9": (0x01, 0x00, 9),
    "tze12": (0x01, 0x00, 12), "tze18": (0x01, 0x00, 18), "tze24": (0x01, 0x00, 24),
    "tze36": (0x01, 0x00, 36),
    "hse5.8": (0x11, 0x11, 6), "hse8.8": (0x11, 0x11, 9), "hse11.7": (0x11, 0x11, 12),
    "hse17.7": (0x11, 0x11, 18), "hse23.6": (0x11, 0x11, 24),
}


def preset_for_status(media_type: int, width_code: int) -> str | None:
    """Map a live status (byte 11, byte 10) to a preset name."""
    for name, (st, _z, w) in MEDIA_PRESETS.items():
        if st == media_type and w == width_code:
            return name
    return None

# --------------------------------------------------------------------------- head

# Head geometry from Brother "Raster Command Reference PT-P900/P900W/P950NW/P910BT"
# v1.02 section 2.3.5 (560-pin head, 70 bytes per raster line).
#
# CONVENTION (E850-verified on TZe 36mm, captures 2 and 3, 2026-10-06): the first value is
# the number of LEADING zero pins in raster-line byte order (pin 0 = MSB of byte 0), i.e.
# the P900 doc's *right* margin column. P-touch Editor's 36mm jobs ink pins 61..514, while
# the doc lists 45 left / 61 right. Other widths are flipped the same way but unverified.
#
# key: preset name -> (leading_pins, print_pins, verified_on_E850)
HEAD_PINS = 560
HEAD_BYTES_PER_LINE = 70
HEAD_GEOMETRY = {
    "tze3.5": (264, 48, False),
    "tze6": (256, 64, False),
    "tze9": (235, 106, True),
    "tze12": (213, 150, False),
    "tze18": (171, 234, False),
    "tze24": (128, 320, False),
    "tze36": (61, 454, True),
    "hse5.8": (260, 56, False),
    "hse8.8": (240, 96, False),
    "hse11.7": (222, 132, False),
    "hse17.7": (182, 212, False),
    "hse23.6": (160, 256, False),
}

# Minimum lengths / margins (P900 doc §2.3.3–2.3.4), in 360-dpi dots.
MIN_MARGIN_DOTS = 14            # 1 mm
MIN_LENGTH_DOTS = {"tze": 57, "hse": 60}   # 4 mm / 4.2 mm print data
MIN_FEED_MM = 27                # physical minimum tape fed out (cutter position)
MAX_LENGTH_DOTS = {"tze": 14173, "hse": 7087}

# --------------------------------------------------------------------------- status

BATTERY = {0: "full", 1: "half", 2: "low", 3: "needs charging", 4: "on AC adapter", 0xFF: "unknown"}
EXT_ERROR = {0x10: "FLe tape end", 0x1D: "hi-res/draft error",
             0x1E: "adapter pull/insert",
             0x1F: "incompatible media (LPrint)", 0x21: "incompatible media (P900 doc)"}
ERR1 = {0x01: "no media", 0x02: "end of media", 0x04: "cutter jam", 0x08: "low battery",
        0x10: "printer in use", 0x20: "printer off", 0x40: "high-voltage adapter",
        0x80: "fan motor error"}
ERR2 = {0x01: "wrong media", 0x02: "expansion buffer full", 0x04: "communication error",
        0x08: "communication buffer full", 0x10: "cover open",
        0x20: "overheat / cancel key", 0x40: "feed error", 0x80: "system error"}
STATUS_TYPE = {0: "reply to status request", 1: "printing completed", 2: "error occurred",
               3: "exit IF mode", 4: "turned off", 5: "notification", 6: "phase change"}
PHASE_TYPE = {0: "editing/receiving", 1: "printing"}
NOTIFICATION = {0: "none", 1: "cover open", 2: "cover closed",
                3: "cooling started", 4: "cooling finished"}

KNOWN_MODELS = {  # (series, model) bytes 3,4 — P900 doc: P900 'q', P900W 'o', P950NW 'p', P910BT 'x'
    (0x30, 0x66): "PT-E550W", (0x30, 0x68): "PT-P750W", (0x30, 0x6F): "PT-P900W",
    (0x30, 0x70): "PT-P950NW", (0x30, 0x71): "PT-P900", (0x30, 0x78): "PT-P910BT",
    # (0x30, 0x??): "PT-E850TKW",  <- fill in from first real status reply
}


def bits(value: int, table: dict[int, str]) -> list[str]:
    return [name for bit, name in table.items() if value & bit]


def _ascii(bs: bytes) -> str:
    return "".join(chr(c) if 32 <= c < 127 else "." for c in bs)


def decode_status(b: bytes) -> dict:
    """Decode one 32-byte status block. Raises ValueError if it does not look like one."""
    if len(b) != 32 or b[0] != 0x80 or b[1] != 0x20 or b[2] != 0x42:
        raise ValueError(f"not a status block: {b[:4].hex(' ')}")
    return {
        "model": KNOWN_MODELS.get((b[3], b[4]),
                                  f"UNKNOWN series/model {b[3]:#04x}/{b[4]:#04x} ({_ascii(b[3:5])!r})"),
        "battery": BATTERY.get(b[6], f"{b[6]:#04x}"),
        "extended_error": EXT_ERROR.get(b[7], "none" if b[7] == 0 else f"{b[7]:#04x}"),
        "errors": bits(b[8], ERR1) + bits(b[9], ERR2),
        "media_width": f"{b[10]} ({MEDIA_WIDTHS.get(b[10], '?')})",
        "media_type": f"{b[11]:#04x} ({MEDIA_TYPES.get(b[11], 'UNKNOWN')})",
        "mode": f"{b[15]:#04x}",
        "media_length": b[17],
        "status_type": STATUS_TYPE.get(b[18], f"{b[18]:#04x}"),
        "phase": f"{PHASE_TYPE.get(b[19], hex(b[19]))} #{b[20] | (b[21] << 8)}",
        "notification": NOTIFICATION.get(b[22], f"{b[22]:#04x}"),
        "tape_color": f"{b[24]:#04x} ({TAPE_COLORS.get(b[24], '?')})",
        "text_color": f"{b[25]:#04x} ({TEXT_COLORS.get(b[25], '?')})",
        "hw_info": b[26:30].hex(" "),
        "raw": b.hex(" "),
    }


def find_status_blocks(data: bytes) -> list[bytes]:
    """Find every 32-byte status block (80 20 42 ...) in a byte stream."""
    out, i = [], 0
    while True:
        i = data.find(b"\x80\x20\x42", i)
        if i < 0 or i + 32 > len(data):
            return out
        out.append(data[i:i + 32])
        i += 32

# --------------------------------------------------------------------------- packbits


def packbits_decode(data: bytes) -> bytes:
    out, i = bytearray(), 0
    while i < len(data):
        h = data[i]
        i += 1
        if h < 128:
            out += data[i:i + h + 1]
            i += h + 1
        elif h > 128:
            if i >= len(data):
                raise ValueError("truncated PackBits run")
            out += bytes([data[i]]) * (257 - h)
            i += 1
        # h == 128: no-op
    return bytes(out)


def packbits_encode(data: bytes) -> bytes:
    out, i, n = bytearray(), 0, len(data)
    while i < n:
        # run length at i
        j = i + 1
        while j < n and j - i < 128 and data[j] == data[i]:
            j += 1
        run = j - i
        if run >= 2:
            out += bytes([257 - run, data[i]])
            i = j
            continue
        # literal: extend until a run of >=2 starts or 128 bytes
        j = i + 1
        while j < n and j - i < 128:
            if j + 1 < n and data[j] == data[j + 1]:
                break
            j += 1
        out.append(j - i - 1)
        out += data[i:j]
        i = j
    return bytes(out)
