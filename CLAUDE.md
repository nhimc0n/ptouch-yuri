# pte850 — macOS printing for Brother PT-E850TKW (fork of ptouch-rs)

Print to a Brother PT-E850TKW from macOS without Brother's Windows software.
This repository is a fork of https://github.com/vowstar/ptouch-rs (Rust CLI + egui GUI
for P-touch printers). We add E850TKW support to it instead of writing a new stack.
Owner: Yuri (Fiora Việt Nam). Talk to Yuri in Vietnamese; write code, comments,
commit messages and docs in English. **UI strings shown to end users are Vietnamese.**

## Scope

- IN: the **label engine** — TZe (3.5–36mm), HGe, HSe heat-shrink tube.
- IN: printing from any macOS app through a normal print queue, and a small **settings app**
  that configures the driver (see "Settings app plan").
- OUT: the **PVC ferrule/tube engine**. Do not implement, probe, or send commands to it.
  If the printer reports it is in tube mode, detect and refuse with a clear message.
- OUT: firmware updates, writing printer settings, P-touch Template storage.

## Protocol truth lives in one place

Use the `ptouch-e850-dev` skill (`.claude/skills/ptouch-e850-dev/`) for anything about
bytes on the wire. `references/raster-protocol.md` in that skill is the single source of
truth; every fact is tagged `[E850-verified]`, `[sibling-documented]` or
`[observed-elsewhere]`, and §10 is the dated evidence log.

- Brother publishes no command reference for the E850TKW. Values borrowed from the
  PT-P900 family are assumptions until a capture or test print proves them.
- Code that depends on an unverified value must say so in a comment
  (`// UNVERIFIED(E850): ...`) so it is greppable.
- When something gets verified: update the tag + evidence log in `raster-protocol.md`,
  update `scripts/ptouch_common.py` tables, update the Rust constant, add/refresh the
  fixture test — in the same commit.

## Repository layout (upstream ptouch-rs + our additions)

```
crates/
  ptouch-core/      protocol, device + tape tables, status, USB session, network.rs  GPL-3.0+
  ptouch-render/    bitmap, text, images, .ptl layouts, raster placement            MIT (raster.rs GPL)
  ptouch-cli/       `ptouch` binary (`--host` = network, full pre-flight)            MIT
  ptouch-gui/       upstream egui label editor (not E850-aware yet)                  MIT
  ptouch-cups/      `rastertoptouch` CUPS filter: print queue -> label job           GPL-3.0+
  ptouch-settings/  (planned) Tauri 2 settings app for the driver                    GPL-3.0+
data/cups/          PT-E850TKW.ppd (label sizes + driver options)
scripts/            install-cups-macos.sh (queue + filter, sudo) and upstream scripts
fixtures/
  captures/         raw .pcapng from P-touch Editor (LOCAL ONLY, git-ignored)
  jobs/             jobs extracted from captures + NOTES.md
  pdf/              test and calibration labels
.claude/skills/ptouch-e850-dev/   protocol reference + Python tools
```

Remotes: `upstream` = vowstar/ptouch-rs, `origin` = our fork (nhimc0n/ptouch-yuri).
Work happens on `feat/e850-network-printing` (pushed). Keep commits small.

## How printing works (all verified on the printer unless marked)

- **Transport**: the printer is on Wi-Fi infrastructure at 192.168.99.107 (needs a DHCP
  reservation). Windows/P-touch Editor prints by **LPR, TCP 515, queue `BINARY_P1`**, and reads
  state by **SNMP** (hrPrinterStatus: 3 idle, 4 printing, 1 = error shown on the panel).
  Raw TCP 9100 never answers; probing it (PJL, bare `ESC i a 01`) puts the printer into an
  error. **Do NOT probe 9100.** USB from the Mac has never worked (hub); PID still unknown.
- **Job**: 200x`00`, `ESC @`, `ESC i a 01`, `ESC i U` (job tag), `ESC i z`, `ESC i M`, `ESC i A`,
  `ESC i K`, `ESC i k 63 01 00`, `ESC i d`, `M 02`, `G` PackBits lines of 70 bytes, `1A`.
  Our header is byte-identical to Brother's (golden tests in `protocol.rs`).
