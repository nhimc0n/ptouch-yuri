// SPDX-License-Identifier: GPL-3.0-or-later

//! Turn CUPS raster pages into PT-E850TKW label jobs.
//!
//! macOS renders a PDF into a CUPS raster at the resolution and page size from
//! the PPD (`cgpdftoraster`). The PPD declares each label as a **landscape**
//! page (length x tape width), so raster x is the feed direction and raster y
//! is across the tape, which is the layout the printer encoder was verified
//! with (letter F capture). This crate reads that raster and builds the job.

use ptouch_core::{device, tape};
use ptouch_render::{bitmap::LabelBitmap, raster};
use std::io::Read;

/// Errors are shown to the user in the print queue, so keep them plain.
pub type Result<T> = std::result::Result<T, String>;

/// Print resolution of the label engine in dots per inch.
pub const DPI: u32 = 360;
/// Shortest label the printer accepts, in dots (TZe, PT-P900 reference).
pub const MIN_LENGTH_DOTS: u32 = 57;
/// Longest label, in dots (TZe, PT-P900 reference; UNVERIFIED(E850)).
pub const MAX_LENGTH_DOTS: u32 = 14173;
const HEADER_LEN: usize = 1796;
const CSPACE_W: u32 = 0;
const CSPACE_K: u32 = 3;
const CSPACE_SW: u32 = 18;

/// One raster page reduced to 8-bit luminance (255 = white, 0 = black).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// Pixels along the feed direction.
    pub width: u32,
    /// Pixels across the tape.
    pub height: u32,
    /// Horizontal resolution in dpi.
    pub dpi_x: u32,
    /// Vertical resolution in dpi.
    pub dpi_y: u32,
    /// `width * height` luminance bytes, row by row.
    pub pixels: Vec<u8>,
}

fn u32_at(buf: &[u8], offset: usize, little_endian: bool) -> u32 {
    let bytes: [u8; 4] = buf[offset..offset + 4].try_into().unwrap_or([0; 4]);
    if little_endian {
        u32::from_le_bytes(bytes)
    } else {
        u32::from_be_bytes(bytes)
    }
}

/// Read every page of an uncompressed CUPS raster stream (sync word `RaS3`).
///
/// macOS' `cgpdftoraster` writes version 3. Compressed version 2 and the
/// version 1 header are not needed for this printer and are refused.
pub fn read_raster<R: Read>(mut input: R) -> Result<Vec<Page>> {
    let mut sync = [0u8; 4];
    input
        .read_exact(&mut sync)
        .map_err(|e| format!("cannot read raster: {e}"))?;
    let little_endian = match &sync {
        b"3SaR" => true,
        b"RaS3" => false,
        b"2SaR" | b"RaS2" => return Err("compressed CUPS raster (v2) is not supported".into()),
        _ => return Err("input is not a CUPS raster stream".into()),
    };
    let mut pages = Vec::new();
    loop {
        let mut header = vec![0u8; HEADER_LEN];
        let mut filled = 0;
        while filled < HEADER_LEN {
            match input.read(&mut header[filled..]) {
                Ok(0) if filled == 0 => return Ok(pages),
                Ok(0) => return Err("truncated raster page header".into()),
                Ok(n) => filled += n,
                Err(e) => return Err(format!("cannot read raster: {e}")),
            }
        }
        let u = |offset| u32_at(&header, offset, little_endian);
        let (dpi_x, dpi_y) = (u(276), u(280));
        let (width, height) = (u(372), u(376));
        let (bits_per_color, bits_per_pixel, bytes_per_line) = (u(384), u(388), u(392));
        let color_space = u(400);
        if u(404) != 0 {
            return Err("compressed raster data is not supported".into());
        }
        let (invert, gray) = match (color_space, bits_per_color, bits_per_pixel) {
            (CSPACE_W | CSPACE_SW, 8, 8) => (false, true),
            (CSPACE_K, 8, 8) => (true, true),
            _ => (false, false),
        };
        if !gray {
            return Err(format!(
                "unsupported raster format (colorspace {color_space}, {bits_per_pixel} bit); choose Grayscale"
            ));
        }
        if width == 0
            || height == 0
            || bytes_per_line < width
            || width > 1 << 20
            || height > 1 << 20
        {
            return Err("raster page has invalid dimensions".into());
        }
        let mut pixels = Vec::with_capacity((width * height) as usize);
        let mut line = vec![0u8; bytes_per_line as usize];
        for _ in 0..height {
            input
                .read_exact(&mut line)
                .map_err(|_| "truncated raster page data".to_string())?;
            let row = &line[..width as usize];
            if invert {
                pixels.extend(row.iter().map(|v| 255 - v));
            } else {
                pixels.extend_from_slice(row);
            }
        }
        pages.push(Page {
            width,
            height,
            dpi_x,
            dpi_y,
            pixels,
        });
    }
}

