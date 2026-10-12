#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Build the macOS installer: dist/PT-E850TKW-<version>.dmg
#
#   scripts/build-installer.sh
#
# The DMG holds "Cài đặt PT-E850TKW.pkg" (driver + settings app + a script that
# creates the print queue and finds the printer with Bonjour), a script to
# uninstall, and a short read-me. Apple Silicon only (the Rust target installed
# here is aarch64-apple-darwin).
#
# Optional signing, if you have an Apple Developer ID (without it macOS asks the
# user to right-click > Open the first time):
#   SIGN_APP_ID="Developer ID Application: Name (TEAM)"   signs the app
#   SIGN_PKG_ID="Developer ID Installer: Name (TEAM)"     signs the installer
# Notarization is not done here.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP_NAME="PT-E850TKW.app"
PKG_ID="vn.fiora.ptouch-e850.pkg"
VERSION="$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['version'])" "$ROOT/settings-app/src-tauri/tauri.conf.json")"
OUT="$ROOT/dist"
WORK="$OUT/build"
APP="$ROOT/settings-app/src-tauri/target/release/bundle/macos/$APP_NAME"
DMG="$OUT/PT-E850TKW-$VERSION.dmg"

command -v cargo-tauri >/dev/null || { echo "cargo-tauri is missing: cargo install tauri-cli"; exit 1; }
if pgrep -f "cargo-tauri tauri dev" >/dev/null; then
    echo "Stop 'cargo tauri dev' first: it shares the target directory and corrupts this build."
    exit 1
fi

echo "== 1/5 print filter"
cargo build --release -p ptouch-cups --manifest-path "$ROOT/Cargo.toml"

echo "== 2/5 settings app"
# macOS 27's linker sometimes writes proc-macro dylibs that dlopen rejects
# (E0463). Remove unloadable ones and retry; see CLAUDE.md.
built=""
for attempt in 1 2 3 4; do
    for f in "$ROOT"/settings-app/src-tauri/target/release/deps/*.dylib; do
        [ -e "$f" ] || continue
        python3 -c "import ctypes,sys; ctypes.CDLL(sys.argv[1])" "$f" 2>/dev/null || rm -f "$f"
    done
    if (cd "$ROOT/settings-app/src-tauri" && cargo build --bins --features tauri/custom-protocol --release); then
        built=yes
        break
    fi
    echo "build attempt $attempt failed, retrying"
done
[ -n "$built" ] || { echo "the settings app did not build"; exit 1; }
(cd "$ROOT/settings-app" && cargo tauri bundle --bundles app)
codesign --force --deep -s "${SIGN_APP_ID:--}" "$APP"
codesign --verify --deep --strict "$APP"

echo "== 3/5 payload"
rm -rf "$WORK"
mkdir -p "$WORK/payload/Library/Printers/PTouchE850" "$WORK/payload/Applications" "$WORK/pkg" "$WORK/dmg"
cp "$ROOT/target/release/rastertoptouch" "$WORK/payload/Library/Printers/PTouchE850/"
cp "$ROOT/data/cups/PT-E850TKW.ppd" "$WORK/payload/Library/Printers/PTouchE850/"
ditto "$APP" "$WORK/payload/Applications/$APP_NAME"

echo "== 4/5 installer package"
pkgbuild --analyze --root "$WORK/payload" "$WORK/components.plist" >/dev/null
# Keep the app where we put it (no relocation to an older copy elsewhere) and
# always overwrite it, even when the version number is the same.
python3 - "$WORK/components.plist" <<'PY'
import plistlib, sys
path = sys.argv[1]
with open(path, "rb") as f:
    components = plistlib.load(f)
for component in components:
    component["BundleIsRelocatable"] = False
    component["BundleIsVersionChecked"] = False
    component["BundleOverwriteAction"] = "upgrade"
with open(path, "wb") as f:
    plistlib.dump(components, f)
PY
pkgbuild --root "$WORK/payload" --component-plist "$WORK/components.plist" \
    --identifier "$PKG_ID" --version "$VERSION" --install-location / \
    --ownership recommended --scripts "$ROOT/installer/scripts" "$WORK/pkg/component.pkg"
sed "s/__VERSION__/$VERSION/" "$ROOT/installer/distribution.xml" >"$WORK/distribution.xml"
PRODUCT="$WORK/dmg/Cài đặt PT-E850TKW.pkg"
if [ -n "${SIGN_PKG_ID:-}" ]; then
    productbuild --distribution "$WORK/distribution.xml" --resources "$ROOT/installer/resources" \
        --package-path "$WORK/pkg" --sign "$SIGN_PKG_ID" "$PRODUCT"
else
    productbuild --distribution "$WORK/distribution.xml" --resources "$ROOT/installer/resources" \
        --package-path "$WORK/pkg" "$PRODUCT"
fi

echo "== 5/5 disk image"
cp "$ROOT/installer/uninstall.command" "$WORK/dmg/Gỡ cài đặt.command"
chmod 755 "$WORK/dmg/Gỡ cài đặt.command"
cat >"$WORK/dmg/Đọc tôi - Read me.txt" <<EOF
PT-E850TKW $VERSION  (Brother PT-E850TKW label printer for macOS, Apple Silicon)

TIẾNG VIỆT
1. Bật máy in và nối cùng mạng với máy tính.
2. Bấm đúp "Cài đặt PT-E850TKW.pkg" và làm theo hướng dẫn (cần mật khẩu quản trị).
   Nếu macOS báo không mở được vì chưa xác minh: bấm chuột phải vào file, chọn Mở, rồi Mở.
   Hoặc vào Cài đặt hệ thống > Quyền riêng tư & Bảo mật > Vẫn mở.
3. Mở ứng dụng PT-E850TKW trong Launchpad để chọn kiểu cắt, chất lượng, khổ nhãn.
4. Gỡ cài đặt: bấm đúp "Gỡ cài đặt.command".

ENGLISH
1. Turn the printer on and connect it to the same network as this computer.
2. Double-click "Cài đặt PT-E850TKW.pkg" and follow the steps (needs an administrator password).
   If macOS says it cannot be opened because it is not verified: right-click the file, choose Open, then Open.
   Or System Settings > Privacy & Security > Open Anyway.
3. Open the PT-E850TKW app from Launchpad to choose the cut, quality and label size.
4. To uninstall, double-click "Gỡ cài đặt.command".
EOF
mkdir -p "$OUT"
rm -f "$DMG"
hdiutil create -volname "PT-E850TKW" -srcfolder "$WORK/dmg" -ov -format UDZO "$DMG" >/dev/null

echo
echo "Built $DMG"
ls -lh "$DMG" | awk '{print "size: " $5}'
shasum -a 256 "$DMG"
