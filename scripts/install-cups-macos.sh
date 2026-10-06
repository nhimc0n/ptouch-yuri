#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Install the Brother PT-E850TKW as a normal macOS printer (CUPS queue).
#
#   scripts/install-cups-macos.sh <printer-host-or-ip>     # install
#   scripts/install-cups-macos.sh --uninstall              # remove queue and files
#
# Needs administrator rights (sudo). It builds the filter, copies it and the
# PPD to /Library/Printers/PTouchE850 and creates the queue "PT-E850TKW" that
# sends jobs by LPR to queue BINARY_P1 on the printer, as the Windows driver does.
set -euo pipefail

NAME="PT-E850TKW"
DEST="/Library/Printers/PTouchE850"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

if [ "${1:-}" = "--uninstall" ]; then
    sudo lpadmin -x "$NAME" 2>/dev/null || true
    sudo rm -rf "$DEST"
    echo "Removed $NAME"
    exit 0
fi

HOST="${1:?usage: $0 <printer-host-or-ip> | --uninstall}"

echo "This will:"
echo "  - build rastertoptouch (cargo build --release)"
echo "  - copy it and the PPD to $DEST"
echo "  - create a print queue '$NAME' -> lpd://$HOST/BINARY_P1"
read -r -p "Continue? [y/N] " answer
[ "$answer" = "y" ] || { echo "Cancelled"; exit 1; }

cargo build --release -p ptouch-cups --manifest-path "$ROOT/Cargo.toml"

sudo mkdir -p "$DEST"
sudo cp "$ROOT/target/release/rastertoptouch" "$DEST/rastertoptouch"
sudo cp "$ROOT/data/cups/PT-E850TKW.ppd" "$DEST/PT-E850TKW.ppd"
sudo chown -R root:wheel "$DEST"
sudo chmod 755 "$DEST" "$DEST/rastertoptouch"
sudo chmod 644 "$DEST/PT-E850TKW.ppd"

sudo lpadmin -p "$NAME" -E -v "lpd://$HOST/BINARY_P1" -P "$DEST/PT-E850TKW.ppd" \
    -D "Brother PT-E850TKW" -L "Label printer"
echo "Done. Open any app, choose Print, and select '$NAME'."
echo "Use a 36 mm cassette. The printer itself rejects a label whose width does not match the loaded tape."
