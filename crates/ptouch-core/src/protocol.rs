// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Huang Rui <vowstar@gmail.com>
// SPDX-FileCopyrightText: Dominic Radermacher and the ptouch-print contributors
//
// Portions derived from ptouch-print, licensed GPL-3.0-or-later:
// https://git.familie-radermacher.ch/linux/ptouch-print.git

//! Protocol command construction for Brother P-Touch printers.
//!
//! All functions in this module are pure -- they construct command byte
//! sequences as `Vec<u8>` without performing any I/O. The transport layer
//! is responsible for sending these commands to the printer.

use serde::{Deserialize, Serialize};

use crate::device::DeviceFlags;

/// Print quality mode.
///
/// On 360 dpi printers the head resolution is fixed at 360 dpi across the
/// tape, but the feed resolution along the tape can be doubled (high
/// resolution, 360x720) or halved (draft, 360x180). Physical label length
/// is preserved by duplicating or dropping raster lines.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrintQuality {
    /// Normal 1:1 printing (e.g. 360x360 dpi).
    #[default]
    Standard,
    /// High resolution: double feed resolution (e.g. 360x720 dpi).
    /// Requires laminated TZe or HG tape.
    HighRes,
    /// Draft / high speed: half feed resolution (e.g. 360x180 dpi).
    Draft,
}

/// Construct the initialization sequence.
///
/// Sends 100 zero bytes followed by the ESC @ (initialize) command.
/// This resets the printer to a known state.
pub fn cmd_init() -> Vec<u8> {
    let mut buf = vec![0u8; 100];
    buf.push(0x1B); // ESC
    buf.push(0x40); // @
    buf
}

/// Construct the status request command.
///
/// Sends ESC i S to request a 32-byte status packet from the printer.
pub fn cmd_status_request() -> Vec<u8> {
    vec![0x1B, 0x69, 0x53]
}

/// Construct the automatic status notification command.
///
/// Brother encodes notification as zero and suppression as one. Keeping
/// notifications enabled lets the transport observe the printer's phase
/// changes without guessing from USB transfer completion or sleeping for a
/// model-specific amount of time.
pub fn cmd_auto_status_notification(enabled: bool) -> Vec<u8> {
    vec![0x1B, 0x69, 0x21, if enabled { 0x00 } else { 0x01 }]
}

/// Construct the raster start command.
///
/// For devices with FLAG_P700_INIT, uses "ESC i a 0x01" (switch to raster mode).
/// For other devices, uses "ESC i R 0x01" (standard raster start).
pub fn cmd_raster_start(flags: DeviceFlags) -> Vec<u8> {
    if flags.contains(DeviceFlags::P700_INIT) {
        vec![0x1B, 0x69, 0x61, 0x01]
    } else {
        vec![0x1B, 0x69, 0x52, 0x01]
    }
}

/// Construct the PackBits compression enable command.
///
/// Sends M 0x02 to enable PackBits compression for raster data.
pub fn cmd_enable_packbits() -> Vec<u8> {
    vec![0x4D, 0x02]
}

/// Construct a line feed command.
///
/// Sends 0x5A to advance one raster line without printing.
pub fn cmd_line_feed() -> Vec<u8> {
    vec![0x5A]
}

/// Construct a form feed command.
///
/// Sends 0x0C to end the current page.
pub fn cmd_form_feed() -> Vec<u8> {
    vec![0x0C]
}

/// Construct the finalize (end of print) command.
///
/// In eject mode, sends 0x1A to eject and cut the tape.
/// In chain mode, sends 0x0C (form feed), except for D460BT-type devices
/// which always use 0x1A.
pub fn cmd_finalize(chain_print: bool, flags: DeviceFlags) -> Vec<u8> {
    if chain_print && !flags.contains(DeviceFlags::D460BT_MAGIC) {
        vec![0x0C]
    } else {
        vec![0x1A]
    }
}

/// Construct the info command (ESC i z).
///
/// Sets media width and raster line count. Used by devices with
/// FLAG_USE_INFO_CMD.
///
/// Format: ESC i z 0x00 0x00 <width_mm> 0x00
///         <raster_lines as 4 LE bytes> <n9> 0x00
///
/// For D460BT devices, n9 is set to 0x02 to feed the last label properly.
pub fn cmd_info(media_width: u8, raster_lines: u32, flags: DeviceFlags) -> Vec<u8> {
    let mut buf = vec![0x1B, 0x69, 0x7A]; // ESC i z
    buf.push(0x00); // n1: quality flags
    buf.push(0x00); // n2: media type
    buf.push(media_width); // n3: media width
    buf.push(0x00); // n4
    buf.push((raster_lines & 0xFF) as u8); // n5: raster lines (LE)
    buf.push(((raster_lines >> 8) & 0xFF) as u8);
    buf.push(((raster_lines >> 16) & 0xFF) as u8);
    buf.push(((raster_lines >> 24) & 0xFF) as u8);
    // n9: D460BT feed control (0x02 to feed last label properly)
    buf.push(if flags.contains(DeviceFlags::D460BT_MAGIC) {
        0x02
    } else {
        0x00
    });
    buf.push(0x00); // n10
    buf
}