/// Tape width in millimetres that a page of this height (in dots at 360 dpi)
/// is meant for, within a few dots of tolerance.
///
/// macOS emits either the full page (36 mm is 510 dots) or, when the PPD
/// declares an imageable area, only that area, which the PPD sets to the
/// printable band (454 dots). Both are accepted. Only widths with a known band
/// on the E850TKW qualify.
pub fn tape_width_mm_for_height(height: u32) -> Option<u8> {
    let close = |dots: f64| (f64::from(height) - dots).abs() <= 4.0;
    [4u8, 6, 9, 12, 18, 24, 36].into_iter().find(|&mm| {
        tape::head_band_560(0x01, mm).is_some_and(|band| {
            close(f64::from(mm) * f64::from(DPI) / 25.4) || close(f64::from(band.print_pins))
        })
    })
}

/// A label ready for the encoder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    /// Tape the page was laid out for.
    pub tape_mm: u8,
    /// 70-byte raster lines, one per dot along the feed.
    pub lines: Vec<Vec<u8>>,
}

/// Convert a page to raster lines. Ink is any pixel darker than mid-gray.
///
/// The page height must match a supported tape; the printable band is cut from
/// the middle of the page (the PPD leaves the unprintable edge as margin).
pub fn page_to_label(page: &Page) -> Result<Label> {
    if page.dpi_x != DPI || page.dpi_y != DPI {
        return Err(format!(
            "page resolution is {}x{} dpi; the PT-E850TKW needs {DPI}x{DPI}",
            page.dpi_x, page.dpi_y
        ));
    }
    let tape_mm = tape_width_mm_for_height(page.height).ok_or_else(|| {
        format!(
            "page height {} dots does not match a supported tape width (36 mm is 454 or 510 dots)",
            page.height
        )
    })?;
    if page.width < MIN_LENGTH_DOTS {
        return Err(format!(
            "label is too short: {} dots, minimum {MIN_LENGTH_DOTS}",
            page.width
        ));
    }
    if page.width > MAX_LENGTH_DOTS {
        return Err(format!(
            "label is too long: {} dots, maximum {MAX_LENGTH_DOTS}",
            page.width
        ));
    }
    let band = tape::head_band_560(0x01, tape_mm).ok_or("tape has no known print band")?;
    let band_px = u32::from(band.print_pins);
    if page.height < band_px {
        return Err("page is narrower than the printable band".into());
    }
    let first_row = (page.height - band_px) / 2;
    let mut bitmap = LabelBitmap::new(page.width, band_px);
    for y in 0..band_px {
        let row = ((first_row + y) * page.width) as usize;
        for x in 0..page.width {
            if page.pixels[row + x as usize] < 128 {
                bitmap.set_pixel(x, y, true);
            }
        }
    }
    let lines = raster::bitmap_to_raster_lines_at(&bitmap, 560, band.left_pins);
    Ok(Label { tape_mm, lines })
}

/// Cut behaviour chosen in the print dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutMode {
    /// Cut through the label and backing after printing.
    Full,
    /// Cut the label but keep the backing (E850-verified).
    Half,
}

