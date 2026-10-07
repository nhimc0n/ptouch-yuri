# PT-series raster protocol — as it applies to the PT-E850TKW (label engine)

Sources (in order of trust):
1. Brother "Raster Command Reference PT-P900/P900W/P950NW/P910BT" v1.02
   (download.brother.com/welcome/docp100407/cv_ptp900_eng_raster_102.pdf). Closest
   sibling — same 560-pin 360 dpi head class, TZe up to 36mm and HSe. Section numbers
   below (§2.3.x, §4) refer to this document. E850TKW is **not** in Brother's list of
   command-reference models.
2. Brother raster references for PT-E550W/P750W/P710BT and PT-H500/P700/E500.
3. LPrint wiki "Brother Driver" (github.com/michaelrsweet/lprint/wiki).
4. ptouch-rs / ptouch-print source — see `prior-art.md`.

Tags: **[E850-verified]** **[sibling-documented]** **[observed-elsewhere]** — see SKILL.md.
When you verify something, change the tag and add a line to the Evidence log at the bottom.

## Contents
1. Command table
2. ESC i z — print information
3. ESC i M / ESC i K / ESC i A / ESC i d
4. Compression and raster lines
5. Status reply (ESC i S)
6. Media codes
7. Head geometry (MUST verify)
8. Extended info (ESC i X G)
9. Open questions
10. Evidence log

## 1. Command table

