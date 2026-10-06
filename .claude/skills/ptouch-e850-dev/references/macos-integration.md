# macOS integration

A "printer driver" on macOS is user-space software that turns a page into the printer's
command stream and delivers it. No kernel extension, no DriverKit.

## Transports

**TCP 9100 (raw)** — simplest and the default for development. Open a socket, write the
job, read status replies on the same socket. Discover the printer with Bonjour
(`dns-sd -B _pdl-datastream._tcp`) or SNMP if it advertises; otherwise a configured IP.

**USB** — Brother vendor ID `0x04F9`; the E850TKW product ID is unknown until read from
the device: `system_profiler SPUSBDataType` or `ioreg -p IOUSB -l -w0 | grep -i -A20 brother`.
Record it in `raster-protocol.md`. ptouch-rs already does this with `rusb` (vendored libusb): claim the printer-class
interface, write to bulk OUT, read status from bulk IN with retries (first reads may be
empty). If CUPS currently has a queue printing to the same device, access will be
contended — remove the macOS print queue for the printer (ptouch-rs README) or use TCP.

**SNMP** (UDP 161) — Brother printers expose status via the standard Printer MIB and
Brother private OIDs. Optional; ESC i S over the job channel is enough.

## Integration options

1. **ptouch-rs fork: library + CLI** (`ptouch-core`, `ptouch`) — built first; everything
   else wraps it. See `prior-art.md`.
2. **PAPPL / LPrint printer application** — recommended route to "print from any app".
   LPrint (Michael Sweet) is a PAPPL-based label printer application that already has a
   Brother PT/QL driver (`lprint-brother.c`). Adding the E850TKW there means: model
   entry, media list (TZe + HSe sizes), head geometry table, and any header differences
   we verify. macOS sees it as an IPP Everywhere / AirPrint printer — no PPD, no
   deprecated CUPS driver APIs. Upstreaming to LPrint is a nice option to keep in mind.
   If we prefer our own Rust code path: a small PAPPL app in C calling the Rust encoder
   through a C ABI (`cbindgen`), or a pure-Rust IPP server (more work).
3. **Classic CUPS filter + PPD** — `rastertopte850` filter reading CUPS raster. Works
   today but Apple/CUPS have deprecated PPD printer drivers; avoid for new work unless a
   quick hack is needed.
4. **Desktop GUI** — ptouch-rs already ships an egui GUI (`ptouch-gui`) with preview,
   layouts and CSV batch; extend it first. A separate Tauri app only if the egui UI
   cannot reach the cable-labelling UX we want.

Suggested order: 1 → 4 (extend existing GUI) → 2 (system-wide printing).

## Packaging

- CLI: Homebrew tap or plain signed binary.
- App: Developer ID signing + notarization (`xcrun notarytool`), hardened runtime.
  Network client entitlement for TCP; USB access needs `com.apple.security.device.usb`
  if sandboxed (Tauri apps are usually not sandboxed outside the App Store).
- PAPPL app: runs as a LaunchAgent/LaunchDaemon; needs local network permission on
  recent macOS for Bonjour.

## Testing on macOS without printing

- Unit tests compare encoder output to the golden `.bin` fixtures (decompressed raster
  hash + header bytes).
- A fake printer: `nc -l 9100 > got.bin` lets the CLI "print" to localhost; then run
  `parse_stream.py got.bin`. Status replies can be faked with a tiny Python server that
  writes a canned 32-byte block after receiving `1B 69 53`.