- **Geometry**: 560 pins; TZe 36 mm = pins 61..=514, TZe 9 mm = pins 235..=340 (the P900 doc's
  left/right columns are reversed vs byte order). Top of the label = lowest pin, first raster
  line = leading edge. The printer adds a 14 dot (~1 mm) feed margin at each end.
- **Quality**: Normal (flags 84); High quality (flags C4, slower, same raster); High resolution
  360x720 (flags 86 + type 09, K bit 6, margin 28, twice the lines; laminated TZe only).
- **Cut**: half cut `K 0C` (printed OK), full cut `K 08` and no cut (`M 00`, no `ESC i A`,
  `K 04`) are built from captures but UNVERIFIED on the printer.
- **Two ways to print**:
  1. `ptouch print --host <ip>`: `NetworkPrinter` with a full pre-flight (model, idle, tape
     width == label, known band), LPR submit, SNMP wait.
  2. macOS print queue `PT-E850TKW`: `cgpdftoraster` renders the page -> `rastertoptouch` builds
     the job -> CUPS' `lpd` backend sends it. CUPS runs filters as root **in a sandbox without
     network**, so there is no pre-flight; the printer itself rejects a wrong tape width
     (observed: Device Status ERROR, SNMP other(1), nothing fed).
- **Pages**: landscape (length x tape) and portrait (tape x length, the usual label PDF) both
  work; portrait is rotated a quarter turn counterclockwise. `Auto*` sizes trim blank ends
  (2 mm kept); fixed/custom sizes drop the 1 mm feed margins so the label is exactly the chosen
  length. Blank labels, more than one page and copies > 1 are refused.

## Owner decisions (do not relitigate)

- **2026-10-06, no pre-flight in the CUPS path.** The sandbox makes it impossible; rely on the
  printer's own width check. This deviates from the hardware rule below on purpose.
- **2026-10-07, driver options are set in a settings app, not in the macOS print dialog.**
  The dialog proved unreliable for vendor options on macOS 27: they only appear in a separate
  "Printer Features" window, a choice with an empty PostScript code hides the whole option,
  running apps keep the first PPD they loaded, and even after quitting Brave the Print Quality
  option did not show. Do not spend more time on print-dialog UI.

## Settings app plan (current work)

Goal: an app like Brother's driver settings on Windows. Yuri picks the modes once; they stay in
the driver and apply to every print from any app. The app does not have to be running.

**Where settings live: the CUPS queue defaults.** `lpadmin -p PT-E850TKW -o Key=Value` rewrites
the `*Default<Key>` line of the queue's PPD. Yuri's account is in `_lpadmin`, so this needs no
password (checked: exit 0 without sudo). CUPS hands the filter `PPD=/private/etc/cups/ppd/
PT-E850TKW.ppd`, which the sandboxed filter can read. Resolution order in the filter:
**explicit job option (argv[5]) > queue default read from `$PPD` > built-in default.**
macOS apps only send the PPD options the user changed, so untouched options fall through to
the queue default (seen in job attributes of jobs 350/352).

Settings in v1 (each is a PPD option so `lp -o` keeps working):

| Setting | PPD key | UI control | Notes |
|---|---|---|---|
| Half cut | `HalfCut` | check box | default on; wins over Full cut |
| Full cut | `FullCut` | check box | neither ticked = no cut |
| Chain printing | `Chain` | check box, disabled | needs a capture first (see Open items) |
| Mirror | `MirrorPrint` | check box | filter reverses line order; skip if the job already has `mirror=true` (macOS flipped it) |
| Print quality | `LabelQuality` | 3-way choice | Normal / High / HiRes; HiRes also sets 720 dpi rendering through the PPD default |
| Default label size | `PageSize` | choice | Auto, Auto portrait, Auto 9 mm, fixed sizes |
| Printer address | device URI | text + find | `lpadmin -v lpd://<ip>/BINARY_P1` |

