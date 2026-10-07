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

impl Page {
    /// Halve the number of rows (the axis across the tape) of a landscape page
    /// rendered at 720 dpi: the head has 360 dpi across, only the feed is finer.
    /// Each output pixel is the average of two input rows, so a one-dot line at
    /// 720 dpi still counts as ink.
    pub fn halved_across(&self) -> Page {
        let (w, h) = (self.width as usize, (self.height / 2) as usize);
        let mut pixels = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                let a = u16::from(self.pixels[(2 * y) * w + x]);
                let b = u16::from(self.pixels[(2 * y + 1) * w + x]);
                pixels.push(((a + b) / 2) as u8);
            }
        }
        Page {
            width: self.width,
            height: h as u32,
            dpi_x: self.dpi_x,
            dpi_y: self.dpi_y / 2,
            pixels,
        }
    }

    /// Turn a portrait page (tape width across, length down) into the landscape
    /// layout used everywhere else (length across, tape width down): the top of
    /// the portrait page becomes the leading edge (left) and its left edge
    /// becomes the bottom (highest pin), a quarter turn counterclockwise.
    ///
    /// UNVERIFIED(E850): which way up the printed text appears for portrait
    /// documents needs a test print.
    pub fn rotated_to_landscape(&self) -> Page {
        let (w, h) = (self.width as usize, self.height as usize);
        let mut pixels = vec![255u8; w * h];
        // new page: width h (feed), height w (across); new(xl, yl) = old(w-1-yl, xl)
        for yl in 0..w {
            for xl in 0..h {
                pixels[yl * h + xl] = self.pixels[xl * w + (w - 1 - yl)];
            }
        }
        Page {
            width: self.height,
            height: self.width,
            dpi_x: self.dpi_y,
            dpi_y: self.dpi_x,
            pixels,
        }
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
    /// Dots per inch along the feed: 360, or 720 for high resolution.
    pub feed_dpi: u32,
}

impl Label {
    /// Factor between this label's feed dots and the 360 dpi values used for
    /// minimum lengths and margins.
    pub fn scale(&self) -> usize {
        (self.feed_dpi / DPI).max(1) as usize
    }
}

/// Blank margin kept before and after the content in automatic length mode,
/// in dots (2 mm). The printer adds its own 14 dot feed margin on top.
pub const AUTO_MARGIN_DOTS: usize = 28;

impl Label {
    /// Shrink the label to its content: drop blank lines before the first and
    /// after the last ink, keeping `lead` and `trail` blank lines, and pad up
    /// to the printer's minimum length. Errors on a label with no ink at all.
    pub fn trim_to_content(&mut self, lead: usize, trail: usize) -> Result<()> {
        let inked = |line: &Vec<u8>| line.iter().any(|b| *b != 0);
        let first = self
            .lines
            .iter()
            .position(inked)
            .ok_or("the label is blank; nothing to print")?;
        let last = self.lines.iter().rposition(inked).unwrap_or(first);
        let start = first.saturating_sub(lead);
        let end = (last + 1 + trail).min(self.lines.len());
        self.lines = self.lines[start..end].to_vec();
        let blank = vec![0u8; self.lines[0].len()];
        let minimum = MIN_LENGTH_DOTS as usize * self.scale();
        while self.lines.len() < minimum {
            self.lines.push(blank.clone());
        }
        Ok(())
    }
}

/// Feed margin the printer adds before and after the printed data, in dots
/// (`ESC i d`, minimum 14 = 1 mm each). E850-verified by measurement: a 50 mm
/// page came out 52 mm and an 80 mm page 82 mm.
pub const FEED_MARGIN_DOTS: usize = ptouch_core::protocol::MIN_MARGIN_DOTS as usize;

