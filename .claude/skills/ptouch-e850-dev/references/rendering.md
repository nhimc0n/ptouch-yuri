# Rendering labels for the raster engine

## Coordinate systems

Design space: a label is *L* mm long (feed direction) × *W* mm high (tape width).
Printer space: one raster line per dot of length (360 per inch at normal resolution),
each line is the full head width in pins; the tape's printable band is a contiguous
range of pins (head geometry table).

Pipeline: render design at 360 dpi → 1-bit image of size (length_dots × band_pins) →
rotate so length becomes the line index → place the band at the left-margin offset in a
full-head-width line → pack bits MSB-first → PackBits → `g` lines.

Mirror/flip conventions must come from the asymmetric-glyph fixture, not from guessing.
Encode the convention once in the library and test it against that fixture.

High-resolution mode (ESC i K 0x40, P900 family) doubles dots along the feed direction
(360×720). Treat it as a later feature; verify on E850TKW first.

## Text

- Vietnamese needs full coverage of Latin Extended Additional (ạ ả ấ ầ ẩ ẫ ậ ắ …). Safe
  bundled choices: Noto Sans / Noto Sans Mono, Be Vietnam Pro, Roboto. Do not rely on
  system fonts for reproducible output.
- Chinese labels: Noto Sans CJK SC/TC. Large file; bundle only if needed.
- Render text with anti-aliasing off or threshold at 50% — 1-bit thermal output looks
  better with hinted, non-AA glyphs at small sizes. Test 6mm and HSe 5.8mm specifically:
  the band is ~50 pins (~3.5mm), so 1 line of text at ~2.5mm cap height is the realistic max.
- Use a text shaping library (rustybuzz + ttf-parser/ab_glyph, or cosmic-text) so
  combining diacritics are positioned correctly.

## Images, barcodes, QR

- Images: Floyd–Steinberg or ordered dithering to 1-bit; let the user pick threshold.
- Barcodes/QR: render as exact modules aligned to whole dots; never scale a rasterised
  barcode — compute module size in dots (e.g. QR module = 4 dots ≈ 0.28mm).

## Heat-shrink tube (HSe)

- The printed face is flat in the cassette; after shrinking, text wraps around a
  cylinder. Keep text away from the band edges (≥ 0.5mm).
- HSe 2:1 shrinks to half diameter — text stays readable; very small fonts get distorted.
- Margins and minimum length may differ from TZe — take values from captures.
- Typical job: many short tubes, one per wire, sequential numbers. Batch printing with
  half-cut between labels and full cut at the end is the key workflow.

## Batch / cable-label workflow

Input: CSV or a pattern (`L{1..3}-{01..48}`, `X1:{1..24}`). Each row → one label.
Group into one job with ESC i A / half-cut so the printer does not re-feed margins
between every label. Show a preview strip before printing and a count of media length
needed.