The app also shows live status (it is not sandboxed, so it can use `ptouch-core::network`:
idle/printing/error, loaded tape width), warns when the loaded tape does not match the default
label size, prints a test label through `NetworkPrinter` (the path with the full pre-flight),
and installs or repairs the queue (the only step that needs an admin prompt, because the filter
is copied to `/Library/Printers`).

**Stack: Tauri 2 + vanilla HTML/CSS/JS, no bundler** (`frontendDist` points at the UI folder),
the same stack as Yuri's ResBoost, in a new crate `crates/ptouch-settings` (GPL, links
`ptouch-core`). Tauri CLI 2.11, Node 24 and Xcode 27 are installed.

Phases, each ending in something Yuri can check:

1. **Driver reads queue defaults.** Parse `*Default<Key>` from `$PPD` in `ptouch-cups` (pure
   function, unit-tested); add `MirrorPrint`; apply the precedence above. Check with
   `lpadmin -o ...` then a `--dry-run`, then one real print per changed default.
2. **Queue access module.** Read/write the queue defaults and device URI (wrapping
   `lpoptions`/`lpadmin`), read status, find the printer by Bonjour (`_pdl-datastream._tcp`).
   Unit-test the parsing; no GUI yet.
3. **UI design.** Follow the UI skill's flow: brief, the main job of the screen, 2-3 wireframes
   with real content, Yuri picks one, then build. One window: status on one side, the option
   groups (Cut, Modes, Quality, Default size) on the other, Apply with a clear "applied" state,
   light and dark themes, Vietnamese strings. Yuri asked for the "UI UX Pro Max" standard; that
   skill is not installed in this workspace and not in the plugin catalogue, the installed
   equivalent is `evon:ui-ux` — use it unless Yuri installs the other one.
4. **Build the app.** Tauri commands `get_status`, `get_settings`, `apply_settings`,
   `test_print`, `install_driver`, `open_queue`.
5. **Package.** `.app` bundle with the filter, PPD and installer inside; app icon.
6. **Verify on the printer**, one print per mode, each only when Yuri asks.

Rules for the app code:

- Every Tauri command that waits on the network or a subprocess is `#[tauri::command(async)]`;
  a plain command runs on the main thread and freezes the window (lesson from ResBoost).
- The UI never builds printer bytes; it only reads/writes settings and calls `ptouch-core`.
- No new front-end framework or bundler. No `unwrap()` outside tests.
- The app must not print except through `test_print`, which keeps the pre-flight.

## Licensing

Anything linking `ptouch-core` is GPL-3.0-or-later. That is fine for internal use and a
public fork. Do not copy GPL code into Fiora's closed-source projects. Every new source
file gets an SPDX header matching its crate's licence, as upstream does.

## Commands

```
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cargo run -p ptouch-cli -- info --host 192.168.99.107          # read-only: SNMP + web page
cargo run -p ptouch-cli -- print "TEST" -o /tmp/t.png -w 454   # render only, no printer

# print queue: dry run without a printer (macOS renders, the filter builds the job)
cupsfilter -p data/cups/PT-E850TKW.ppd -m application/vnd.cups-raster -o PageSize=L50 in.pdf > in.ras
target/release/rastertoptouch --dry-run in.ras out.bin "PageSize=L50 HalfCut=False FullCut=True"
scripts/install-cups-macos.sh 192.168.99.107      # sudo; rebuilds, copies filter + PPD, recreates the queue
lpoptions -p PT-E850TKW -l                        # what the queue offers and its defaults
lpadmin -p PT-E850TKW -o FullCut=True             # change a driver default (no sudo for _lpadmin)

# Python reference tools (stdlib; Pillow for PNG)
S=.claude/skills/ptouch-e850-dev/scripts
python3 $S/parse_stream.py fixtures/jobs/<job>.bin [--png out.png] [--json]
python3 $S/extract_lpr.py capture.pcapng outdir/     # LPR jobs out of a capture, no tshark
python3 $S/snmp_dump.py capture.pcapng
python3 $S/build_job.py --test-pattern rect --media tze36 -o /tmp/rect.bin
```