impl Label {
    /// Make the physical label as long as the page by dropping the part of the
    /// page the printer's own feed margins already cover: `FEED_MARGIN_DOTS`
    /// lines from each end (never going below the printer's minimum length).
    /// Errors if that removes all the ink.
    ///
    /// UNVERIFIED(E850): that the two margins are one at each end; the total
    /// (+2 mm) is measured, their split is not.
    pub fn compensate_feed_margin(&mut self) -> Result<()> {
        let scale = self.scale();
        let spare = self
            .lines
            .len()
            .saturating_sub(MIN_LENGTH_DOTS as usize * scale);
        let crop = (2 * FEED_MARGIN_DOTS * scale).min(spare);
        let front = crop / 2;
        let end = self.lines.len() - (crop - front);
        self.lines = self.lines[front..end].to_vec();
        if self.lines.iter().all(|l| l.iter().all(|b| *b == 0)) {
            return Err("the label is blank once the 1 mm feed margins are removed".into());
        }
        Ok(())
    }
}

/// Convert a page to raster lines. Ink is any pixel darker than mid-gray.
///
/// The page height must match a supported tape; the printable band is cut from
/// the middle of the page (the PPD leaves the unprintable edge as margin).
pub fn page_to_label(page: &Page) -> Result<Label> {
    // 360 x 360 dpi, or 720 x 720 for high resolution (the head is 360 dpi
    // across the tape, so the cross axis is halved below; the feed stays 720).
    let scale = match (page.dpi_x, page.dpi_y) {
        (DPI, DPI) => 1,
        (720, 720) => 2,
        (x, y) => {
            return Err(format!(
                "page resolution is {x}x{y} dpi; the PT-E850TKW needs {DPI}x{DPI} or 720x720"
            ));
        }
    };
    // Documents come in two conventions: landscape (length x tape width) and
    // portrait (tape width x length, what most label PDFs use). Normalise to
    // landscape first.
    let rotated;
    let page = if tape_width_mm_for_height(page.height / scale).is_none()
        && tape_width_mm_for_height(page.width / scale).is_some()
    {
        rotated = page.rotated_to_landscape();
        &rotated
    } else {
        page
    };
    let halved;
    let page = if scale == 2 {
        halved = page.halved_across();
        &halved
    } else {
        page
    };
    let tape_mm = tape_width_mm_for_height(page.height).ok_or_else(|| {
        format!(
            "the page is {}x{} dots; one side must be the tape width (36 mm = 454 or 510 dots)",
            page.width, page.height
        )
    })?;
    // At 720 dpi a label has twice the lines for the same length, but the
    // printer's line limit is unchanged (UNVERIFIED(E850)), so it is shorter.
    if page.width < MIN_LENGTH_DOTS * scale as u32 {
        return Err(format!(
            "label is too short: {} dots, minimum {}",
            page.width,
            MIN_LENGTH_DOTS * scale as u32
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
    // A blank label is never intended and can waste up to a metre of tape,
    // typically because the document page does not match the label page
    // (portrait page on a landscape label, or content outside the band).
    if lines.iter().all(|line| line.iter().all(|b| *b == 0)) {
        return Err(
            "the label is blank: nothing falls inside the printable area. Check that the \
             paper size is the label size (36 mm on one side) and the content is not outside it"
                .into(),
        );
    }
    Ok(Label {
        tape_mm,
        lines,
        feed_dpi: DPI * scale as u32,
    })
}

/// Cut behaviour chosen in the print dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutMode {
    /// Cut through the label and backing after printing.
    Full,
    /// Cut the label but keep the backing (E850-verified).
    Half,
    /// Do not cut: the tape is fed out to tear off. Bytes copied from the
    /// quality captures (`ESC i M 00`, no `ESC i A`, `ESC i K 04`);
    /// UNVERIFIED(E850) that this is what Editor's "no cut" sends.
    None,
}

/// Print quality chosen in the print dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    /// 360 x 360 dpi (E850-verified).
    Normal,
    /// "Give priority to print quality": slower, same raster (E850-verified).
    High,
    /// 360 x 720 dpi: slowest, finest along the feed (E850-verified on TZe).
    HiRes,
}

/// Print options from the CUPS command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// Cut behaviour.
    pub cut: CutMode,
    /// Copies requested by the application.
    pub copies: u32,
    /// Size the label to its content instead of the page length
    /// (page size "Auto").
    pub auto_length: bool,
    /// Print quality.
    pub quality: Quality,
    /// Chain printing (no feed or cut at the end, for continuous labels).
    /// Not available yet: it needs a capture of a multi-label job.
    pub chain: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            cut: CutMode::Half,
            copies: 1,
            auto_length: false,
            quality: Quality::Normal,
            chain: false,
        }
    }
}