/// Construct the pre-cut on/off command.
///
/// Sends ESC i M followed by 0x40 (auto-cut on) or 0x00 (auto-cut off).
pub fn cmd_precut(enabled: bool) -> Vec<u8> {
    vec![0x1B, 0x69, 0x4D, if enabled { 0x40 } else { 0x00 }]
}

/// Construct the legacy print information command (ESC i c).
///
/// Used by the PT-9500PC generation of 360 dpi printers.
/// Byte values are verified with those the Windows driver sends.
/// The byte format is almost the same as ESC i z command documented for the PT-P900 series:
///
/// ESC i c {n1} {n2} {n3} {n4} {n5}
/// {n1}:   Valid flag: Specifies which values are valid
///         #define PI_KIND 0x02 // Media type
///         #define PI_WIDTH 0x04 // Media width
///         #define PI_LENGTH 0x08 // Media length
///         #define PI_QUALITY 0x40 // Priority given to print quality (Not used)
///         #define PI_RECOVER 0x80 // Printer recovery always on
/// {n2}:   Media type
///         Laminated/Non-laminated tape: 00h
///         High Grade tape: 09h (required for high-resolution and draft printing)
/// {n3}:   Media width (mm)
/// {n4}:   Media length (mm)
/// {n5}:   Unknown / Undocumented, Windows driver sets it to 0x01 only for high-res printing
pub fn cmd_legacy_info(media_width: u8, quality: PrintQuality) -> Vec<u8> {
    match quality {
        PrintQuality::Draft => vec![0x1B, 0x69, 0x63, 0x86, 0x09, media_width, 0x00, 0x00],
        PrintQuality::Standard => vec![0x1B, 0x69, 0x63, 0x84, 0x00, media_width, 0x00, 0x00],
        PrintQuality::HighRes => vec![0x1B, 0x69, 0x63, 0x86, 0x09, media_width, 0x00, 0x01],
    }
}

/// Construct the advanced mode command (ESC i K)
///
/// Bit 0 selects draft (high speed) printing per the Brother PT-P900 raster reference.
/// Bit 2 enables half-cut for printers with that feature (default true when this command isn't sent)
/// Bit 3 enables cut at the end of the print (default true when this command isn't sent)
/// Bit 4 enables special tape mode (all cuts are disable)
/// Bit 6 selects high-resolution mode
pub fn cmd_advanced_mode(
    quality: PrintQuality,
    special_tape: bool,
    cut_at_end: bool,
    half_cut: bool,
) -> Vec<u8> {
    let flags = ((quality == PrintQuality::HighRes) as u8) << 6
        | (special_tape as u8) << 4
        | (cut_at_end as u8) << 3
        | (half_cut as u8) << 2
        | ((quality == PrintQuality::Draft) as u8);
    vec![0x1B, 0x69, 0x4B, flags]
}

/// Construct the D460BT magic initialization command.
///
/// Sends ESC i d followed by 4 parameter bytes:
///   n1/n2: margin value as LE u16 (0x0001 = minimal margin)
///   n3: must be 0x4D or the print gets corrupted
///   n4: reserved (0x00)
pub fn cmd_d460bt_magic() -> Vec<u8> {
    vec![0x1B, 0x69, 0x64, 0x01, 0x00, 0x4D, 0x00]
}

/// Construct the D460BT chain print command.
///
/// Sends ESC i K 0x00 0x00.
/// The trailing 0x00 is a required NUL terminator.
pub fn cmd_d460bt_chain() -> Vec<u8> {
    vec![0x1B, 0x69, 0x4B, 0x00, 0x00]
}

/// Construct a raster data command without PackBits compression.
///
/// Format: G <length as 2 LE bytes> <data>
pub fn cmd_send_raster(data: &[u8]) -> Vec<u8> {
    let len = data.len() as u16;
    let mut buf = Vec::with_capacity(3 + data.len());
    buf.push(0x47); // G
    buf.push((len & 0xFF) as u8);
    buf.push((len >> 8) as u8);
    buf.extend_from_slice(data);
    buf
}

/// Construct a raster data command with PackBits "fake" compression.
///
/// The PackBits variant wraps the data with a header byte that indicates
/// the run length. For our purposes, we send uncompressed data with the
/// PackBits framing: G <total_len as 2 LE bytes> <(data.len()-1) as u8> <data>.
///
/// This is a "fake" PackBits encoding where the entire line is sent as a
/// single literal run.
pub fn cmd_send_raster_packbits(data: &[u8]) -> Vec<u8> {
    if data.is_empty() {
        return cmd_send_raster(data);
    }
    // PackBits literal run: control byte = (count - 1) for literal run
    let control_byte = (data.len() - 1) as u8;
    let total_len = (1 + data.len()) as u16;
    let mut buf = Vec::with_capacity(3 + 1 + data.len());
    buf.push(0x47); // G
    buf.push((total_len & 0xFF) as u8);
    buf.push((total_len >> 8) as u8);
    buf.push(control_byte);
    buf.extend_from_slice(data);
    buf
}

