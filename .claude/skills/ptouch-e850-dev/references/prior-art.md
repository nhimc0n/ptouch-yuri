# Prior art: what to reuse, what to change

Reviewed 2026-10-06. Re-check upstream before relying on details below.

## vowstar/ptouch-rs — the base we build on

Rust workspace (edition 2024), CLI + egui GUI, builds on macOS with vendored libusb.
Version 0.8.x at review time.

```
crates/ptouch-core    USB transport, protocol, device/tape tables, status, session  (GPL-3.0+)
crates/ptouch-render  bitmap, text, images, layouts (.ptl), CSV templating          (MIT, except raster.rs = GPL)
crates/ptouch-cli     `ptouch` binary                                                (MIT)
crates/ptouch-gui     egui app with live preview                                     (MIT)
```

Already solved there (do not rewrite): text/image rendering with rotation and auto font
size, `.ptl` layouts with `{{placeholders}}`, CSV batch printing as one continuous strip,
copies, chain print, half-cut / cut-at-end (`cmd_advanced_mode`), hi-res/draft quality
on 360 dpi models, PackBits, status parsing incl. media type 0x11, a session layer that
waits for "printing completed" with idle timeouts and never auto-retries, export to PNG
without a printer, macOS .app packaging.

What it does NOT do (our work, in this order):

1. **No 560-pin / PT-P900-class device.** The device table tops out at 384 px
   (PT-9500PC/9700PC). Add an E850TKW entry: `max_px: 560`, `dpi: 360`, flags
   `RASTER_PACKBITS | USE_INFO_CMD | HAS_PRECUT` (+ whatever captures show). PID unknown
   until read from the device.
2. **Band placement is centred.** `ptouch-render/src/raster.rs::bitmap_to_raster_lines`
   computes `offset = max_px/2 - height/2` and fills bytes from the *end* of the line
   (bottom-to-top). On the P900 head the band is asymmetric (12mm: 197 left / 213 right),
   so centring is off by ~8 pins (≈0.56mm), worse for 36mm (45/61). Add an explicit
   per-media left-margin table (raster-protocol.md §7) and use it when the device has
   one. Keep their bit-order convention consistent — convert "left margin from pin 0
   (MSB of first byte)" carefully into their end-relative indexing and test against a
   capture.
3. **Tape table is generic 360 dpi** (`TAPE_TABLE_360`: 12mm=150, 18mm=234, 24mm=320,
   36mm=454 — these match the P900 doc; 3.5mm=48, 6mm=64, 9mm=106 also match). No HSe
   entries: add HSe 5.8/8.8/11.7/17.7/23.6 keyed by (media_type 0x11, width).
4. **ESC i z sends media type 0 and flags 0** (`cmd_info`). For HSe we must send 0x11 and
   should set valid-flags 0x86 so the printer itself rejects a media mismatch. Page byte
   n9 is hard-coded 0 (2 only for D460BT) — align with what captures show.
5. **USB only.** `Transport` is a `pub(crate)` trait in `session.rs` with
   `send/receive/close`; add a `TcpTransport` (port 9100) inside `ptouch-core` and a
   `--host` CLI option. Bluetooth (macOS RFCOMM) exists for PT-P300BT — irrelevant here.
6. **macOS USB caveat** (from its README): claiming fails if the printer is added as a
   print queue in System Settings — remove the queue or use TCP.

### Licence consequence (decide consciously)

`ptouch-core` and `raster.rs` are GPL-3.0-or-later, so any binary linking them is GPL.
Fine for internal use and for publishing the fork. If Fiora ever wants to ship a closed
product, rewrite `ptouch-core` + `raster.rs` clean-room from Brother's documentation
(our Python tools + raster-protocol.md are enough) and keep the MIT crates.

### Upstream etiquette

Keep E850TKW changes as small, reviewable commits (device entry, geometry table, HSe
media, TCP transport) so they can be offered upstream as PRs. Put Fiora-specific UI
(Vietnamese strings, cable-marking presets) in separate crates/commits.

## DavidPhillipOster/ptouch-print-macOS — reference only

Objective-C/C Xcode port of Dominic Radermacher's `ptouch-print` (GPL-3): libusb sources
bundled, libgd replaced by Core Graphics, fonts from Font Book. Supports only older
128-pin 180 dpi models (PT-2420PC/1230PC/2430PC/2730/P700/D450).

Useful for: how to render with Core Graphics / Core Text on macOS (if a native Swift
front-end is ever wanted), and PLite-mode gotcha (P700 shows a different PID until the
PLite button is held ~2s — check whether the E850TKW has any "editor lite"/mass-storage
mode that changes its USB PID). Not useful as a code base: no 360 dpi / 560-pin
support, no status-driven flow, no HSe.

## Others worth knowing

- LPrint (PAPPL, C) — Brother driver notes are our status/command tables' second source;
  the route to "print from any macOS app" later.
- printer-driver-ptouch (CUPS, C), rasterprynt (Python), pyPTouch / `ptouch` (Python) —
  the Python `ptouch` package already models HSe tubes for P900-class printers and is a
  good cross-check for media tables.
