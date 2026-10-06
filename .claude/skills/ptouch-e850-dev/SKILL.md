---
name: ptouch-e850-dev
description: Protocol knowledge, capture-analysis tools and working rules for building a macOS driver / printing stack for the Brother PT-E850TKW label printer (label engine only — TZe, HGe, HSe heat-shrink tube). Use this skill whenever the task touches the PT-E850TKW, Brother P-touch raster commands (ESC i z, ESC i S, ESC i K, PackBits, `g` raster lines), decoding printer status bytes, analysing Wireshark/USBPcap captures of P-touch Editor, building print jobs, CUPS/PAPPL/lprint drivers for Brother PT printers, or the ptouch-rs fork (ptouch-core / ptouch-render / ptouch-cli) — even if the user only says "the label printer", "máy in nhãn", "máy in ống", "in nhãn dây điện" or pastes a hex dump starting with 1B 69 or 80 20 42.
---

# PT-E850TKW driver development

This skill supports a project that prints to a Brother PT-E850TKW from macOS without
Brother's Windows software. The printer has two print engines; **only the label engine is
in scope**. The PVC ferrule/tube engine is explicitly out of scope — do not design for it,
do not send it commands, and say so if asked.

## The one rule that matters: verified vs. borrowed

Brother does not publish a command reference for the E850TKW. Everything we know comes
from sibling models — above all Brother's PT-P900/P900W/P950NW raster reference (same
560-pin 360 dpi head class, TZe to 36mm, HSe) — plus LPrint's notes and the ptouch-rs code. These are very likely the same protocol family, but "very likely"
is not good enough when a wrong byte can waste a cassette of HSe tube or wedge the cutter.

So every protocol fact carries one of three tags (see `references/raster-protocol.md`):

- **[E850-verified]** — seen in a capture from this printer, or confirmed by a test print.
- **[sibling-documented]** — in a Brother manual for P900/E550W, not yet seen on E850TKW.
- **[observed-elsewhere]** — reverse-engineered by others (e.g. LPrint), not documented.

When you write code or advice based on a fact that is not `[E850-verified]`, say so in
the code comment or the reply, and propose the capture or test that would verify it. When
a capture verifies something, update the tag in `references/raster-protocol.md` and record
the evidence (capture file name, date) — that file is the project's memory of the protocol.

## Workflow

Figure out which stage the user is in and help them move to the next one.

1. **Capture** — get ground truth from P-touch Editor on Windows.
   Read `references/capture-workflow.md`. Extract the job bytes with
   `scripts/extract_payload.py`.
2. **Decode** — turn bytes into understanding.
   - `scripts/parse_stream.py job.bin` — annotated command listing, raster geometry
     summary, optional `--png` preview of what the printer was told to print.
   - `scripts/decode_status.py <32-byte hex>` — human-readable status block.
   Compare the result with `references/raster-protocol.md` and update tags.
3. **Reproduce** — build our own job and diff it against Brother's.
   - `scripts/build_job.py --image x.png ...` is the *reference encoder* (Python, small,
     readable). Its output is the golden file the Rust encoder must match byte-for-byte.
   - Diff with `parse_stream.py` on both files; differences in header commands matter,
     differences in PackBits run choices usually do not (decompressed raster must match).
4. **Print** — only after the job is byte-equivalent to a Brother job for the same media.
   `scripts/send_raw.py` queries status by default; printing needs `--print --confirm`.
   Always check the status first: loaded media type/width must match what the job declares.
5. **Implement in the ptouch-rs fork** — the project builds on vowstar/ptouch-rs rather
   than starting from zero. Read `references/prior-art.md` for what it already does,
   the six concrete gaps for the E850TKW (560-pin device, non-centred band, HSe media,
   ESC i z media type, TCP transport, macOS USB caveat) and the GPL consequence.
6. **System-wide printing / packaging** — see `references/macos-integration.md`.

## Hardware safety (why these rules exist)

- Never send a job whose `ESC i z` media width/type disagrees with the live status read —
  the printer will either error out or, worse, print off the tape onto the platen.
- Test new header combinations on cheap 12mm TZe first, not HSe tube.
- Keep test labels short (≤ 30mm) while experimenting.
- Never send `ESC i X ...` / undocumented setting commands that write to the printer
  (anything that is not a query). Querying is fine; writing settings is not, unless the
  user explicitly asks and understands the risk.
- Do not attempt firmware updates, mode switches into the tube engine, or P-touch
  Template writes from our tools.

## Quick protocol picture

A job for the PT label engine looks like this (details and tags in the reference):

```
00 × 200            invalidate (clears a half-received job)
1B 40               ESC @ initialise
1B 69 53            ESC i S status request   (read 32 bytes back, check media)
--- per page ---
1B 69 61 01         ESC i a 01 → raster mode
1B 69 7A …10 bytes  ESC i z print info: 0x86, type (00 tape / 11 HSe), width mm, 0,
                    raster lines u32 LE, page (0 first/1 mid/2 last), 0
1B 69 4D nn         ESC i M various mode (0x40 auto-cut, 0x80 mirror)
1B 69 41 nn         ESC i A cut every n labels
1B 69 4B nn         ESC i K advanced (0x04 half-cut, 0x08 cut at end, 0x40 hi-res)
1B 69 64 n1 n2      ESC i d margin (feed amount) in dots
4D 02               M 02 → PackBits compression
47 n1 n2 data…      G: one raster line (n = byte count after compression; 70 B decompressed)
5A                  Z: one all-zero raster line
1A                  print + feed (last page)   |  0C = print, more pages follow
```

Raster lines run *along the feed direction*: each `G` line is one column of the label,
covering the full 560-pin head; the tape occupies a band of pins that is **not centred**
(12mm TZe = pins 197..346). The P900 doc's table is in the reference §7; confirming it on
the E850TKW is the single most important early capture. Never send anything while the
printer is printing — wait for the "printing completed" status.

## Rendering notes

Read `references/rendering.md` before writing layout code: text has to be rotated 90°,
Vietnamese diacritics need a font with full Latin Extended Additional coverage, and HSe
tube has a smaller printable band and different margins than flat TZe tape.

## Files

| Path | Read when |
|---|---|
| `references/raster-protocol.md` | Any question about bytes, commands, status, media codes |
| `references/capture-workflow.md` | Setting up or analysing Wireshark / USBPcap captures |
| `references/macos-integration.md` | USB/TCP transport on macOS, PAPPL/LPrint, CUPS, packaging |
| `references/rendering.md` | Layout, fonts, rotation, dithering, batch cable labels |
| `references/prior-art.md` | Working in the ptouch-rs fork; reuse vs. change; licensing |
| `scripts/ptouch_common.py` | Shared tables + PackBits; imported by the other scripts |
| `scripts/parse_stream.py` | Decode a job/capture into an annotated listing (+ PNG) |
| `scripts/decode_status.py` | Decode a 32-byte status reply |
| `scripts/extract_payload.py` | Pull printer-bound bytes out of a .pcap/.pcapng (needs tshark) |
| `scripts/build_job.py` | Reference encoder: PNG → job .bin |
| `scripts/send_raw.py` | TCP 9100 status query / guarded printing |

All scripts are Python 3 standard library only, except `--png` options which need Pillow.