/// Construct the page flags / margin command.
///
/// Sends ESC i d followed by 2 bytes for the margin value (little-endian).
pub fn cmd_page_flags(margin: u16) -> Vec<u8> {
    vec![
        0x1B,
        0x69,
        0x64,
        (margin & 0xFF) as u8,
        ((margin >> 8) & 0xFF) as u8,
    ]
}

/// Minimum feed margin in dots, PT-P900 reference section 2.3.3.
pub const MIN_MARGIN_DOTS: u16 = 14;

/// Construct the 200 byte invalidate sequence followed by ESC @.
///
/// UNVERIFIED(E850): the PT-P900 reference asks for 200 bytes; the generic
/// [`cmd_init`] sends 100.
pub fn cmd_init_p900() -> Vec<u8> {
    let mut buf = vec![0u8; 200];
    buf.extend_from_slice(&[0x1B, 0x40]);
    buf
}

/// Map a status media type (byte 11) to the media type for ESC i z.
///
/// Laminated and non-laminated tape report 0x01/0x03 in status but are 0x00 in
/// ESC i z; heat-shrink tube uses the same code in both. `None` for media we
/// must not print on (no media, incompatible, unknown).
///
/// UNVERIFIED(E850): PT-P900 reference; HGe is assumed to behave as tape.
pub fn info_media_type(status_media_type: u8) -> Option<u8> {
    match status_media_type {
        0x01 | 0x03 => Some(0x00),
        0x11 => Some(0x11),
        _ => None,
    }
}

/// Construct the PT-P900-style info command (ESC i z).
///
/// E850-verified for TZe 36 mm (P-touch Editor captures, 2026-10-06): valid
/// flags 0x84 (media width + printer recovery, so the printer itself rejects a
/// width mismatch), media type 0x00, media length 0, and page byte 2 for a
/// single/last page. UNVERIFIED(E850): for HSe (type != 0) we also set the
/// "media type valid" flag 0x02 as the PT-P900 reference describes, and a page
/// byte other than 2 for pages that are followed by more.
pub fn cmd_info_p900(
    info_media_type: u8,
    media_width: u8,
    raster_lines: u32,
    last_page: bool,
) -> Vec<u8> {
    let flags = if info_media_type == 0x00 { 0x84 } else { 0x86 };
    let mut buf = vec![0x1B, 0x69, 0x7A, flags, info_media_type, media_width, 0x00];
    buf.extend_from_slice(&raster_lines.to_le_bytes());
    buf.push(if last_page { 0x02 } else { 0x00 }); // n9: page
    buf.push(0x00); // n10
    buf
}

/// Construct the job tag command (ESC i U) that P-touch Editor sends before
/// ESC i z: 15 payload bytes, constant for a given PC except byte 11, which
/// equals the LPR job number. Meaning of the constant part is unknown and
/// whether the printer needs it is unverified, so it is sent only on request.
pub fn cmd_job_tag(job_number: u8) -> Vec<u8> {
    let mut buf = vec![0x1B, 0x69, 0x55];
    buf.extend_from_slice(&[
        0x4A, 0x00, 0x0C, 0x04, 0x42, 0x1A, 0xB3, 0xD4, 0x9C, 0x00, 0x00,
    ]);
    buf.extend_from_slice(&[job_number, 0x00, 0x00, 0x00]);
    buf
}

/// Construct `ESC i k 63 01 00`, which P-touch Editor sends in every job just
/// before ESC i d. Purpose unknown (E850-seen, not in the PT-P900 reference).
pub fn cmd_esc_i_k_lower() -> Vec<u8> {
    vec![0x1B, 0x69, 0x6B, 0x63, 0x01, 0x00]
}

/// Construct the "cut every n labels" command (ESC i A).
pub fn cmd_cut_every(n: u8) -> Vec<u8> {
    vec![0x1B, 0x69, 0x41, n]
}

/// Create a blank raster line of the given width in pixels.
///
/// Returns a zeroed byte buffer of length `ceil(max_px / 8)`.
pub fn rasterline_blank(max_px: u16) -> Vec<u8> {
    vec![0u8; (max_px as usize).div_ceil(8)]
}

/// Options for building a print job command stream.
#[derive(Debug, Clone, Copy, Default)]
pub struct JobOptions {
    /// Media width in mm from the printer status (used by the info command).
    pub media_width: u8,
    /// Media type from the printer status byte 11 (used by `P900_RASTER`
    /// models so the printer itself rejects a media mismatch).
    pub media_type: u8,
    /// Emit the ESC i U job tag with this job number (`P900_RASTER` only).
    pub job_number: Option<u8>,
    /// Half-cut between labels (`P900_RASTER` models only).
    pub half_cut: bool,
    /// Feed margin in dots (`P900_RASTER` models only); 0 selects the
    /// documented minimum of 14 dots.
    pub margin_dots: u16,
    /// Chain print: skip the final feed and cut.
    pub chain_print: bool,
    /// Request the precut command (only sent if the device supports it).
    pub precut: bool,
    /// Print quality (only acted on if the device supports it).
    pub quality: PrintQuality,
}

