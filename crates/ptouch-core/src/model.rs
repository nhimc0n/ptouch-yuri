// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Huang Rui <vowstar@gmail.com>
// SPDX-FileCopyrightText: Dominic Radermacher and the ptouch-print contributors
//
// Portions derived from ptouch-print, licensed GPL-3.0-or-later:
// https://git.familie-radermacher.ch/linux/ptouch-print.git

//! Model capabilities independent of connection identifiers.

use crate::{
    device::{DeviceFlags, DeviceInfo},
    tape,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dialect {
    Usb,
    P300Bt,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ModelProfile {
    pub(crate) dialect: Dialect,
    pub(crate) name: &'static str,
    pub(crate) raster_width_px: u16,
    pub(crate) dpi: u16,
    pub(crate) flags: DeviceFlags,
}

impl From<&DeviceInfo> for ModelProfile {
    fn from(info: &DeviceInfo) -> Self {
        Self {
            dialect: Dialect::Usb,
            name: info.name,
            raster_width_px: info.max_px,
            dpi: info.dpi,
            flags: info.flags,
        }
    }
}

impl ModelProfile {
    #[cfg(any(test, all(feature = "bluetooth", target_os = "macos")))]
    pub(crate) const P300BT: Self = Self {
        dialect: Dialect::P300Bt,
        name: "PT-P300BT",
        raster_width_px: 128, // raster transfer width, separate from printable width
        dpi: 180,
        flags: DeviceFlags::NONE,
    };

    /// Printable band for the loaded media (status bytes 11 and 10), when the
    /// model places the band by per-media offsets instead of centring it.
    pub(crate) fn head_band(self, media_type: u8, width_mm: u8) -> Option<tape::HeadBand> {
        if self.dialect == Dialect::Usb && self.flags.contains(DeviceFlags::P900_RASTER) {
            tape::head_band_560(media_type, width_mm)
        } else {
            None
        }
    }

    pub(crate) fn tape_width_px(self, media_type: u8, width_mm: u8) -> Option<u16> {
        match self.dialect {
            Dialect::Usb if self.flags.contains(DeviceFlags::P900_RASTER) => self
                .head_band(media_type, width_mm)
                .map(|band| band.print_pins),
            Dialect::Usb => {
                tape::tape_pixels(width_mm, self.dpi).map(|px| px.min(self.raster_width_px))
            }
            // Only this tape profile has been physically verified so far.
            Dialect::P300Bt if width_mm == 12 => Some(64),
            Dialect::P300Bt => None,
        }
    }
}
