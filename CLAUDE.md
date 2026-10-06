# pte850 — macOS printing for Brother PT-E850TKW (fork of ptouch-rs)

Print to a Brother PT-E850TKW from macOS without Brother's Windows software.
This repository is a fork of https://github.com/vowstar/ptouch-rs (Rust CLI + egui GUI
for P-touch printers). We add E850TKW support to it instead of writing a new stack.
Owner: Yuri (Fiora Việt Nam). Talk to Yuri in Vietnamese; write code, comments,
commit messages and docs in English.

## Scope

- IN: the **label engine** — TZe (3.5–36mm), HGe, HSe heat-shrink tube.
- OUT: the **PVC ferrule/tube engine**. Do not implement, probe, or send commands to it.
  If the printer reports it is in tube mode, detect and refuse with a clear message.
- OUT: firmware updates, writing printer settings, P-touch Template storage.

## Protocol truth lives in one place

Use the `ptouch-e850-dev` skill (`.claude/skills/ptouch-e850-dev/`) for anything about
bytes on the wire. `references/raster-protocol.md` in that skill is the single source of
truth; every fact is tagged `[E850-verified]`, `[sibling-documented]` or
`[observed-elsewhere]`.

- Brother publishes no command reference for the E850TKW. Values borrowed from the
  PT-P900 family are assumptions until a capture or test print proves them.
- Code that depends on an unverified value must say so in a comment
  (`// UNVERIFIED(E850): head geometry borrowed from PT-P900`) so it is greppable.
- When something gets verified: update the tag + evidence log in `raster-protocol.md`,
  update `scripts/ptouch_common.py` tables, update the Rust constant, add/refresh the
  fixture test — in the same commit.

## Repository layout (upstream ptouch-rs + our additions)

```
crates/
  ptouch-core/      USB transport, protocol, device + tape tables, status, session   GPL-3.0+
  ptouch-render/    bitmap, text, images, .ptl layouts, CSV templates               MIT (raster.rs GPL)
  ptouch-cli/       `ptouch` binary                                                  MIT
  ptouch-gui/       egui app                                                         MIT
fixtures/                                   (ours)
  captures/         raw .pcapng from P-touch Editor (git-lfs)
  jobs/             extracted .bin + <name>.notes.md (media, settings, result)
.claude/skills/ptouch-e850-dev/             (ours) protocol reference + Python tools
CLAUDE.md                                   (ours)
```

Remotes: `upstream` = vowstar/ptouch-rs, `origin` = our fork. Rebase on upstream
regularly; keep E850TKW work as small commits that could become upstream PRs.

## Work plan (gaps vs. upstream — details in the skill's references/prior-art.md)

1. Device entry for PT-E850TKW in `ptouch-core/src/device.rs`: max_px 560, dpi 360,
   PackBits, ESC i z, precut. USB PID: read from the real device.
2. Per-media pin offsets for the 560-pin head (P900 table, raster-protocol.md §7);
   `ptouch-render/src/raster.rs` currently centres the band — must use the table.
3. HSe media (0x11) in `tape.rs` and in `cmd_info` (ESC i z media type + flags 0x86).
4. `TcpTransport` (port 9100) implementing the `Transport` trait in `session.rs`;
   `--host` option in the CLI and a host field in the GUI connection selector.
5. Fiora additions after the above work: cable-marking presets (HSe sequences
   L1-01…), Vietnamese UI strings, bundled Vietnamese-capable font.

Not planned: CUPS/PAPPL integration (later, separate decision), PVC tube engine (never).

## Licensing

Anything linking `ptouch-core` is GPL-3.0-or-later. That is fine for internal use and a
public fork. Do not copy GPL code into Fiora's closed-source projects. Every new source
file gets an SPDX header matching its crate's licence, as upstream does.

## Commands

```
cargo build --release --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cargo run -p ptouch-cli -- list
cargo run -p ptouch-cli -- info                                  # USB
cargo run -p ptouch-cli -- print "TEST" -o /tmp/t.png -w 150     # no printer needed

# Python reference tools (stdlib; Pillow for PNG)
S=.claude/skills/ptouch-e850-dev/scripts
python3 $S/parse_stream.py fixtures/jobs/<job>.bin [--png out.png] [--json]
python3 $S/decode_status.py <hex | file>
python3 $S/build_job.py --test-pattern rect --media tze12 -o /tmp/rect.bin
python3 $S/send_raw.py <printer-ip>          # status only; printing needs --print FILE --confirm
```

macOS USB: remove any print queue for the printer in System Settings, otherwise
claiming the interface fails (busy/access error).

## Rust conventions

- Follow upstream style: edition 2024, `cargo fmt`, clippy clean, errors via the
  crate's existing `Error` type, no `unwrap()` outside tests.
- Extend existing tables/flags (`DeviceFlags`, `TapeInfo`, `cmd_*` builders) rather than
  adding parallel code paths; add a new `DeviceFlags` bit only when behaviour truly differs.
- Every protocol constant added for the E850TKW cites the reference section it comes
  from, and carries `// UNVERIFIED(E850)` until a capture or test print confirms it.
- Unit tests next to the code, mirroring upstream (`test_cmd_*`, `test_job_*`).

## Testing

- **Golden tests**: for each fixture in `fixtures/jobs/`, `ptouch-core`'s job builder given
  the same design + settings must produce the same header commands and the same
  decompressed raster (compare SHA-256 of raster, like `parse_stream.py --json`).
  PackBits run choices may differ.
- Python `build_job.py` is the reference encoder: when Rust and Python disagree, find out
  which matches the Brother capture; fix the other.
- Status decoder tests use real 32-byte replies captured from the printer.
- Fake printer for integration tests: TCP listener on localhost that records the job
  and answers `1B 69 53` with a canned status block.

## Hardware rules (real media costs real money)

- Never send a job without first reading status and checking that loaded media type and
  width match the job's `ESC i z`. The transport layer enforces this; do not add bypasses.
- Experiment on 12mm TZe, labels ≤ 30mm long. HSe only after the TZe path is verified.
- Do not run a print against the real printer from an automated test or without Yuri
  explicitly asking for that print in the current session.

## Current status

Phase 5 — first real print done. `ptouch print "F" -p 40 --precut --host <ip>` printed a correct
label on **TZe 36 mm** (cassette TZe-S661): upright, inside the tape, auto-cut. Printer is on Wi-Fi
infrastructure at 192.168.99.107 (set a DHCP reservation). USB is not working from the Mac yet.

**How it works** (verified, see `raster-protocol.md` §10): print by **LPR, TCP 515, queue
`BINARY_P1`**; state by **SNMP** (hrPrinterStatus); media width from the web page
(`/general/status.html`; the server ignores EOF, read by Content-Length). Raw TCP 9100 never
answers and probing it (PJL, bare `ESC i a 01`) put the printer into an error — do NOT probe 9100.
36 mm band = pins 61..=514. Rust header matches Brother byte-for-byte (golden test).
Code: `ptouch-core/src/network.rs` (`NetworkPrinter`), CLI `--host`.

**Open / next:**
- TZe 9 mm: capture (or careful test print) to verify its band (currently the P900 table flipped, unverified).
- Are `ESC i U` / `ESC i k 63 01 00` required? (we always send them)
- Chain printing / multiple labels per job over LPR is blocked until captured (CSV and `--copies` > 1 error out).
- HSe tube: untested, web page format unknown.
- GUI still centres the band and has no network target; USB PID still a placeholder (0).
- Commit in small pieces (device, geometry, header, network) — nothing is committed yet.
Update this section when the phase changes.