/// Append raster lines (blank ones as `Z`), each repeated `repeat` times.
fn push_raster_lines(job: &mut Vec<Vec<u8>>, lines: &[&Vec<u8>], repeat: usize, packbits: bool) {
    for line in lines {
        for _ in 0..repeat {
            if rasterline_is_blank(line) {
                job.push(cmd_line_feed());
            } else if packbits {
                job.push(cmd_send_raster_packbits(line));
            } else {
                job.push(cmd_send_raster(line));
            }
        }
    }
}

/// Build the complete command stream for one print job.
///
/// Returns the commands in send order, one chunk per USB transfer. Keeping
/// the chunks separate preserves the transfer timing of the original
/// per-command sends (a single huge transfer could stall on the printer's
/// internal buffer).
///
/// For `P900_RASTER` models the sequence is rasterstart -> job tag -> info ->
/// precut/cut-every -> advanced mode -> ESC i k -> margin -> packbits ->
/// raster lines -> finalize (E850-verified against P-touch Editor jobs), and an empty `Vec` is returned when the media
/// type cannot be validated; callers must check [`info_media_type`] first.
///
/// Sequence: rasterstart -> info -> d460bt_magic -> precut ->
/// d460bt_chain -> packbits -> raster lines -> finalize
///
/// The compression select (M) is the last control code before the raster
/// data, as specified by the Brother raster command references (e.g.
/// PT-P900 reference, section 2.2.3). Printers that boot in ESC/P mode
/// (PT-9700PC) lock up when M arrives before the raster mode switch.
pub fn build_print_job(lines: &[Vec<u8>], flags: DeviceFlags, opts: &JobOptions) -> Vec<Vec<u8>> {
    let use_packbits = flags.contains(DeviceFlags::RASTER_PACKBITS);
    let is_d460bt = flags.contains(DeviceFlags::D460BT_MAGIC);

    // Quality modes change the feed resolution along the tape. Keep the
    // physical length by duplicating lines (high resolution) or dropping
    // every other line (draft).
    let quality = if flags.contains(DeviceFlags::LEGACY_HIRES) {
        opts.quality
    } else {
        PrintQuality::Standard
    };
    let (repeat, step) = match quality {
        PrintQuality::Standard => (1, 1),
        PrintQuality::HighRes => (2, 1),
        PrintQuality::Draft => (1, 2),
    };
    let selected: Vec<&Vec<u8>> = lines.iter().step_by(step).collect();
    let line_count = (selected.len() * repeat) as u32;

    let mut job: Vec<Vec<u8>> = Vec::with_capacity(selected.len() * repeat + 8);

    job.push(cmd_raster_start(flags));

    if flags.contains(DeviceFlags::P900_RASTER) {
        // Same order as P-touch Editor: a, U, z, M, A, K, k, d, M 02, raster.
        // Refuse media the printer cannot validate rather than sending a
        // header that disables the media check.
        let Some(kind) = info_media_type(opts.media_type) else {
            return Vec::new();
        };
        if let Some(number) = opts.job_number {
            job.push(cmd_job_tag(number));
        }
        job.push(cmd_info_p900(
            kind,
            opts.media_width,
            line_count,
            !opts.chain_print,
        ));
        if opts.precut {
            job.push(cmd_precut(true));
            job.push(cmd_cut_every(1));
        }
        job.push(cmd_advanced_mode(
            PrintQuality::Standard,
            false,
            !opts.chain_print,
            opts.half_cut,
        ));
        job.push(cmd_esc_i_k_lower());
        let margin = opts.margin_dots.max(MIN_MARGIN_DOTS);
        job.push(cmd_page_flags(margin));
        job.push(cmd_enable_packbits());
        push_raster_lines(&mut job, &selected, repeat, true);
        job.push(cmd_finalize(opts.chain_print, flags));
        return job;
    }

    // The standard quality path stays byte identical to the verified
    // stream; quality commands are only sent when a non-default mode is
    // requested on a device that supports it.
    if flags.contains(DeviceFlags::LEGACY_HIRES) && quality != PrintQuality::Standard {
        job.push(cmd_legacy_info(opts.media_width, quality));
        job.push(cmd_advanced_mode(quality, false, true, true));
    }

    if flags.contains(DeviceFlags::USE_INFO_CMD) {
        job.push(cmd_info(opts.media_width, line_count, flags));
    }

    if is_d460bt {
        job.push(cmd_d460bt_magic());
    }

    if flags.contains(DeviceFlags::HAS_PRECUT) && opts.precut {
        job.push(cmd_precut(true));
    }

    if is_d460bt && opts.chain_print {
        job.push(cmd_d460bt_chain());
    }

    if use_packbits {
        job.push(cmd_enable_packbits());
    }

    push_raster_lines(&mut job, &selected, repeat, use_packbits);

    job.push(cmd_finalize(opts.chain_print, flags));

    job
}

