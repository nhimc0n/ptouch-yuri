#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Removes PT-E850TKW: the print queue, the driver files and the settings app.
# Double-click it, or run it from Terminal. It asks for your password (sudo).
echo "Gỡ cài đặt PT-E850TKW / Uninstall PT-E850TKW"
echo "Sẽ xoá máy in, driver và app. / This removes the printer, the driver and the app."
read -r -p "Tiếp tục? / Continue? [y/N] " answer
[ "$answer" = "y" ] || { echo "Đã huỷ / Cancelled"; exit 1; }

osascript -e 'tell application "PT-E850TKW" to quit' 2>/dev/null
sudo lpadmin -x PT-E850TKW 2>/dev/null
sudo rm -rf /Library/Printers/PTouchE850 /Applications/PT-E850TKW.app
sudo pkgutil --forget vn.fiora.ptouch-e850.pkg >/dev/null 2>&1
echo "Xong. / Done."
