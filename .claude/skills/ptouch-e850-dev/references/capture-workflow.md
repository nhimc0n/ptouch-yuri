# Capturing ground truth from P-touch Editor

Goal: a library of `.bin` files, each one a complete job that Brother's own software sent
to the E850TKW for a known design and known media. These are the test fixtures for
everything else.

## Setup

- Windows VM on the Mac (UTM on Apple Silicon runs Windows 11 ARM; Parallels also works).
  Install the PT-E850TKW driver + P-touch Editor from support.brother.com.
- **Prefer Wi-Fi/TCP capture.** Put the printer on the LAN (infrastructure mode), add it
  in Windows as a network printer, and capture on the Mac or the VM:
  - Wireshark filter: `tcp.port == 9100` (raw print) and `udp.port == 161` (SNMP status).
  - Advantage: no USB passthrough fiddling, and both directions are on one TCP stream.
- USB capture (fallback): pass the printer through to the VM and use USBPcap in the VM,
  or on macOS capture USB with Wireshark on an `XHC` interface (requires
  `sudo ifconfig XHC20 up` style setup on Intel; on Apple Silicon USB capture is limited —
  prefer the network route).

## Recording a capture session

Name files so the fixture explains itself:
`YYYYMMDD_<media>_<design>_<options>.pcapng`, e.g.
`20261010_tze12_blackrect40mm_fullcut.pcapng`,
`20261010_hse8.8_text-L1-01_halfcut.pcapng`.

Minimum fixture set (cheap media first):
1. TZe 12mm, solid black rectangle 30mm long, auto-cut — gives head geometry for 12mm.
2. TZe 12mm, asymmetric glyph ("F" or an arrow) — gives orientation/mirror.
3. TZe 12mm, 3 copies, cut every 1, half-cut on — gives ESC i A / ESC i K behaviour.
4. Same black-rectangle design for each other TZe width you own.
5. HSe 8.8mm (and any other HSe you own), black rectangle — HSe geometry + header bytes.
6. HSe text label as electricians would use it — realistic job.
7. Idle connection (open P-touch Editor, no print) — startup queries (ESC i S, ESC i X G, SNMP).

Also write down, per capture: media actually loaded, cassette part number, P-touch Editor
settings (margins, cut options, high-res), and whether the print came out right.

## Extracting the job bytes

```
python3 scripts/extract_payload.py capture.pcapng -o job.bin            # printer-bound (default)
python3 scripts/extract_payload.py capture.pcapng -o replies.bin --from-printer
python3 scripts/extract_payload.py capture.pcapng -o job.bin --usb      # USBPcap capture
```
Requires `tshark` (`brew install wireshark` provides it). The script concatenates TCP
payloads to port 9100 in stream order (or USB bulk OUT data for `--usb`).

If tshark is unavailable: in Wireshark, Follow → TCP Stream → show "Raw", choose only
the client→printer direction, "Save as…" → that file is the job.

## No tshark? Pure-Python route (what the E850 needs)

The E850TKW is printed by Windows through **LPR on TCP 515** (queue `BINARY_P1`), not raw 9100:
```
python3 scripts/extract_lpr.py capture.pcapng fixtures/jobs/   # lists LPR jobs, saves each data file
python3 scripts/snmp_dump.py capture.pcapng                    # SNMP requests/responses (status polling)
```
Capture with the filter `host <printer-ip>` so both LPR and SNMP are recorded. The data
file extracted from LPR is exactly the job `parse_stream.py` expects.

## Analysing

```
python3 scripts/parse_stream.py job.bin                 # annotated listing + summary
python3 scripts/parse_stream.py job.bin --png out.png   # what the printer was told to print
python3 scripts/parse_stream.py job.bin --lines         # also dump each raster line
python3 scripts/decode_status.py replies.bin            # each 32-byte status block in the file
```

The summary prints: commands seen, decompressed bytes/line, number of raster lines,
ink span (first..last set pin). From the black-rectangle fixtures, the ink span is the
printable band → fill §7 of `raster-protocol.md`.

Anything the parser flags as UNKNOWN is the most valuable output — record the bytes and
the context in `raster-protocol.md` §1 and §9.

## Diffing our output against Brother's

```
python3 scripts/build_job.py --image same_design.png --media tze12 -o ours.bin
python3 scripts/parse_stream.py brother.bin --json > a.json
python3 scripts/parse_stream.py ours.bin --json > b.json
diff a.json b.json
```
Header commands must match. Raster must match after decompression (the JSON contains a
hash of the decompressed raster). Different PackBits run choices are fine.