/// Value of a boolean option as CUPS writes it (`True`, `False`, `on`, ...).
fn truthy(value: &str) -> bool {
    matches!(value, "true" | "on" | "yes" | "1")
}

/// Parse the CUPS option string (`key=value key2=value2 flag`).
///
/// Cutting is two check boxes in the print dialog: `FullCut` and `HalfCut`
/// (half cut wins if both are ticked; neither means no cut). Mirror printing
/// is the standard "Flip horizontally" option, which macOS applies to the
/// raster itself, so it is deliberately not read here.
pub fn parse_options(text: &str) -> Options {
    let mut options = Options::default();
    let (mut half, mut full) = (true, false);
    for token in text.split_whitespace() {
        let (key, value) = token.split_once('=').unwrap_or((token, ""));
        match (
            key.to_ascii_lowercase().as_str(),
            value.to_ascii_lowercase().as_str(),
        ) {
            ("halfcut", v) => half = truthy(v),
            ("fullcut", v) => full = truthy(v),
            ("chain", v) => options.chain = truthy(v),
            // single-choice form, still accepted on the command line
            ("cutmode", "full") => (half, full) = (false, true),
            ("cutmode", "half") => (half, full) = (true, false),
            ("cutmode", "none") => (half, full) = (false, false),
            ("copies", n) => options.copies = n.parse().unwrap_or(1).max(1),
            ("pagesize", name) if name.starts_with("auto") => options.auto_length = true,
            ("labelquality", "normal") => options.quality = Quality::Normal,
            ("labelquality", "high") => options.quality = Quality::High,
            ("labelquality", "hires") => options.quality = Quality::HiRes,
            _ => {}
        }
    }
    options.cut = if half {
        CutMode::Half
    } else if full {
        CutMode::Full
    } else {
        CutMode::None
    };
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
    let hi_res = options.quality == Quality::HiRes;
    if hi_res != (label.feed_dpi == 720) {
        return Err(if hi_res {
            "High resolution was selected but the page was not rendered at 720 dpi".into()
        } else {
            "the page was rendered at 720 dpi but High resolution is not selected".into()
        });
    }
    let job_options = ptouch_core::protocol::JobOptions {
        media_width: label.tape_mm,
        media_type: 0x01,
        precut: options.cut != CutMode::None,
        no_cut: options.cut == CutMode::None,
        // "None" keeps the half-cut bit like the capture it is copied from.
        half_cut: options.cut != CutMode::Full,
        margin_dots: ptouch_core::protocol::MIN_MARGIN_DOTS * label.scale() as u16,
        job_number,
        quality: if hi_res {
            ptouch_core::PrintQuality::HighRes
        } else {
            ptouch_core::PrintQuality::Standard
        },
        quality_priority: options.quality == Quality::High,
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

    fn page_dpi(width: u32, height: u32, dpi: u32, fill: impl Fn(u32, u32) -> u8) -> Page {
        read_raster(&raster_bytes(width, height, dpi, fill)[..])
            .unwrap()
            .remove(0)
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
    fn portrait_pages_are_rotated_a_quarter_turn_counterclockwise() {
        // 36 mm wide (454 band) x 600 long portrait page, ink at its top-left corner
        let portrait = page(454, 600, |x, y| if x == 0 && y == 0 { 0 } else { 255 });
        let landscape = portrait.rotated_to_landscape();
        assert_eq!((landscape.width, landscape.height), (600, 454));
        // top of the portrait page -> leading edge (x = 0); its left edge -> bottom row
        assert_eq!(landscape.pixels[453 * 600], 0);
        assert_eq!(landscape.pixels.iter().filter(|p| **p == 0).count(), 1);
        // bottom-right corner -> top-right
        let br = page(454, 600, |x, y| if x == 453 && y == 599 { 0 } else { 255 })
            .rotated_to_landscape();
        assert_eq!(br.pixels[599], 0);
    }

    #[test]
    fn a_portrait_label_page_prints_like_its_landscape_twin() {
        let portrait = page(454, 600, |x, y| if y < 100 && x > 200 { 0 } else { 255 });
        let label = page_to_label(&portrait).unwrap();
        assert_eq!((label.tape_mm, label.lines.len()), (36, 600));
        let twin = page_to_label(&portrait.rotated_to_landscape()).unwrap();
        assert_eq!(label, twin);
        // a page with no side matching a tape width is still refused
        assert!(
            page_to_label(&page(300, 700, |_, _| 0))
                .unwrap_err()
                .contains("tape width")
        );
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
    fn rejects_a_blank_page_so_no_tape_is_wasted() {
        let err = page_to_label(&page(2000, 510, |_, _| 255)).unwrap_err();
        assert!(err.contains("blank"), "{err}");
        // one dark pixel inside the band is enough
        assert!(
            page_to_label(&page(2000, 510, |x, y| if x == 5 && y == 255 {
                0
            } else {
                255
            }))
            .is_ok()
        );
        // ink only in the 28 dot unprintable edge does not count
        assert!(page_to_label(&page(2000, 510, |_, y| if y < 10 { 0 } else { 255 })).is_err());
    }

    #[test]
    fn rejects_pages_the_printer_cannot_take() {
        assert!(
            page_to_label(&page(100, 300, |_, _| 255))
                .unwrap_err()
                .contains("tape width")
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

    fn label_with_ink_at(total: usize, ink: &[usize]) -> Label {
        let mut lines = vec![vec![0u8; 70]; total];
        for &i in ink {
            lines[i][8] = 0x10;
        }
        Label {
            tape_mm: 36,
            lines,
            feed_dpi: 360,
        }
    }

    #[test]
    fn trimming_keeps_margins_around_the_content() {
        let mut label = label_with_ink_at(1000, &[200, 500]);
        label.trim_to_content(28, 28).unwrap();
        // 200-28 .. 500+1+28  => 357 lines, ink at 28 and 328
        assert_eq!(label.lines.len(), 357);
        assert!(label.lines[28].iter().any(|b| *b != 0));
        assert!(label.lines[328].iter().any(|b| *b != 0));
        assert!(label.lines[0].iter().all(|b| *b == 0));
        assert!(label.lines[356].iter().all(|b| *b == 0));
    }

    #[test]
    fn trimming_clamps_at_the_edges_pads_short_labels_and_rejects_blank() {
        let mut label = label_with_ink_at(100, &[2, 4]);
        label.trim_to_content(28, 28).unwrap();
        // content 2..=4 plus 28 trailing = 33 lines, padded to the 57 dot minimum
        assert_eq!(label.lines.len(), MIN_LENGTH_DOTS as usize);
        assert!(label.lines[2].iter().any(|b| *b != 0));
        let mut tiny = label_with_ink_at(100, &[10]);
        tiny.trim_to_content(0, 0).unwrap();
        assert_eq!(tiny.lines.len(), MIN_LENGTH_DOTS as usize);
        let mut blank = label_with_ink_at(100, &[]);
        assert!(blank.trim_to_content(28, 28).unwrap_err().contains("blank"));
    }

    #[test]
    fn feed_margin_compensation_makes_the_label_as_long_as_the_page() {
        let mut label = label_with_ink_at(1000, &[14, 500, 985]);
        label.compensate_feed_margin().unwrap();
        assert_eq!(label.lines.len(), 1000 - 2 * FEED_MARGIN_DOTS);
        // content keeps its place relative to the label: page x=14 is now line 0
        assert!(label.lines[0].iter().any(|b| *b != 0));
        assert!(label.lines[500 - 14].iter().any(|b| *b != 0));
        assert!(label.lines.last().unwrap().iter().any(|b| *b != 0));
    }

    #[test]
    fn feed_margin_compensation_respects_the_minimum_length_and_blank_check() {
        let mut short = label_with_ink_at(70, &[30]);
        short.compensate_feed_margin().unwrap();
        assert_eq!(short.lines.len(), MIN_LENGTH_DOTS as usize);
        let mut edge_only = label_with_ink_at(1000, &[3]);
        assert!(
            edge_only
                .compensate_feed_margin()
                .unwrap_err()
                .contains("blank")
        );
    }

    #[test]
    fn a_720_dpi_page_keeps_the_feed_resolution_and_halves_the_cross_axis() {
        // 80 mm x 36 mm band at 720 dpi: 2268 x 908 dots
        let label = page_to_label(&page_dpi(
            2268,
            908,
            720,
            |x, _| if x < 400 { 0 } else { 255 },
        ))
        .unwrap();
        assert_eq!(
            (label.tape_mm, label.feed_dpi, label.lines.len()),
            (36, 720, 2268)
        );
        let pins: Vec<usize> = (0..560)
            .filter(|p| label.lines[0][p / 8] & (0x80 >> (p % 8)) != 0)
            .collect();
        assert_eq!((pins[0], *pins.last().unwrap(), pins.len()), (61, 514, 454));
        // portrait pages work at 720 dpi too
        let portrait = page_dpi(908, 2268, 720, |_, y| if y < 400 { 0 } else { 255 });
        assert_eq!(page_to_label(&portrait).unwrap().lines.len(), 2268);
    }

    #[test]
    fn halving_keeps_one_dot_lines_visible() {
        let thin = page_dpi(100, 908, 720, |_, y| if y == 301 { 0 } else { 255 });
        let halved = thin.halved_across();
        assert_eq!(halved.height, 454);
        assert!(halved.pixels[150 * 100] < 128, "a 1 dot line must stay ink");
    }

    #[test]
    fn hi_res_scales_the_minimum_length_and_margins() {
        let mut label = page_to_label(&page_dpi(
            2268,
            908,
            720,
            |x, _| if x == 700 { 0 } else { 255 },
        ))
        .unwrap();
        label.compensate_feed_margin().unwrap();
        assert_eq!(label.lines.len(), 2268 - 2 * 2 * FEED_MARGIN_DOTS);
        let mut tiny = page_to_label(&page_dpi(
            300,
            908,
            720,
            |x, _| if x == 100 { 0 } else { 255 },
        ))
        .unwrap();
        tiny.trim_to_content(56, 56).unwrap();
        assert!(tiny.lines.len() >= 2 * MIN_LENGTH_DOTS as usize);
    }

    #[test]
    fn quality_options_and_the_job_headers_they_produce() {
        assert_eq!(parse_options("LabelQuality=HiRes").quality, Quality::HiRes);
        assert_eq!(parse_options("LabelQuality=High").quality, Quality::High);
        assert_eq!(parse_options("").quality, Quality::Normal);

        let normal = page_to_label(&page(400, 454, |x, _| if x < 50 { 0 } else { 255 })).unwrap();
        let high = Options {
            quality: Quality::High,
            ..Options::default()
        };
        let job = build_job_offline(&normal, &high, None).unwrap();
        assert!(
            job.windows(5).any(|w| w == [0x7A, 0xC4, 0x00, 36, 0]),
            "priority flag 0xC4"
        );

        let fine = page_to_label(&page_dpi(
            800,
            908,
            720,
            |x, _| if x < 100 { 0 } else { 255 },
        ))
        .unwrap();
        let hires = Options {
            quality: Quality::HiRes,
            ..Options::default()
        };
        let job = build_job_offline(&fine, &hires, None).unwrap();
        assert!(
            job.windows(6).any(|w| w == [0x7A, 0x86, 0x09, 36, 0, 0x20]),
            "type 09, 800 lines"
        );
        assert!(job.windows(5).any(|w| w == [0x1B, 0x69, 0x64, 28, 0]));
        // option and page resolution must agree
        assert!(build_job_offline(&fine, &Options::default(), None).is_err());
        assert!(build_job_offline(&normal, &hires, None).is_err());
    }

    #[test]
    fn cut_check_boxes_are_parsed() {
        assert_eq!(parse_options("").cut, CutMode::Half); // default: half cut ticked
        assert_eq!(
            parse_options("HalfCut=True FullCut=False").cut,
            CutMode::Half
        );
        assert_eq!(
            parse_options("HalfCut=False FullCut=True").cut,
            CutMode::Full
        );
        assert_eq!(
            parse_options("HalfCut=False FullCut=False").cut,
            CutMode::None
        );
        // both ticked: half cut wins (K bits are the same as half cut)
        assert_eq!(
            parse_options("HalfCut=True FullCut=True").cut,
            CutMode::Half
        );
        assert_eq!(parse_options("CutMode=None").cut, CutMode::None);
        assert!(parse_options("Chain=True").chain);
        assert!(!parse_options("Chain=False").chain);
        assert!(!parse_options("").chain);
        assert!(parse_options("PageSize=Auto9").auto_length);
    }

    #[test]
    fn the_standard_mirror_option_is_left_to_macos() {
        // macOS flips the raster itself; reading it here would flip twice.
        assert_eq!(parse_options("mirror=true"), Options::default());
        assert_eq!(parse_options("Mirror=On"), Options::default());
    }

    #[test]
    fn cut_modes_produce_the_matching_job_bytes() {
        let label = page_to_label(&page(400, 454, |x, _| if x < 50 { 0 } else { 255 })).unwrap();
        let build = |cut| {
            let options = Options {
                cut,
                ..Options::default()
            };
            build_job_offline(&label, &options, None).unwrap()
        };
        let has = |job: &[u8], bytes: &[u8]| job.windows(bytes.len()).any(|w| w == bytes);
        let half = build(CutMode::Half);
        assert!(has(&half, &[0x1B, 0x69, 0x4D, 0x40]) && has(&half, &[0x1B, 0x69, 0x4B, 0x0C]));
        let full = build(CutMode::Full);
        assert!(has(&full, &[0x1B, 0x69, 0x4D, 0x40]) && has(&full, &[0x1B, 0x69, 0x4B, 0x08]));
        let none = build(CutMode::None);
        assert!(has(&none, &[0x1B, 0x69, 0x4D, 0x00]));
        assert!(has(&none, &[0x1B, 0x69, 0x4B, 0x04]));
        assert!(
            !has(&none, &[0x1B, 0x69, 0x41]),
            "no ESC i A without cutting"
        );
    }

    #[test]
    fn nine_mm_tape_is_recognised_in_both_orientations() {
        // E850-verified band: pins 235..=340 (capture cap_9mm_new)
        assert_eq!(tape_width_mm_for_height(106), Some(9));
        assert_eq!(tape_width_mm_for_height(128), Some(9));
        let label = page_to_label(&page(300, 106, |_, _| 0)).unwrap();
        assert_eq!(label.tape_mm, 9);
        let pins: Vec<usize> = (0..560)
            .filter(|p| label.lines[0][p / 8] & (0x80 >> (p % 8)) != 0)
            .collect();
        assert_eq!(
            (pins[0], *pins.last().unwrap(), pins.len()),
            (235, 340, 106)
        );
        let portrait = page(106, 300, |_, y| if y < 100 { 0 } else { 255 });
        assert_eq!(page_to_label(&portrait).unwrap().tape_mm, 9);
    }

    #[test]
    fn auto_page_size_is_detected_from_the_options() {
        assert!(parse_options("PageSize=Auto CutMode=Half").auto_length);
        assert!(parse_options("PageSize=AutoP").auto_length);
        assert!(!parse_options("PageSize=L100").auto_length);
        assert!(!parse_options("PageSize=Custom.425x102").auto_length);
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
                ..Default::default()
            },
            None,
        )
        .unwrap();
        assert!(job.windows(4).any(|w| w == [0x1B, 0x69, 0x4B, 0x0C]));
        assert!(full.windows(4).any(|w| w == [0x1B, 0x69, 0x4B, 0x08]));
    }
}