| Bytes | Name | Tag | Notes |
|---|---|---|---|
| `00` ×200 | Invalidate | [E850-verified] | 200 zero bytes then ESC @, first bytes of every Editor job (captures 2, 3). |
| `1B 40` | ESC @ initialise | [E850-verified] | Follows the 200 zeros. |
| `1B 69 53` | ESC i S status request | [sibling-documented] | Reply: 32 bytes. Over USB the first 1–2 reads may be empty — retry. |
| `1B 69 61 n` | ESC i a switch mode | [E850-verified] (`01`) | `01` raster. (`00` ESC/P, `03` P-touch Template on models that have them.) |
| `1B 69 7A` +10 | ESC i z print info | [E850-verified] for TZe 36 mm | §2 |
| `1B 69 4D n` | ESC i M various mode | [E850-verified] (`40`) | §3 |
| `1B 69 41 n` | ESC i A cut every n | [E850-verified] (`01`) | Only meaningful with auto-cut bit. |
| `1B 69 4B n` | ESC i K advanced mode | [E850-verified] (`0C` = half-cut + cut at end) | §3 |
| `1B 69 64 n1 n2` | ESC i d margin | [E850-verified] (`0E 00` = 14 dots, UI feed 0.04") | Feed amount in dots, little-endian. |
| `4D n` | M compression | [E850-verified] (`02`) | `00` none, `02` PackBits. |
| `47 n1 n2 d…` | G raster line | [E850-verified] | n = n1 + 256·n2 bytes that follow. Brother sends 47h ('G') with real PackBits; 70 bytes per line after decompression. Uncompressed: k must be 70. |
| `5A` | Z zero line | [sibling-documented] | One blank raster line. |
| `0C` | Print (more pages follow) | [sibling-documented] | |
| `1A` | Print with feed (last page) | [E850-verified] | Last byte of every complete Editor job. |
| `1B 69 21 n` | ESC i ! auto status notification | [sibling-documented] | n=0 notify, 1 off (default). Documented for P910BT; on P900 the 0x80 flag of ESC i z already enables notifications. |
| `1B 69 58 47` | ESC i X G extended info | [observed-elsewhere] | Undocumented; P-touch Editor 5.x sends it at start. ASCII INI reply. |
| `1B 69 55` +15 | ESC i U job tag | [E850-verified] present; meaning unknown | Sent right after `ESC i a 01`, before `ESC i z`. Payload `4A 00 0C 04 42 1A B3 D4 9C 00 00 NN 00 00 00`; NN equals the LPR job number (05,06,07,08,0B,0C in captures 2-3), the rest was constant across all jobs from one PC. Needed by the printer? Unknown. |
| `1B 69 6B 63 01 00` | ESC i k | [E850-verified] present; meaning unknown | In every Editor job, between `ESC i K` and `ESC i d`. Not in the P900 reference. |

Anything else seen in an E850TKW capture: add it here with the exact bytes and context.

## 2. ESC i z — print information (10 parameter bytes)

| Byte | Meaning |
|---|---|
| 0 | Valid-flags (**P-touch Editor sends 0x84**, i.e. width + recovery, with media type 0x00 for TZe 36 mm; 0x86 is our HSe assumption):  0x02 media type, 0x04 media width, 0x08 media length, 0x40 quality priority, 0x80 printer recovery (always set) |
| 1 | Media type **for ESC i z**: 0x00 laminated/non-laminated tape (TZe, probably HGe), 0x11 HSe 2:1, 0x17 HSe 3:1, 0x13 FLe. NB: differs from the *status* code for tape (0x01). P900 doc also mentions 0x09 for hi-res/draft on laminated tape — unclear, verify. |
| 2 | Media width in mm (status-code value, e.g. 9 for 8.8mm HSe) |
| 3 | Media length in mm, 0 = continuous (TZe/HSe are continuous) |
| 4–7 | Number of raster lines, little-endian u32 |
| 8 | Page: 0 first, 1 middle, 2 last (P900 doc). **E850-verified: a single-page job sends 2** (all six captured jobs); ptouch-rs sends 0. Multi-page values still unverified. |
| 9 | 0 |

Tag: [E850-verified] for TZe 36 mm single page (bytes `84 00 24 00 <lines LE> 02 00`). Byte 0 with 0x80 ("printer recovery") also turns on
bidirectional status notifications during printing on the P900 family. If PI_KIND /
PI_WIDTH are set and the loaded media does not match, the printer answers with error
info 2 bit 0 (wrong media) — a built-in safety net we should use (send 0x86).
P900 doc example for 24mm/100mm on a 180 dpi model: `1B 69 7A 84 00 18 00 9C 02 00 00 00 00`.

## 3. Mode bytes

**ESC i M n** — 0x40 auto-cut, 0x80 mirror. [sibling-documented]

**ESC i K n** [sibling-documented / observed-elsewhere]
- 0x01 draft (P900 family)
- 0x04 half-cut (P900 family; not P710BT)
- 0x08 chain printing OFF ⇒ i.e. "cut at end / feed after last label". Brother docs phrase
  this as *no chain printing*; LPrint calls it "cut at end". Same bit.
- 0x10 special tape (no cutting) — used for some special media; check HSe captures.
- 0x40 high-resolution (P900 family: 360×720 dpi)
- 0x80 do not clear expansion buffer

**ESC i A n** — cut every n labels (1–99). [sibling-documented]

**ESC i d n1 n2** — feed margin in dots. P900 docs: minimum ~14 dots, P-touch Editor
often uses 14 (≈1mm) for TZe. HSe values unknown → verify.

## 4. Compression and raster lines

`M 02` enables TIFF PackBits per line:
- header byte h, 0..127: copy next h+1 literal bytes
- h, 129..255 (i.e. −127..−1 signed): repeat next byte 257−h times
- 128: no-op
Each `g` line compresses independently; decompressed length must equal the head's
bytes-per-line. An all-zero line may be sent as `Z` instead (saves bytes).

Bit order: MSB of the first byte is the first pin. Which end of the head the first pin
is (tape top vs bottom) determines mirror/flip — verify with an asymmetric test image.

## 5. Status reply — 32 bytes

| Byte | Field | Notes |
|---|---|---|
| 0 | 0x80 print head mark | |
| 1 | 0x20 size | |
| 2 | 0x42 'B' | |
| 3 | Series code | e.g. '0' (0x30) for PT |
| 4 | Model code | **E850TKW value unknown — record it** |
| 5 | Country code | 0x30 |
| 6 | Battery level | 0 full, 1 half, 2 low, 3 needs charge, 4 on AC (P900 family) |
| 7 | Extended error | 0x10 FLe end, 0x1D hi-res/draft error, 0x1E adapter, incompatible media = 0x21 (P900 doc) / 0x1F (LPrint) |
| 8 | Error info 1 | 0x01 no media, 0x02 end of media, 0x04 cutter jam, 0x08 low batt, 0x10 in use, 0x20 off, 0x40 HV adapter, 0x80 fan |
| 9 | Error info 2 | 0x01 wrong media, 0x02 expansion buf full, 0x04 comm error, 0x08 comm buf full, 0x10 cover open, 0x20 overheat/cancel, 0x40 feed error, 0x80 system error |
| 10 | Media width (mm) | §6 |
| 11 | Media type | §6 |
| 12–14 | colours/fonts | 0 |
| 15 | Mode | last ESC i M |
| 16 | Density | |
| 17 | Media length | 0 continuous |
| 18 | Status type | 0 reply, 1 printing done, 2 error, 3 exit IF, 4 off, 5 notification, 6 phase change |
| 19 | Phase type | 0 editing/receiving, 1 printing |
| 20–21 | Phase number LE | |
| 22 | Notification | 1 cover open, 2 cover closed, 3 cooling start, 4 cooling end |
| 23 | 0 | |
| 24 | Tape colour | 0x01 white, 0x03 clear, 0x70 white heat-shrink, … (full list in ptouch_common.py) |
| 25 | Text colour | 0x08 black, 0x01 white, … |
| 26–31 | hw info / reserved | undocumented |

Tag: [sibling-documented] + [observed-elsewhere]. Unsolicited status replies (status type
1/2/5/6) arrive during printing; a reader must handle them, not just the reply to ESC i S.

## 6. Media codes

Media width byte: 0 none, 4 → 3.5mm, 6 → 6mm / 5.8mm HS, 9 → 9mm / 8.8mm HS, 12 → 12mm /
11.7mm HS, 18 → 18mm / 17.7mm HS, 24 → 24mm / 23.6mm HS, 36 → 36mm.

Media type byte: 0x00 none, 0x01 laminated (TZe/HGe), 0x03 non-laminated, 0x04 fabric,
0x11 heat-shrink 2:1 (HSe), 0x13 FLe, 0x14 flexible ID, 0x15 satin, 0x17 heat-shrink 3:1,
0xFF incompatible.

E850TKW media (spec sheet): TZe 3.5/6/9/12/18/24/36mm, HGe 6–36mm, HSe 5.8/8.8/11.7/17.7/23.6mm.
Whether HGe reports 0x01 or something else: **unknown, verify**.

## 7. Head geometry — TZe 36 mm E850-verified, rest from the P900 doc

560 pins, 70 bytes per raster line, pin 0 = MSB of the first byte (§2.3.5). With
compression the decompressed line is always 70 bytes. The print band is **not centred**.

**E850-verified (TZe 36 mm, captures 2 and 3, 2026-10-06):** 70 bytes per line, and P-touch Editor inks pins **61..=514** (454 pins), i.e. 61 leading zero pins in byte order. The P900 doc lists 45 left / 61 right for 36 mm, so **the doc's columns are the reverse of byte order**: the number of leading pins in a raster line is the doc's *right* margin. The table below keeps the doc's columns; the code (`tape::head_band_560`) stores the flipped value. Other rows [sibling-documented] and presumed to flip the same way.

Orientation (F capture, correct print): top of the label = lowest pin, first raster line = left edge of the label.

| Media | Left margin pins | Print pins | Right margin pins |
|---|---|---|---|
| TZe 3.5mm | 248 | 48 | 264 |
| TZe 6mm | 240 | 64 | 256 |
| TZe 9mm | 219 | 106 | 235 |
| TZe 12mm | 197 | 150 | 213 |
| TZe 18mm | 155 | 234 | 171 |
| TZe 24mm | 112 | 320 | 128 |
| TZe 36mm | 45 | 454 | 61 |
| HSe 5.8mm | 244 | 56 | 260 |
| HSe 8.8mm | 224 | 96 | 240 |
| HSe 11.7mm | 206 | 132 | 222 |
| HSe 17.7mm | 166 | 212 | 182 |
| HSe 23.6mm | 144 | 256 | 160 |

(HS 3:1 rows exist in the P900 doc but E850TKW spec only lists HSe 2:1 sizes.)

Lengths and margins (§2.3.3–2.3.4), 360×360 dpi:
- Margin (ESC i d): min 14 dots (1mm), max 1800 dots. Hi-res doubles these (28 / 3600).
- Print length: TZe 57–14173 dots (4mm–1000mm); HSe 60–7087 dots (4.2mm–500mm).
- Physical minimum tape fed out is 27mm (cutter position): shorter data still uses 27mm.

How to verify (no printing needed): capture a P-touch Editor job of a full-bleed black
rectangle for each media you own, run `parse_stream.py job.bin`, and compare
`ink_span_pins` with this table. Mark rows [E850-verified] here and flip `verified` in
`ptouch_common.py::HEAD_GEOMETRY`.

## 7b. Print flow rules (P900 doc §1, §5)

- Read status once before sending. **No command — not even ESC i S — may be sent while
  printing**; wait for status "printing completed" (and phase change back to receiving)
  before the next page/job.
- USB + uncompressed raster ⇒ *concurrent printing* (starts before the print command);
  compressed or network ⇒ *buffered printing* (one page received first). Prefer
  PackBits: errors are cleaner to recover.
- On error the printer clears everything received; resend from the page whose
  "Printing" phase was not seen.
- Network (TCP 9100) per doc: data "simply sent as is" by the port monitor; the doc does
  not promise status replies over TCP — test it (open question).

## 7c. USB (Appendix A, P900 family)

VID 0x04F9; P900 0x2083, P900W 0x2085, P950NW 0x2086, P910BT 0x20C7. E850TKW PID unknown.
Printer class, one interface, EP1 bulk IN (status), EP2 bulk OUT (data), 64-byte
packets, full speed.

## 8. ESC i X G

Undocumented query. Reply is an INI-like ASCII block (printer name, firmware versions,
counters, network config), possibly preceded by a short binary prefix and spread over
several USB reads. Useful for model detection and showing firmware version in the UI.

## 9. Open questions (move to §10 when answered)

- Model code byte (status byte 4) for E850TKW. (Still open: no status block obtained; TCP 9100 stays silent.)
- USB PID of the E850TKW.
- Confirm the §7 table for TZe 3.5/6/9/12/18/24 and HSe (36 mm done: 70 bytes/line, pins 61..514).
- ~~`G` (47h) vs `g` (67h)~~ answered: 47h.
- Exact `ESC i z` bytes 0, 1, 8 and the `ESC i K` value P-touch Editor uses for TZe vs HSe.
- What P-touch Editor sends that the doc calls "internal" (job ID, copy count) — ignorable?
- Does HSe need the "special tape" bit (0x10) or a different margin?
- Does the printer accept jobs while the tube engine is selected? (Only to detect &
  refuse — we never drive the tube engine.)
- ~~Is TCP 9100 behaviour identical to USB~~ answered: Windows does not print via 9100 at all. It prints by **LPR, TCP 515, queue `BINARY_P1`** and reads status by **SNMP, UDP 161**. See evidence log.
- Does the printer send unsolicited status over TCP while printing? (LPR replies are 1-byte acks only; status is polled by SNMP hrPrinterStatus.)
- What are `ESC i U` (job tag) and `ESC i k 63 01 00` for? Can they be omitted?
- Where does the media TYPE (byte 11) come from over the network? Windows reads no OID for it; the printer validates width itself via the 0x04 flag in `ESC i z`.

## 10. Evidence log

Format: `YYYY-MM-DD | fact | tag change | evidence (capture file / test print)`

2026-10-06 | Device identity: web UI reports "Brother PT-E850TKW", firmware 1.59, network FW 1.00; SNMP sysDescr "Brother NC-27036w"; IEEE1284 id `MFG:Brother;CMD:PJL;MDL:PT-E850TKW;CLS:PRINTER;` | n/a | live queries, Wi-Fi Direct (192.168.118.1) and infrastructure (192.168.99.107)
2026-10-06 | TCP 9100 is open (Bonjour `_pdl-datastream._tcp`, `_printer._tcp`) but sent NO reply to `ESC i S` (alone, after `ESC @`, after 200x00+`ESC @`, waited up to 10 s) nor to PJL `INFO ID/STATUS/CONFIG` | §9 "TCP status replies": answer NO for TCP 9100 on FW 1.59 (Wi-Fi Direct and infrastructure) (also none after `ESC i a 01` and after 200x00+`ESC @`+`ESC i a 01`, 8 s each; afterwards the web UI showed Device Status ERROR and the panel displayed "download failed, cannot receive file from other product"; trigger is either the PJL INFO probes or the ESC i a 01 probe, not isolated. DO NOT send PJL or bare ESC i a 01 / ESC i S probes to 9100 again; only a complete Brother-style job should ever be sent) | send_raw.py + inline probes; both Wi-Fi modes
2026-10-06 | SNMP v1 `public` answers: hrPrinterStatus idle(3), error state 00; Brother private MIB 1.3.6.1.4.1.2435.2.3.9.4.2.1.5.5.8.0 = `00 01 04 00 00 00 00 FF`, `.5.5.10.0` = TLV-like blob (not decoded; media type byte 11 NOT yet obtained) | n/a | snmpwalk
2026-10-06 | Print path of P-touch Editor/Windows driver: **LPR (RFC 1179) to TCP 515, queue `BINARY_P1`**; data file = the raw job; control file `H<host> P<user> J<doc> l/U/N dfA...`; printer replies only 1-byte acks. Status polled with **SNMP UDP 161**: hrPrinterStatus (1.3.6.1.2.1.25.3.5.1.1.1: 3 idle, 4 printing), hrPrinterDetectedErrorState, Brother serial 1.3.6.1.4.1.2435.2.3.9.4.2.1.5.5.1.0, IEEE1284 id via 1.3.6.1.4.1.2699.1.2.1.2.1.1.3.1. No OID for media type. | §9 TCP question: answered | fixtures/captures/{1,2,3}.pcapng
2026-10-06 | TZe 36 mm job header (cassette TZe-S661, 1.4" paper size, half-cut, standard 360x360, feed 0.04"): `00`x200 `1B 40` `1B 69 61 01` `1B 69 55`+15 `1B 69 7A 84 00 24 00 <lines LE> 02 00` `1B 69 4D 40` `1B 69 41 01` `1B 69 4B 0C` `1B 69 6B 63 01 00` `1B 69 64 0E 00` `4D 02` raster `1A` | §1/§2 rows -> [E850-verified] | fixtures/jobs/cap2_*, cap3_011I/012I (Rust test `test_job_p900_header_matches_p_touch_editor_capture`)
2026-10-06 | Raster: 70 bytes/line, `G` with PackBits; 36 mm ink span pins 61..=514 (454); F glyph upright with pin 0 at top and line index to the right | §7 36 mm [E850-verified], doc table columns are reversed vs byte order | fixtures/jobs/cap2_005I (rect), cap3_011I (F)
2026-10-06 | cap3_008I is an aborted job (stream cut, 622 of 1069 lines) and A009 an empty one; identical jobs 005/006 and 011/012 are reprints | n/a | fixtures/captures/3.pcapng
2026-10-06 | **First print from our own stack succeeded**: `ptouch print "F" -p 40 --precut --host 192.168.99.107` on TZe 36 mm (TZe-S661). 240 raster lines, 12329-byte job (200x00 + ESC @ + header incl. ESC i U + ESC i k, as in the Brother captures) sent by LPR to queue BINARY_P1; printer went printing -> idle; label correct (F upright, fully inside the tape, auto-cut) | TZe 36 mm end to end: [E850-verified]: band pins 61..=514 and orientation, header bytes, LPR path. Still unknown whether ESC i U / ESC i k are required (we always send them). | physical label, confirmed by Yuri
2026-10-06 | The printer's embedded web server (debut/1.30) sends Content-Length but does NOT close the connection; reading to EOF stalls ~6 s and makes the single-threaded stack slow to answer SNMP. Read by Content-Length and close. | n/a | `ptouch info --host` 6/6 runs at ~0.3 s after the fix
2026-10-07 | Physical label length = data length + 2 mm: a 50 mm page printed 52 mm and an 80 mm page 82 mm (ESC i d 14 dots = ~1 mm at each end, split unverified) | §3 ESC i d: margin adds to the label length at both ends | measured by Yuri on TZe 36 mm via the CUPS queue
2026-10-07 | **Print quality modes from P-touch Editor (TZe 36 mm, cap_highquarity/cap_highres/cap_highspeed):** (a) "priority to print quality": `ESC i z` flags **0xC4** (0x40 added), type 00, raster unchanged, K 04, d 14; (b) **high resolution 360x720**: `ESC i z` flags 0x86 + media type **0x09**, K **0x44** (bit 6), **d 28**, **2x raster lines** (645 vs 323 for the same design); (c) high speed (draft): flags 0x86 type 09, K 05 (bit 0), d **7**, 0.5x lines (162). All keep `ESC i k 63 01 00`. In these captures Editor sent `ESC i M 00` (no auto cut) and no `ESC i A`; that is a cut setting, independent of quality. | §2 flags/type, §3 K/d: quality modes [E850-verified]; draft captured but not offered | fixtures/jobs/cap_highquarity.bin, cap_highres.bin, cap_highspeed.bin (Rust golden tests for the first two)
2026-10-07 | **TZe 9 mm capture (cap_9mm.bin)**: `ESC i z 86 09 09 00 6f 01 00 00 02 00`, K 44, d 1c: it was printed in HIGH RESOLUTION (367 lines), ink on pins 265..322 for an F that is not full-bleed. 9 mm band (235..340 after the flip) is consistent but NOT discriminating; needs a full-bleed black rectangle on 9 mm | §7 9 mm: still [sibling-documented] | fixtures/jobs/cap_9mm.bin
2026-10-07 | Calibration label (calibrate-36x75-portrait.pdf) on P75: measured top 3 mm, bottom 3 mm, total length 75.2 mm: the printer's feed margin is symmetric (no lead/trail correction needed). The 3 mm vs the ~5 mm predicted is unexplained (measurement or label edge definition) | n/a | measured by Yuri
2026-10-07 | **Printed on TZe 36 mm via the CUPS queue:** high quality (flag 0xC4) and high resolution 360x720 (jobs 355-357): printer accepted both, no error, high resolution label correct length and position, printer slower; sharpness gain over normal is small (not distinguishable side by side by eye) as expected since only the feed axis is 720 dpi | quality modes: header + physical result [E850-verified] | physical labels, confirmed by Yuri