After reinstalling the queue, quit and reopen any app whose print dialog should see the change.

## Rust conventions

- Follow upstream style: edition 2024, `cargo fmt`, clippy clean, errors via the
  crate's existing `Error` type, no `unwrap()` outside tests.
- Extend existing tables/flags (`DeviceFlags`, `TapeInfo`, `cmd_*` builders) rather than
  adding parallel code paths; add a new `DeviceFlags` bit only when behaviour truly differs.
- Every protocol constant added for the E850TKW cites its evidence and carries
  `// UNVERIFIED(E850)` until a capture or test print confirms it.
- Unit tests next to the code. Keep pure logic (parsing, geometry, option resolution) in
  functions that need no printer.
- PPD: every choice needs a non-empty code (`"<<>>setpagedevice"`), and UI text must not
  contain a colon (it ends the text and the rest is parsed as code).

## Testing

- **Golden tests** compare our bytes with Brother's captured jobs in `fixtures/jobs/`.
- `build_job.py` is the reference encoder: when Rust and Python disagree, find out which
  matches the capture and fix the other.
- `network.rs` is tested against a fake printer (LPR + SNMP + HTTP on localhost) that keeps the
  HTTP connection open like the real one.
- The print queue is tested with `cupsfilter` + `rastertoptouch --dry-run`, then `parse_stream.py`.
- Run `cargo test --workspace` for the full count: the CLI enables `ptouch-core`'s `bluetooth`
  feature, so `-p ptouch-core` alone runs two tests fewer.

## Hardware rules (real media costs real money)

- Never send a job without first reading status and checking that loaded media type and
  width match the job's `ESC i z`. The CLI/network path enforces this; do not add bypasses.
  (One owner-approved exception: the macOS print queue, see Owner decisions.)
- **Check the tape in its own step and read the result before any print command.** Chaining
  the check and the print in one shell command sent a 36 mm label to a 9 mm tape (2026-10-07).
- Only print when the printer reports idle/READY. If it reports an error, stop and ask.
- Keep test labels short (≤ 50 mm). HSe only after a capture.
- Do not print from an automated test, and not without Yuri asking for that print in the
  current session.

## Current status (2026-10-07)

Printing works end to end on TZe 36 mm from the CLI and from the macOS print queue (browser,
`lp`): correct orientation, position and length, half cut, High quality and High resolution all
printed. TZe 9 mm geometry is verified by capture but nothing has been printed on it yet.
**Settings app phase 1 is coded** (not yet installed or printed): the filter resolves options as
job option > queue default from `$PPD` > built-in (`resolve_options`, `defaults_from_ppd` in
`ptouch-cups`), and the PPD has `MirrorPrint`. Checked through `cupsfilter` with a PPD copy whose
defaults were changed: Full cut + Mirror + HiRes applied with an empty job option string, a job
option overrode one of them, and a job that names no page size is trimmed because the default is
`Auto`. Still to do for phase 1: reinstall, change a default with `lpadmin` on the real queue and
print once to confirm the sandboxed filter really reads `$PPD`.

**Printer state:** 9 mm tape is loaded and the printer has reported ERROR since a mismatched
36 mm test job; Yuri says the panel is clear but web/SNMP still say ERROR. Do not print until
`ptouch info --host` says Idle.

**Open items:**
- Settings app: phase 1 needs the on-printer check above; phase 2 (queue access module) is next.
- Chain printing: needs captures from P-touch Editor of (a) three labels with chain print on
  and (b) one label with chain print on.
- Full cut and No cut: never printed.
- Portrait pages: which way up the text prints is unverified.
- Are `ESC i U` / `ESC i k 63 01 00` required? (always sent)
- HSe tube: untested; how the web page names it is unknown.
- `ptouch-gui` (egui) still centres the band and has no network target.
- Apple warns that PPD printer drivers will stop working in a future CUPS; the long-term
  replacement is an IPP Everywhere printer application reusing `ptouch-core`/`ptouch-cups`.

Update this section when the phase changes.