/// Check if a raster line is entirely blank (all zeros).
pub fn rasterline_is_blank(line: &[u8]) -> bool {
    line.iter().all(|&b| b == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmd_init() {
        let cmd = cmd_init();
        assert_eq!(cmd.len(), 102);
        assert!(cmd[..100].iter().all(|&b| b == 0));
        assert_eq!(cmd[100], 0x1B);
        assert_eq!(cmd[101], 0x40);
    }

    #[test]
    fn test_cmd_status_request() {
        assert_eq!(cmd_status_request(), vec![0x1B, 0x69, 0x53]);
    }

    #[test]
    fn test_cmd_auto_status_notification() {
        assert_eq!(
            cmd_auto_status_notification(true),
            vec![0x1B, 0x69, 0x21, 0x00]
        );
        assert_eq!(
            cmd_auto_status_notification(false),
            vec![0x1B, 0x69, 0x21, 0x01]
        );
    }

    #[test]
    fn test_cmd_raster_start_normal() {
        let cmd = cmd_raster_start(DeviceFlags::NONE);
        assert_eq!(cmd, vec![0x1B, 0x69, 0x52, 0x01]);
    }

    #[test]
    fn test_cmd_raster_start_p700() {
        let cmd = cmd_raster_start(DeviceFlags::P700_INIT);
        assert_eq!(cmd, vec![0x1B, 0x69, 0x61, 0x01]);
    }

    #[test]
    fn test_cmd_finalize_eject() {
        assert_eq!(cmd_finalize(false, DeviceFlags::NONE), vec![0x1A]);
    }

    #[test]
    fn test_cmd_finalize_chain() {
        assert_eq!(cmd_finalize(true, DeviceFlags::NONE), vec![0x0C]);
    }

    #[test]
    fn test_cmd_finalize_chain_d460bt() {
        // D460BT always uses eject
        assert_eq!(cmd_finalize(true, DeviceFlags::D460BT_MAGIC), vec![0x1A]);
    }

    #[test]
    fn test_cmd_precut() {
        assert_eq!(cmd_precut(true), vec![0x1B, 0x69, 0x4D, 0x40]);
        assert_eq!(cmd_precut(false), vec![0x1B, 0x69, 0x4D, 0x00]);
    }

    #[test]
    fn test_cmd_send_raster() {
        let data = vec![0xFF, 0x00, 0xAA];
        let cmd = cmd_send_raster(&data);
        assert_eq!(cmd[0], 0x47);
        assert_eq!(cmd[1], 0x03); // length low
        assert_eq!(cmd[2], 0x00); // length high
        assert_eq!(&cmd[3..], &data[..]);
    }

    #[test]
    fn test_cmd_send_raster_packbits() {
        let data = vec![0xFF, 0x00, 0xAA];
        let cmd = cmd_send_raster_packbits(&data);
        assert_eq!(cmd[0], 0x47);
        assert_eq!(cmd[1], 0x04); // total_len = 1 + 3 = 4
        assert_eq!(cmd[2], 0x00);
        assert_eq!(cmd[3], 0x02); // control byte = 3 - 1 = 2
        assert_eq!(&cmd[4..], &data[..]);
    }

    #[test]
    fn test_cmd_info() {
        let cmd = cmd_info(24, 100, DeviceFlags::NONE);
        assert_eq!(cmd.len(), 13);
        assert_eq!(cmd[0], 0x1B);
        assert_eq!(cmd[1], 0x69);
        assert_eq!(cmd[2], 0x7A);
        assert_eq!(cmd[3], 0x00); // n1: quality flags = 0
        assert_eq!(cmd[4], 0x00); // n2: media type = 0
        assert_eq!(cmd[5], 24); // n3: width
        assert_eq!(cmd[6], 0x00); // n4
        assert_eq!(cmd[7], 100); // n5: raster lines low byte
        assert_eq!(cmd[8], 0x00);
        assert_eq!(cmd[9], 0x00);
        assert_eq!(cmd[10], 0x00);
        assert_eq!(cmd[11], 0x00); // n9: not D460BT
        assert_eq!(cmd[12], 0x00); // n10
    }

    #[test]
    fn test_cmd_info_d460bt() {
        let cmd = cmd_info(24, 100, DeviceFlags::D460BT_MAGIC);
        assert_eq!(cmd[11], 0x02); // n9: D460BT feed control
    }

    #[test]
    fn test_cmd_d460bt_magic() {
        assert_eq!(
            cmd_d460bt_magic(),
            vec![0x1B, 0x69, 0x64, 0x01, 0x00, 0x4D, 0x00]
        );
    }

    #[test]
    fn test_cmd_d460bt_chain() {
        assert_eq!(cmd_d460bt_chain(), vec![0x1B, 0x69, 0x4B, 0x00, 0x00]);
    }

    #[test]
    fn test_cmd_page_flags() {
        let cmd = cmd_page_flags(0x0300);
        assert_eq!(cmd, vec![0x1B, 0x69, 0x64, 0x00, 0x03]);
    }

    // -- Whole-job byte stream tests --
    //
    // These pin the exact command sequence sent to the printer so protocol
    // changes are visible in review as byte-level diffs.

    fn flat(job: &[Vec<u8>]) -> Vec<u8> {
        job.concat()
    }

    #[test]
    fn test_job_plain_raster() {
        // Device without packbits (e.g. PT-2430PC): raw G lines only.
        let lines = vec![vec![0xFF, 0x00], vec![0x00, 0x00]];
        let opts = JobOptions {
            media_width: 12,
            chain_print: false,
            precut: false,
            quality: PrintQuality::Standard,
            ..Default::default()
        };
        let job = build_print_job(&lines, DeviceFlags::NONE, &opts);
        let expected: Vec<u8> = [
            vec![0x1B, 0x69, 0x52, 0x01],       // ESC i R 01 raster start
            vec![0x47, 0x02, 0x00, 0xFF, 0x00], // G len data
            vec![0x5A],                         // Z blank line
            vec![0x1A],                         // print with feeding
        ]
        .concat();
        assert_eq!(flat(&job), expected);
    }

    #[test]
    fn test_job_packbits_p700() {
        // PT-P700 class: packbits framing plus ESC i a mode switch.
        let lines = vec![vec![0xAA, 0x55]];
        let flags = DeviceFlags::RASTER_PACKBITS
            .union(DeviceFlags::P700_INIT)
            .union(DeviceFlags::HAS_PRECUT);
        let opts = JobOptions {
            media_width: 24,
            chain_print: false,
            precut: true,
            quality: PrintQuality::Standard,
            ..Default::default()
        };
        let job = build_print_job(&lines, flags, &opts);
        let expected: Vec<u8> = [
            vec![0x1B, 0x69, 0x61, 0x01],             // ESC i a 01 raster mode
            vec![0x1B, 0x69, 0x4D, 0x40],             // ESC i M precut on
            vec![0x4D, 0x02],                         // M 02, last before data
            vec![0x47, 0x03, 0x00, 0x01, 0xAA, 0x55], // G with packbits run
            vec![0x1A],
        ]
        .concat();
        assert_eq!(flat(&job), expected);
    }

    #[test]
    fn test_job_compression_select_is_last_control_code() {
        // Brother raster references place the compression select (M)
        // immediately before the raster data. Windows driver captures
        // (rasterprynt, PT-9800PCN) confirm this order. Printers that
        // boot in ESC/P mode lock up when M arrives earlier.
        let lines = vec![vec![0xFF]];
        let flags = DeviceFlags::RASTER_PACKBITS.union(DeviceFlags::HAS_PRECUT);
        let opts = JobOptions {
            media_width: 12,
            chain_print: false,
            precut: true,
            quality: PrintQuality::Standard,
            ..Default::default()
        };
        let job = build_print_job(&lines, flags, &opts);
        let m_pos = job.iter().position(|c| c == &vec![0x4D, 0x02]).unwrap();
        assert_eq!(job[m_pos + 1][0], 0x47, "raster data must follow M");
    }

    #[test]
    fn test_job_chain_print() {
        let lines = vec![vec![0x01]];
        let opts = JobOptions {
            media_width: 12,
            chain_print: true,
            precut: false,
            quality: PrintQuality::Standard,
            ..Default::default()
        };
        let job = build_print_job(&lines, DeviceFlags::NONE, &opts);
        assert_eq!(job.last().unwrap(), &vec![0x0C]); // form feed, no cut
    }

    #[test]
    fn test_job_precut_requires_device_support() {
        let lines = vec![vec![0x01]];
        let opts = JobOptions {
            media_width: 12,
            chain_print: false,
            precut: true,
            quality: PrintQuality::Standard,
            ..Default::default()
        };
        let job = build_print_job(&lines, DeviceFlags::NONE, &opts);
        assert!(!flat(&job).windows(3).any(|w| w == [0x1B, 0x69, 0x4D]));
    }

    #[test]
    fn test_job_d460bt() {
        let lines = vec![vec![0x01]];
        let flags = DeviceFlags::P700_INIT
            .union(DeviceFlags::USE_INFO_CMD)
            .union(DeviceFlags::D460BT_MAGIC);
        let opts = JobOptions {
            media_width: 12,
            chain_print: true,
            precut: false,
            quality: PrintQuality::Standard,
            ..Default::default()
        };
        let job = build_print_job(&lines, flags, &opts);
        let expected: Vec<u8> = [
            vec![0x1B, 0x69, 0x61, 0x01], // raster mode
            vec![
                0x1B, 0x69, 0x7A, 0x00, 0x00, 12, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00,
            ], // ESC i z info
            vec![0x1B, 0x69, 0x64, 0x01, 0x00, 0x4D, 0x00], // D460BT magic
            vec![0x1B, 0x69, 0x4B, 0x00, 0x00], // D460BT chain
            vec![0x47, 0x01, 0x00, 0x01],
            vec![0x1A], // D460BT always ejects
        ]
        .concat();
        assert_eq!(flat(&job), expected);
    }

    /// Flags of the PT-9700PC as listed in the device table.
    fn pt9700_flags() -> DeviceFlags {
        DeviceFlags::RASTER_PACKBITS
            .union(DeviceFlags::P700_INIT)
            .union(DeviceFlags::HAS_PRECUT)
            .union(DeviceFlags::LEGACY_HIRES)
    }

    #[test]
    fn test_job_standard_quality_has_no_quality_commands() {
        // The hardware-verified standard path must stay byte identical:
        // no ESC i c and no ESC i K may appear.
        let lines = vec![vec![0xFF]];
        let opts = JobOptions {
            media_width: 24,
            ..JobOptions::default()
        };
        let job = build_print_job(&lines, pt9700_flags(), &opts);
        let bytes = flat(&job);
        assert!(!bytes.windows(3).any(|w| w == [0x1B, 0x69, 0x63]));
        assert!(!bytes.windows(3).any(|w| w == [0x1B, 0x69, 0x4B]));
    }

    #[test]
    fn test_job_hires_quality() {
        let lines = vec![vec![0xAA], vec![0x55]];
        let opts = JobOptions {
            media_width: 24,
            quality: PrintQuality::HighRes,
            ..JobOptions::default()
        };
        let job = build_print_job(&lines, pt9700_flags(), &opts);
        let expected: Vec<u8> = [
            vec![0x1B, 0x69, 0x61, 0x01],                       // raster mode
            vec![0x1B, 0x69, 0x63, 0x86, 0x09, 24, 0x00, 0x01], // ESC i c hires
            vec![0x1B, 0x69, 0x4B, 0x4c], // ESC i K hires bit with default half-cut and cut-at-end bits
            vec![0x4D, 0x02],             // packbits
            vec![0x47, 0x02, 0x00, 0x00, 0xAA], // line 1
            vec![0x47, 0x02, 0x00, 0x00, 0xAA], // line 1 repeated
            vec![0x47, 0x02, 0x00, 0x00, 0x55], // line 2
            vec![0x47, 0x02, 0x00, 0x00, 0x55], // line 2 repeated
            vec![0x1A],
        ]
        .concat();
        assert_eq!(flat(&job), expected);
    }

    #[test]
    fn test_job_draft_quality() {
        // Draft halves the feed resolution: every other line is dropped.
        let lines = vec![vec![0x01], vec![0x02], vec![0x03]];
        let opts = JobOptions {
            media_width: 12,
            quality: PrintQuality::Draft,
            ..JobOptions::default()
        };
        let job = build_print_job(&lines, pt9700_flags(), &opts);
        let expected: Vec<u8> = [
            vec![0x1B, 0x69, 0x61, 0x01],                       // raster mode
            vec![0x1B, 0x69, 0x63, 0x86, 0x09, 12, 0x00, 0x00], // ESC i c to require HG tape as draft mode needs it
            vec![0x1B, 0x69, 0x4B, 0x0d], // ESC i K draft bit with default half-cut and cut-at-end bits
            vec![0x4D, 0x02],             // packbits
            vec![0x47, 0x02, 0x00, 0x00, 0x01], // line 1
            vec![0x47, 0x02, 0x00, 0x00, 0x03], // line 3 (line 2 dropped)
            vec![0x1A],
        ]
        .concat();
        assert_eq!(flat(&job), expected);
    }

    #[test]
    fn test_job_quality_ignored_without_device_support() {
        let lines = vec![vec![0xFF]];
        let opts = JobOptions {
            media_width: 12,
            quality: PrintQuality::HighRes,
            ..JobOptions::default()
        };
        let job = build_print_job(&lines, DeviceFlags::RASTER_PACKBITS, &opts);
        let bytes = flat(&job);
        assert!(!bytes.windows(3).any(|w| w == [0x1B, 0x69, 0x63]));
        // Lines are not duplicated either.
        assert_eq!(bytes.iter().filter(|&&b| b == 0x47).count(), 1);
    }

    fn e850_flags() -> DeviceFlags {
        DeviceFlags::RASTER_PACKBITS
            | DeviceFlags::P700_INIT
            | DeviceFlags::USE_INFO_CMD
            | DeviceFlags::HAS_PRECUT
            | DeviceFlags::P900_RASTER
    }

    fn line_70(first: u8) -> Vec<u8> {
        let mut line = vec![0u8; 70];
        line[0] = first;
        line
    }

    #[test]
    fn test_info_p900_matches_reference_encoder_bytes() {
        // P-touch Editor (E850 capture): 0x84, type 0, width, 0, lines LE u32, page 2, 0
        assert_eq!(
            cmd_info_p900(0x00, 9, 0x0000_0150, true),
            [
                0x1B, 0x69, 0x7A, 0x84, 0x00, 0x09, 0x00, 0x50, 0x01, 0x00, 0x00, 0x02, 0x00
            ]
        );
        assert_eq!(cmd_info_p900(0x11, 9, 1, false)[3..6], [0x86, 0x11, 0x09]);
    }

    #[test]
    fn test_info_media_type_mapping() {
        assert_eq!(info_media_type(0x01), Some(0x00));
        assert_eq!(info_media_type(0x03), Some(0x00));
        assert_eq!(info_media_type(0x11), Some(0x11));
        for refused in [0x00, 0xFF, 0x17, 0x04] {
            assert_eq!(info_media_type(refused), None);
        }
    }

    #[test]
    fn test_job_p900_header_order_single_label() {
        let lines = vec![line_70(0xFF), line_70(0x00)];
        let opts = JobOptions {
            media_width: 36,
            media_type: 0x01,
            precut: true,
            ..JobOptions::default()
        };
        let job = flat(&build_print_job(&lines, e850_flags(), &opts));
        let mut expected = vec![0x1B, 0x69, 0x61, 0x01]; // raster mode
        expected.extend([0x1B, 0x69, 0x7A, 0x84, 0x00, 36, 0x00, 2, 0, 0, 0, 2, 0]);
        expected.extend([0x1B, 0x69, 0x4D, 0x40]); // auto-cut
        expected.extend([0x1B, 0x69, 0x41, 0x01]); // cut every 1
        expected.extend([0x1B, 0x69, 0x4B, 0x08]); // cut at end
        expected.extend([0x1B, 0x69, 0x6B, 0x63, 0x01, 0x00]);
        expected.extend([0x1B, 0x69, 0x64, 14, 0]); // minimum margin
        expected.extend([0x4D, 0x02]);
        assert!(job.starts_with(&expected), "{job:02x?}");
        assert_eq!(*job.last().unwrap(), 0x1A);
        assert_eq!(job[expected.len()], 0x47); // first line is non-blank
    }

    /// P-touch Editor job (capture 3, letter F, TZe 36 mm, half-cut, 360x360):
    /// our header must match its bytes exactly up to the raster data.
    #[test]
    fn test_job_p900_header_matches_p_touch_editor_capture() {
        const FIXTURE: &[u8] = include_bytes!("../../../fixtures/jobs/cap3_011I_19892B.bin");
        const LINES: usize = 1069;
        let header_end = FIXTURE
            .windows(2)
            .position(|w| w == [0x4D, 0x02])
            .expect("compression select")
            + 2;
        assert_eq!(&FIXTURE[..202], cmd_init_p900().as_slice());

        let opts = JobOptions {
            media_width: 36,
            media_type: 0x01,
            precut: true,
            half_cut: true,
            job_number: Some(11),
            ..JobOptions::default()
        };
        let lines = vec![line_70(1); LINES];
        let job = flat(&build_print_job(&lines, e850_flags(), &opts));
        assert_eq!(
            &job[..header_end - 202],
            &FIXTURE[202..header_end],
            "header differs from the Brother capture"
        );
        assert_eq!(*job.last().unwrap(), *FIXTURE.last().unwrap()); // 0x1A
    }

    #[test]
    fn test_job_p900_chain_and_half_cut() {
        let lines = vec![line_70(0x80)];
        let opts = JobOptions {
            media_width: 9,
            media_type: 0x11,
            chain_print: true,
            half_cut: true,
            margin_dots: 40,
            ..JobOptions::default()
        };
        let job = flat(&build_print_job(&lines, e850_flags(), &opts));
        assert!(job.windows(4).any(|w| w == [0x1B, 0x69, 0x4B, 0x04])); // half-cut, no cut at end
        assert!(job.windows(5).any(|w| w == [0x1B, 0x69, 0x64, 40, 0]));
        assert_eq!(*job.last().unwrap(), 0x0C); // more pages follow
    }

    #[test]
    fn test_job_p900_margin_is_clamped_to_documented_minimum() {
        let opts = JobOptions {
            media_width: 9,
            media_type: 0x01,
            margin_dots: 3,
            ..JobOptions::default()
        };
        let job = flat(&build_print_job(&[line_70(1)], e850_flags(), &opts));
        assert!(job.windows(5).any(|w| w == [0x1B, 0x69, 0x64, 14, 0]));
    }

    #[test]
    fn test_job_p900_refuses_unvalidated_media() {
        for media_type in [0x00, 0xFF, 0x17] {
            let opts = JobOptions {
                media_width: 9,
                media_type,
                ..JobOptions::default()
            };
            assert!(build_print_job(&[line_70(1)], e850_flags(), &opts).is_empty());
        }
    }

    #[test]
    fn test_job_p900_blank_lines_use_z_and_init_is_200_bytes() {
        let opts = JobOptions {
            media_width: 9,
            media_type: 0x01,
            ..JobOptions::default()
        };
        let job = build_print_job(&[line_70(0), line_70(1)], e850_flags(), &opts);
        assert!(job.iter().any(|c| c == &vec![0x5A]));
        assert_eq!(cmd_init_p900().len(), 202);
        assert_eq!(cmd_init_p900()[200..], [0x1B, 0x40]);
    }

    #[test]
    fn test_legacy_job_unchanged_by_new_options() {
        // Models without P900_RASTER must not pick up the new header commands.
        let opts = JobOptions {
            media_width: 12,
            media_type: 0x01,
            half_cut: true,
            margin_dots: 40,
            ..JobOptions::default()
        };
        let job = flat(&build_print_job(
            &[vec![1u8; 16]],
            DeviceFlags::RASTER_PACKBITS,
            &opts,
        ));
        assert!(!job.windows(3).any(|w| w == [0x1B, 0x69, 0x4B]));
        assert!(!job.windows(3).any(|w| w == [0x1B, 0x69, 0x64]));
    }
}