/// Print options from the CUPS command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// Cut behaviour.
    pub cut: CutMode,
    /// Copies requested by the application.
    pub copies: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            cut: CutMode::Half,
            copies: 1,
        }
    }
}

/// Parse the CUPS option string (`key=value key2=value2 flag`).
pub fn parse_options(text: &str) -> Options {
    let mut options = Options::default();
    for token in text.split_whitespace() {
        let (key, value) = token.split_once('=').unwrap_or((token, ""));
        match (
            key.to_ascii_lowercase().as_str(),
            value.to_ascii_lowercase().as_str(),
        ) {
            ("cutmode", "full") => options.cut = CutMode::Full,
            ("cutmode", "half") => options.cut = CutMode::Half,
            ("copies", n) => options.copies = n.parse().unwrap_or(1).max(1),
            _ => {}
        }
    }
    options
}

/// Build the full job. Nothing here contacts the printer: CUPS runs filters
/// in a sandbox without network access, so the tape check is left to the
/// printer, which rejects a job whose `ESC i z` width flag (0x84) does not match
/// the loaded cassette.
///
/// `job_number` goes into the ESC i U job tag like P-touch Editor does.
pub fn build_job_offline(
    label: &Label,
    options: &Options,
    job_number: Option<u8>,
) -> Result<Vec<u8>> {
    let info =
        device::find_device_by_name("PT-E850TKW").ok_or("PT-E850TKW is not in the device table")?;
    let job_options = ptouch_core::protocol::JobOptions {
        media_width: label.tape_mm,
        media_type: 0x01,
        precut: true,
        half_cut: options.cut == CutMode::Half,
        margin_dots: ptouch_core::protocol::MIN_MARGIN_DOTS,
        job_number,
        ..Default::default()
    };
    let chunks = ptouch_core::protocol::build_print_job(&label.lines, info.flags, &job_options);
    if chunks.is_empty() {
        return Err("unsupported media for this job".into());
    }
    let mut data = ptouch_core::protocol::cmd_init_p900();
    data.extend(chunks.into_iter().flatten());
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic v3 little-endian raster with one gray page.
    fn raster_bytes(width: u32, height: u32, dpi: u32, fill: impl Fn(u32, u32) -> u8) -> Vec<u8> {
        let mut out = b"3SaR".to_vec();
        let mut header = vec![0u8; HEADER_LEN];
        let mut put = |offset: usize, value: u32| {
            header[offset..offset + 4].copy_from_slice(&value.to_le_bytes())
        };
        put(276, dpi);
        put(280, dpi);
        put(372, width);
        put(376, height);
        put(384, 8);
        put(388, 8);
        put(392, width);
        put(400, CSPACE_W);
        out.extend(header);
        for y in 0..height {
            for x in 0..width {
                out.push(fill(x, y));
            }
        }
        out
    }

    fn page(width: u32, height: u32, fill: impl Fn(u32, u32) -> u8) -> Page {
        read_raster(&raster_bytes(width, height, 360, fill)[..])
            .unwrap()
            .remove(0)
    }

    #[test]
    fn reads_pages_and_rejects_foreign_input() {
        let pages = read_raster(&raster_bytes(8, 4, 360, |x, _| x as u8)[..]).unwrap();
        assert_eq!(pages.len(), 1);
        assert_eq!(
            (pages[0].width, pages[0].height, pages[0].dpi_x),
            (8, 4, 360)
        );
        assert_eq!(pages[0].pixels[3], 3);
        assert!(read_raster(&b"%PDF-1.4"[..]).is_err());
        assert!(
            read_raster(&b"2SaR"[..])
                .unwrap_err()
                .contains("compressed")
        );
        let mut truncated = raster_bytes(8, 4, 360, |_, _| 0);
        truncated.truncate(truncated.len() - 3);
        assert!(read_raster(&truncated[..]).is_err());
        assert!(read_raster(&b"3SaR"[..]).unwrap().is_empty());
    }

    #[test]
    fn tape_width_is_derived_from_page_height() {
        assert_eq!(tape_width_mm_for_height(510), Some(36));
        assert_eq!(tape_width_mm_for_height(512), Some(36));
        assert_eq!(tape_width_mm_for_height(454), Some(36)); // imageable area = band
        assert_eq!(tape_width_mm_for_height(106), Some(9));
        assert_eq!(tape_width_mm_for_height(127), Some(9));
        assert_eq!(tape_width_mm_for_height(300), None);
    }

    #[test]
    fn page_becomes_70_byte_lines_with_the_verified_band() {
        // 36 mm page: ink on the whole page height -> pins 61..=514 only.
        let label = page_to_label(&page(100, 510, |_, _| 0)).unwrap();
        assert_eq!(label.tape_mm, 36);
        assert_eq!(label.lines.len(), 100);
        assert!(label.lines.iter().all(|l| l.len() == 70));
        let line = &label.lines[0];
        let pins: Vec<usize> = (0..560)
            .filter(|p| line[p / 8] & (0x80 >> (p % 8)) != 0)
            .collect();
        assert_eq!((pins[0], *pins.last().unwrap()), (61, 514));
        assert_eq!(pins.len(), 454);
    }

    #[test]
    fn band_sized_page_needs_no_cropping() {
        let label = page_to_label(&page(100, 454, |_, _| 0)).unwrap();
        assert_eq!(label.tape_mm, 36);
        let pins: Vec<usize> = (0..560)
            .filter(|p| label.lines[0][p / 8] & (0x80 >> (p % 8)) != 0)
            .collect();
        assert_eq!((pins[0], *pins.last().unwrap(), pins.len()), (61, 514, 454));
    }

    #[test]
    fn orientation_matches_the_letter_f_capture() {
        // top row of the page -> lowest pin, first column -> first line.
        let label = page_to_label(&page(
            100,
            510,
            |x, y| if x == 0 && y == 28 { 0 } else { 255 },
        ))
        .unwrap();
        let first = &label.lines[0];
        assert!(
            first[61 / 8] & (0x80 >> (61 % 8)) != 0,
            "page row 28 is the first band row"
        );
        assert!(label.lines[1].iter().all(|b| *b == 0));
    }

    #[test]
    fn rejects_pages_the_printer_cannot_take() {
        assert!(
            page_to_label(&page(100, 300, |_, _| 255))
                .unwrap_err()
                .contains("supported tape")
        );
        assert!(
            page_to_label(&page(20, 510, |_, _| 255))
                .unwrap_err()
                .contains("too short")
        );
        let mut low_res = page(100, 510, |_, _| 255);
        low_res.dpi_x = 300;
        assert!(page_to_label(&low_res).unwrap_err().contains("360"));
    }

    #[test]
    fn options_are_parsed() {
        assert_eq!(parse_options("").cut, CutMode::Half);
        assert_eq!(
            parse_options("CutMode=Full copies=1 foo").cut,
            CutMode::Full
        );
        assert_eq!(parse_options("copies=3").copies, 3);
    }

    #[test]
    fn offline_job_starts_like_a_brother_job() {
        let label = page_to_label(&page(100, 510, |x, _| if x < 50 { 0 } else { 255 })).unwrap();
        let job = build_job_offline(&label, &Options::default(), Some(7)).unwrap();
        assert_eq!(&job[..200], &[0u8; 200][..]);
        assert_eq!(&job[200..202], &[0x1B, 0x40]);
        assert_eq!(&job[202..206], &[0x1B, 0x69, 0x61, 0x01]);
        assert_eq!(*job.last().unwrap(), 0x1A);
        let full = build_job_offline(
            &label,
            &Options {
                cut: CutMode::Full,
                copies: 1,
            },
            None,
        )
        .unwrap();
        assert!(job.windows(4).any(|w| w == [0x1B, 0x69, 0x4B, 0x0C]));
        assert!(full.windows(4).any(|w| w == [0x1B, 0x69, 0x4B, 0x08]));
    }
}
