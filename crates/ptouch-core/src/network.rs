// SPDX-License-Identifier: GPL-3.0-or-later

//! Network printing for the PT-E850TKW over Wi-Fi/LAN.
//!
//! This copies what P-touch Editor does on Windows (captures 2 and 3,
//! raster-protocol.md section 10): the job goes out by **LPR (RFC 1179) to TCP
//! 515, queue `BINARY_P1`**, state is polled by **SNMP** (UDP 161), and the
//! loaded media width is read from the printer's own web page. Raw TCP 9100
//! never answers `ESC i S` on this printer, so it is not used.
//!
//! The LPR channel only returns one-byte acknowledgements, so the media safety
//! check is: the web page must report the width we are about to print, and the
//! job header carries the width flag (`ESC i z` 0x84), which makes the printer
//! reject a mismatch itself.

use crate::{
    device::{self, DeviceFlags, DeviceInfo},
    error::{PtouchError, Result},
    protocol::{self, PrintQuality},
    tape,
};
use log::{debug, info};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream, ToSocketAddrs, UdpSocket},
    time::{Duration, Instant},
};

/// LPR (line printer daemon) port.
pub const LPR_PORT: u16 = 515;
/// SNMP agent port.
pub const SNMP_PORT: u16 = 161;
/// Embedded web server port.
pub const HTTP_PORT: u16 = 80;
/// LPR queue used by P-touch Editor (E850-verified).
pub const LPR_QUEUE: &str = "BINARY_P1";

const MODEL_NAME: &str = "PT-E850TKW";
const SNMP_COMMUNITY: &str = "public";
const OID_PRINTER_STATUS: &str = "1.3.6.1.2.1.25.3.5.1.1.1";
const OID_ERROR_STATE: &str = "1.3.6.1.2.1.25.3.5.1.2.1";
const OID_DEVICE_ID: &str = "1.3.6.1.4.1.2699.1.2.1.2.1.1.3.1";

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const LPR_IO_TIMEOUT: Duration = Duration::from_secs(30);
const SNMP_TIMEOUT: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_millis(250);
/// How long a finished job may stay unseen as "printing" before we give up
/// claiming it printed.
const START_TIMEOUT: Duration = Duration::from_secs(20);
/// The printer reports `other(1)` while it shows an error on its panel.
const OTHER_STATE_GRACE: Duration = Duration::from_secs(5);

/// Sub-second clock reading, used only to vary request and job numbers.
fn clock_nanos() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |d| d.subsec_nanos())
}

// ---------------------------------------------------------------------------
// SNMP (v1 GET, just enough BER)
// ---------------------------------------------------------------------------

/// A decoded SNMP value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SnmpValue {
    Int(i64),
    Bytes(Vec<u8>),
    Null,
    NoSuch,
}

fn ber_tlv(tag: u8, body: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    match body.len() {
        n if n < 0x80 => out.push(n as u8),
        n if n <= 0xFF => out.extend([0x81, n as u8]),
        n => out.extend([0x82, (n >> 8) as u8, n as u8]),
    }
    out.extend_from_slice(body);
    out
}

fn ber_int(value: i64) -> Vec<u8> {
    let mut bytes = value.to_be_bytes().to_vec();
    while bytes.len() > 1
        && ((bytes[0] == 0x00 && bytes[1] & 0x80 == 0)
            || (bytes[0] == 0xFF && bytes[1] & 0x80 != 0))
    {
        bytes.remove(0);
    }
    ber_tlv(0x02, &bytes)
}

fn encode_oid(oid: &str) -> Result<Vec<u8>> {
    let arcs: Vec<u32> = oid
        .split('.')
        .map(str::parse)
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| PtouchError::StatusError(format!("Invalid OID {oid}")))?;
    if arcs.len() < 2 || arcs[0] > 2 || arcs[1] >= 40 {
        return Err(PtouchError::StatusError(format!("Invalid OID {oid}")));
    }
    let mut body = vec![(arcs[0] * 40 + arcs[1]) as u8];
    for &arc in &arcs[2..] {
        let mut groups = vec![(arc & 0x7F) as u8];
        let mut rest = arc >> 7;
        while rest > 0 {
            groups.push((rest & 0x7F) as u8 | 0x80);
            rest >>= 7;
        }
        groups.reverse();
        body.extend(groups);
    }
    Ok(ber_tlv(0x06, &body))
}

/// Build an SNMPv1 GetRequest.
pub(crate) fn snmp_get_request(community: &str, request_id: i32, oids: &[&str]) -> Result<Vec<u8>> {
    let mut varbinds = Vec::new();
    for oid in oids {
        let mut pair = encode_oid(oid)?;
        pair.extend([0x05, 0x00]); // NULL
        varbinds.extend(ber_tlv(0x30, &pair));
    }
    let mut pdu = ber_int(i64::from(request_id));
    pdu.extend(ber_int(0)); // error status
    pdu.extend(ber_int(0)); // error index
    pdu.extend(ber_tlv(0x30, &varbinds));
    let mut message = ber_int(0); // SNMP v1
    message.extend(ber_tlv(0x04, community.as_bytes()));
    message.extend(ber_tlv(0xA0, &pdu));
    Ok(ber_tlv(0x30, &message))
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn done(&self) -> bool {
        self.pos >= self.data.len()
    }

    fn tlv(&mut self) -> Result<(u8, &'a [u8])> {
        let bad = || PtouchError::StatusError("Malformed SNMP reply".into());
        let tag = *self.data.get(self.pos).ok_or_else(bad)?;
        let first = *self.data.get(self.pos + 1).ok_or_else(bad)?;
        let (len, header) = if first & 0x80 == 0 {
            (usize::from(first), 2)
        } else {
            let count = usize::from(first & 0x7F);
            if count == 0 || count > 2 {
                return Err(bad());
            }
            let bytes = self
                .data
                .get(self.pos + 2..self.pos + 2 + count)
                .ok_or_else(bad)?;
            (
                bytes
                    .iter()
                    .fold(0usize, |acc, b| (acc << 8) | usize::from(*b)),
                2 + count,
            )
        };
        let start = self.pos + header;
        let body = self.data.get(start..start + len).ok_or_else(bad)?;
        self.pos = start + len;
        Ok((tag, body))
    }
}

fn decode_int(body: &[u8]) -> i64 {
    let mut value = if body.first().is_some_and(|b| b & 0x80 != 0) {
        -1i64
    } else {
        0
    };
    for b in body {
        value = (value << 8) | i64::from(*b);
    }
    value
}

fn decode_oid(body: &[u8]) -> String {
    let Some((&first, rest)) = body.split_first() else {
        return String::new();
    };
    let mut arcs = vec![u64::from(first / 40), u64::from(first % 40)];
    let mut acc = 0u64;
    for b in rest {
        acc = (acc << 7) | u64::from(b & 0x7F);
        if b & 0x80 == 0 {
            arcs.push(acc);
            acc = 0;
        }
    }
    arcs.iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

/// Parse an SNMP response (or any PDU) into its request id and varbinds.
pub(crate) fn snmp_parse(packet: &[u8]) -> Result<(i32, Vec<(String, SnmpValue)>)> {
    let bad = || PtouchError::StatusError("Malformed SNMP reply".into());
    let (_, message) = Reader::new(packet).tlv()?;
    let mut msg = Reader::new(message);
    msg.tlv()?; // version
    msg.tlv()?; // community
    let (_, pdu) = msg.tlv()?;
    let mut pdu = Reader::new(pdu);
    let (_, id) = pdu.tlv()?;
    let (_, error_status) = pdu.tlv()?;
    pdu.tlv()?; // error index
    if decode_int(error_status) != 0 {
        return Err(PtouchError::StatusError(format!(
            "SNMP error status {}",
            decode_int(error_status)
        )));
    }
    let (_, varbinds) = pdu.tlv()?;
    let mut vbs = Reader::new(varbinds);
    let mut values = Vec::new();
    while !vbs.done() {
        let (_, pair) = vbs.tlv()?;
        let mut pair = Reader::new(pair);
        let (oid_tag, oid) = pair.tlv()?;
        if oid_tag != 0x06 {
            return Err(bad());
        }
        let (tag, body) = pair.tlv()?;
        let value = match tag {
            0x02 => SnmpValue::Int(decode_int(body)),
            0x04 => SnmpValue::Bytes(body.to_vec()),
            0x41..=0x43 => SnmpValue::Int(body.iter().fold(0i64, |a, b| (a << 8) | i64::from(*b))),
            0x80..=0x82 => SnmpValue::NoSuch,
            _ => SnmpValue::Null,
        };
        values.push((decode_oid(oid), value));
    }
    Ok((decode_int(id) as i32, values))
}

// ---------------------------------------------------------------------------
// Printer state
// ---------------------------------------------------------------------------

/// `hrPrinterStatus` from the Host Resources MIB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrinterState {
    /// `other(1)`: seen while the printer shows an error on its panel.
    Other,
    /// `unknown(2)`.
    Unknown,
    /// `idle(3)`.
    Idle,
    /// `printing(4)`.
    Printing,
    /// `warmup(5)`.
    Warmup,
    /// Any other value.
    Unrecognized(i64),
}

impl From<i64> for PrinterState {
    fn from(value: i64) -> Self {
        match value {
            1 => Self::Other,
            2 => Self::Unknown,
            3 => Self::Idle,
            4 => Self::Printing,
            5 => Self::Warmup,
            other => Self::Unrecognized(other),
        }
    }
}

/// Status read over SNMP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetworkStatus {
    /// Overall state.
    pub state: PrinterState,
    /// `hrPrinterDetectedErrorState` first byte (lowPaper 0x80, noPaper 0x40,
    /// doorOpen 0x08, jammed 0x04, offline 0x02, serviceRequested 0x01, ...).
    pub error_flags: u8,
}

impl NetworkStatus {
    /// Idle with no reported error.
    pub fn is_ready(&self) -> bool {
        self.state == PrinterState::Idle && self.error_flags == 0
    }

    /// Short description of why the printer is not ready.
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        for (bit, name) in [
            (0x80, "low media"),
            (0x40, "no media"),
            (0x20, "low toner"),
            (0x10, "no toner"),
            (0x08, "cover open"),
            (0x04, "jammed"),
            (0x02, "offline"),
            (0x01, "service requested"),
        ] {
            if self.error_flags & bit != 0 {
                parts.push(name);
            }
        }
        let flags = if parts.is_empty() {
            String::new()
        } else {
            format!(" ({})", parts.join(", "))
        };
        format!("state {:?}{flags}", self.state)
    }
}

// ---------------------------------------------------------------------------
// Web page and LPR helpers (pure parts are unit tested)
// ---------------------------------------------------------------------------

/// Extract the loaded media width in mm from the printer web page.
///
/// UNVERIFIED(E850): scraped from `/general/status.html` (FW 1.59), where the
/// markup is `<dt>Media&#32;Type</dt><dd>36mm(1.4")</dd>`. Only TZe 36 mm has
/// been seen; how HSe tube is shown there is unknown.
pub(crate) fn parse_media_width_mm(html: &str) -> Option<u8> {
    let html = html.replace("&#32;", " ");
    let at = html.find("Media Type")?;
    let tail: String = {
        let mut text = String::new();
        let mut in_tag = false;
        for c in html[at + "Media Type".len()..].chars().take(400) {
            match c {
                '<' => in_tag = true,
                '>' => {
                    in_tag = false;
                    text.push(' ');
                }
                _ if !in_tag => text.push(c),
                _ => {}
            }
        }
        text
    };
    let digits_end = tail.find("mm")?;
    let digits: String = tail[..digits_end]
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    digits.parse::<f32>().ok().map(|mm| mm.round() as u8)
}

/// LPR control file as P-touch Editor sends it (capture 2: H, P, J, l, U, N).
pub(crate) fn lpr_control_file(host: &str, user: &str, name: &str, data_file: &str) -> Vec<u8> {
    format!("H{host}\nP{user}\nJ{name}\nl{data_file}\nU{data_file}\nN{name}\n").into_bytes()
}

fn read_ack(stream: &mut TcpStream, what: &str) -> Result<()> {
    let mut ack = [0u8; 1];
    stream
        .read_exact(&mut ack)
        .map_err(|e| PtouchError::SendFailed(format!("no LPR acknowledgement for {what}: {e}")))?;
    if ack[0] != 0 {
        return Err(PtouchError::SendFailed(format!(
            "printer rejected LPR {what} (code {})",
            ack[0]
        )));
    }
    Ok(())
}

/// Submit one binary job to an LPD server.
fn lpr_submit(addr: SocketAddr, job_no: u8, data: &[u8]) -> Result<()> {
    let io = |what: &'static str| {
        move |e: std::io::Error| PtouchError::SendFailed(format!("{what}: {e}"))
    };
    let mut stream =
        TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT).map_err(io("LPR connect"))?;
    stream
        .set_read_timeout(Some(LPR_IO_TIMEOUT))
        .map_err(io("LPR setup"))?;
    stream
        .set_write_timeout(Some(LPR_IO_TIMEOUT))
        .map_err(io("LPR setup"))?;

    let host = "ptouch";
    let data_file = format!("dfA{job_no:03}{host}");
    let control_file = format!("cfA{job_no:03}{host}");
    let control = lpr_control_file(host, "ptouch", "Label", &data_file);

    stream
        .write_all(format!("\x02{LPR_QUEUE}\n").as_bytes())
        .map_err(io("LPR send"))?;
    read_ack(&mut stream, "queue")?;

    stream
        .write_all(format!("\x02{} {control_file}\n", control.len()).as_bytes())
        .map_err(io("LPR send"))?;
    read_ack(&mut stream, "control file header")?;
    stream.write_all(&control).map_err(io("LPR send"))?;
    stream.write_all(&[0]).map_err(io("LPR send"))?;
    read_ack(&mut stream, "control file")?;

    stream
        .write_all(format!("\x03{} {data_file}\n", data.len()).as_bytes())
        .map_err(io("LPR send"))?;
    read_ack(&mut stream, "data file header")?;
    stream.write_all(data).map_err(io("LPR send"))?;
    stream.write_all(&[0]).map_err(io("LPR send"))?;
    read_ack(&mut stream, "data file")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// NetworkPrinter
// ---------------------------------------------------------------------------

/// Addresses of the three services; defaults use the standard ports.
#[derive(Debug, Clone, Copy)]
pub struct Endpoints {
    /// LPR (TCP).
    pub lpr: SocketAddr,
    /// SNMP (UDP).
    pub snmp: SocketAddr,
    /// Web page (TCP).
    pub http: SocketAddr,
}

impl Endpoints {
    /// Standard ports on one address.
    pub fn standard(ip: std::net::IpAddr) -> Self {
        Self {
            lpr: SocketAddr::new(ip, LPR_PORT),
            snmp: SocketAddr::new(ip, SNMP_PORT),
            http: SocketAddr::new(ip, HTTP_PORT),
        }
    }
}

/// A PT-E850TKW reached over the network.
pub struct NetworkPrinter {
    endpoints: Endpoints,
    info: &'static DeviceInfo,
    media_width_mm: u8,
    media_type: u8,
    half_cut: bool,
    margin_dots: u16,
    job_tag: bool,
    job_timeout: Duration,
    next_job: u8,
}

impl NetworkPrinter {
    /// Connect to a printer by host name or IP address (standard ports).
    ///
    /// Reads the SNMP device id (must be a PT-E850TKW) and the loaded media
    /// width from the web page. Sends nothing to the print queue.
    pub fn open(host: &str) -> Result<Self> {
        let ip = (host, 0)
            .to_socket_addrs()
            .map_err(|e| PtouchError::SendFailed(format!("cannot resolve {host}: {e}")))?
            .next()
            .ok_or_else(|| PtouchError::SendFailed(format!("cannot resolve {host}")))?
            .ip();
        Self::open_with(Endpoints::standard(ip))
    }

    /// Connect using explicit service addresses (tests, non-standard ports).
    pub fn open_with(endpoints: Endpoints) -> Result<Self> {
        let info = device::find_device_by_name(MODEL_NAME).ok_or(PtouchError::DeviceNotFound)?;
        let mut printer = Self {
            endpoints,
            info,
            media_width_mm: 0,
            // UNVERIFIED(E850): assume laminated TZe; the web page does not
            // show the media type, and HSe tube has not been seen over LPR.
            media_type: 0x01,
            half_cut: true,
            margin_dots: protocol::MIN_MARGIN_DOTS,
            job_tag: true,
            job_timeout: Duration::from_secs(180),
            next_job: (clock_nanos() % 200 + 1) as u8,
        };
        let id = printer.snmp_get(&[OID_DEVICE_ID])?;
        let model_ok = matches!(
            id.first(),
            Some((_, SnmpValue::Bytes(b))) if String::from_utf8_lossy(b).contains("MDL:PT-E850TKW")
        );
        if !model_ok {
            return Err(PtouchError::StatusError(format!(
                "{} is not a {MODEL_NAME}",
                printer.endpoints.snmp.ip()
            )));
        }
        printer.refresh_media()?;
        info!(
            "Network printer {} ready: media {} mm",
            printer.endpoints.snmp.ip(),
            printer.media_width_mm
        );
        Ok(printer)
    }

    /// Model name.
    pub fn model_name(&self) -> &'static str {
        self.info.name
    }

    /// Print resolution in DPI.
    pub fn dpi(&self) -> u16 {
        self.info.dpi
    }

    /// Raster line width in pixels (pins).
    pub fn raster_width_px(&self) -> u16 {
        self.info.max_px
    }

    /// Media width last read from the printer, in mm.
    pub fn media_width_mm(&self) -> u8 {
        self.media_width_mm
    }

    /// Printable pixels across the loaded media, if its band is known.
    pub fn tape_width_px(&self) -> Option<u16> {
        tape::head_band_560(self.media_type, self.media_width_mm).map(|b| b.print_pins)
    }

    /// Leading unused pins before the printable band.
    pub fn band_left_px(&self) -> Option<u16> {
        tape::head_band_560(self.media_type, self.media_width_mm).map(|b| b.left_pins)
    }

    /// Half-cut between labels (default on, as in the captures).
    pub fn set_half_cut(&mut self, half_cut: bool) {
        self.half_cut = half_cut;
    }

    /// Feed margin in dots (clamped to the documented minimum of 14).
    pub fn set_margin_dots(&mut self, dots: u16) {
        self.margin_dots = dots.max(protocol::MIN_MARGIN_DOTS);
    }

    /// Send the `ESC i U` job tag like P-touch Editor (default on).
    pub fn set_job_tag(&mut self, enabled: bool) {
        self.job_tag = enabled;
    }

    /// Upper bound for one job including the wait for completion.
    pub fn set_job_timeout(&mut self, timeout: Duration) {
        self.job_timeout = timeout;
    }

    /// Read the loaded media width from the web page.
    pub fn refresh_media(&mut self) -> Result<u8> {
        let html = self.fetch_status_page()?;
        let width = parse_media_width_mm(&html).ok_or_else(|| {
            PtouchError::StatusError(
                "Could not read the media width from the printer web page".into(),
            )
        })?;
        self.media_width_mm = width;
        Ok(width)
    }

    /// Read the printer state over SNMP.
    pub fn status(&self) -> Result<NetworkStatus> {
        let values = self.snmp_get(&[OID_PRINTER_STATUS, OID_ERROR_STATE])?;
        let mut state = None;
        let mut error_flags = 0;
        for (oid, value) in values {
            match (oid.as_str(), value) {
                (OID_PRINTER_STATUS, SnmpValue::Int(v)) => state = Some(PrinterState::from(v)),
                (OID_ERROR_STATE, SnmpValue::Bytes(b)) => {
                    error_flags = b.first().copied().unwrap_or(0)
                }
                _ => {}
            }
        }
        Ok(NetworkStatus {
            state: state.ok_or_else(|| {
                PtouchError::StatusError("Printer did not report its state".into())
            })?,
            error_flags,
        })
    }

    /// Print one label.
    ///
    /// Refuses unless the printer is idle without errors and the web page
    /// reports a media width with a known band. Chain printing is rejected:
    /// how consecutive LPR jobs join into one strip is unverified.
    pub fn print_raster(
        &mut self,
        lines: &[Vec<u8>],
        chain_print: bool,
        precut: bool,
        quality: PrintQuality,
    ) -> Result<()> {
        if quality != PrintQuality::Standard {
            return Err(PtouchError::UnsupportedQuality(self.info.name.to_string()));
        }
        if chain_print {
            return Err(PtouchError::UnsupportedOperation(
                "chain printing over the network is unverified; print one label per job",
            ));
        }
        let expected = usize::from(self.info.max_px) / 8;
        if lines.is_empty() || lines.iter().any(|line| line.len() != expected) {
            return Err(PtouchError::StatusError(format!(
                "Raster must be non-empty lines of {expected} bytes"
            )));
        }

        let before = self.media_width_mm;
        let now = self.refresh_media()?;
        if before != 0 && now != before {
            return Err(PtouchError::StatusError(format!(
                "Media changed from {before} mm to {now} mm since the label was laid out"
            )));
        }
        if self.band_left_px().is_none() {
            return Err(PtouchError::StatusError(format!(
                "Unsupported media: {now} mm has no verified band"
            )));
        }
        let status = self.status()?;
        if !status.is_ready() {
            return Err(PtouchError::StatusError(format!(
                "Printer not ready: {}",
                status.describe()
            )));
        }

        let job_no = self.next_job;
        self.next_job = self.next_job.wrapping_add(1).max(1);
        let opts = protocol::JobOptions {
            media_width: self.media_width_mm,
            media_type: self.media_type,
            precut,
            half_cut: self.half_cut,
            margin_dots: self.margin_dots,
            job_number: self.job_tag.then_some(job_no),
            ..protocol::JobOptions::default()
        };
        let flags: DeviceFlags = self.info.flags;
        let chunks = protocol::build_print_job(lines, flags, &opts);
        if chunks.is_empty() {
            return Err(PtouchError::StatusError(
                "Unsupported media for this job".into(),
            ));
        }
        let mut data = protocol::cmd_init_p900();
        data.extend(chunks.into_iter().flatten());

        debug!("Submitting {} bytes as LPR job {job_no}", data.len());
        lpr_submit(self.endpoints.lpr, job_no, &data)?;
        self.wait_for_completion()
    }

    /// Nothing to release; present for symmetry with the USB device.
    pub fn close(self) -> Result<()> {
        Ok(())
    }

    fn wait_for_completion(&self) -> Result<()> {
        let started = Instant::now();
        let mut saw_printing = false;
        let mut other_since: Option<Instant> = None;
        loop {
            std::thread::sleep(POLL_INTERVAL);
            if started.elapsed() > self.job_timeout {
                return Err(PtouchError::CompletionUnknown);
            }
            let status = match self.status() {
                Ok(status) => status,
                Err(PtouchError::Timeout) => continue,
                Err(e) => return Err(e),
            };
            if status.error_flags != 0 {
                return Err(PtouchError::StatusError(format!(
                    "Printer error: {}",
                    status.describe()
                )));
            }
            match status.state {
                PrinterState::Printing => {
                    saw_printing = true;
                    other_since = None;
                }
                PrinterState::Idle if saw_printing => return Ok(()),
                PrinterState::Idle => {
                    other_since = None;
                    if started.elapsed() > START_TIMEOUT {
                        return Err(PtouchError::CompletionUnknown);
                    }
                }
                PrinterState::Other => {
                    let since = *other_since.get_or_insert_with(Instant::now);
                    if since.elapsed() > OTHER_STATE_GRACE {
                        return Err(PtouchError::StatusError(
                            "Printer reports an error state; check its display".into(),
                        ));
                    }
                }
                _ => other_since = None,
            }
        }
    }

    fn snmp_get(&self, oids: &[&str]) -> Result<Vec<(String, SnmpValue)>> {
        let bind = if self.endpoints.snmp.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        };
        let socket = UdpSocket::bind(bind)
            .map_err(|e| PtouchError::SendFailed(format!("SNMP socket: {e}")))?;
        socket
            .connect(self.endpoints.snmp)
            .and_then(|_| socket.set_read_timeout(Some(SNMP_TIMEOUT)))
            .map_err(|e| PtouchError::SendFailed(format!("SNMP socket: {e}")))?;
        let request_id = (clock_nanos() & 0x7FFF_FFFF) as i32 | 1;
        let request = snmp_get_request(SNMP_COMMUNITY, request_id, oids)?;
        for _attempt in 0..3 {
            socket
                .send(&request)
                .map_err(|e| PtouchError::SendFailed(format!("SNMP send: {e}")))?;
            let mut buf = [0u8; 2048];
            let deadline = Instant::now() + SNMP_TIMEOUT;
            while Instant::now() < deadline {
                match socket.recv(&mut buf) {
                    Ok(n) => {
                        let (id, values) = snmp_parse(&buf[..n])?;
                        if id == request_id {
                            return Ok(values);
                        }
                    }
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                        ) =>
                    {
                        break;
                    }
                    Err(e) => return Err(PtouchError::SendFailed(format!("SNMP receive: {e}"))),
                }
            }
        }
        Err(PtouchError::Timeout)
    }

    fn fetch_status_page(&self) -> Result<String> {
        let io = |what: &'static str| {
            move |e: std::io::Error| PtouchError::SendFailed(format!("{what}: {e}"))
        };
        let mut stream = TcpStream::connect_timeout(&self.endpoints.http, CONNECT_TIMEOUT)
            .map_err(io("web connect"))?;
        stream
            .set_read_timeout(Some(SNMP_TIMEOUT * 3))
            .map_err(io("web setup"))?;
        let request = format!(
            "GET /general/status.html HTTP/1.0\r\nHost: {}\r\nConnection: close\r\n\r\n",
            self.endpoints.http.ip()
        );
        stream
            .write_all(request.as_bytes())
            .map_err(io("web request"))?;
        // The embedded server (debut/1.30) sends the whole page but keeps the
        // connection open, so reading to EOF would stall until the timeout and
        // leave its single-threaded stack busy. Stop at Content-Length.
        let mut response = Vec::new();
        let mut chunk = [0u8; 2048];
        let mut expected_end: Option<usize> = None;
        while expected_end.is_none_or(|end| response.len() < end) && response.len() < 256 * 1024 {
            let n = stream.read(&mut chunk).map_err(io("web read"))?;
            if n == 0 {
                break;
            }
            response.extend_from_slice(&chunk[..n]);
            if expected_end.is_none()
                && let Some(header_end) = response.windows(4).position(|w| w == b"\r\n\r\n")
            {
                let head = String::from_utf8_lossy(&response[..header_end]).to_ascii_lowercase();
                let length = head
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:"))
                    .and_then(|v| v.trim().parse::<usize>().ok());
                // Without a length, fall back to reading until the server closes.
                expected_end = Some(length.map_or(usize::MAX, |n| header_end + 4 + n));
            }
        }
        let _ = stream.shutdown(std::net::Shutdown::Both);
        let text = String::from_utf8_lossy(&response).into_owned();
        if !text.starts_with("HTTP/1.") || !text.lines().next().is_some_and(|l| l.contains(" 200"))
        {
            return Err(PtouchError::StatusError(
                "Printer web page did not return 200".into(),
            ));
        }
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::VecDeque,
        net::TcpListener,
        sync::{Arc, Mutex},
        thread,
    };

    const PAGE: &str = "HTTP/1.1 200 OK\r\nContent-Length: 145\r\n\r\n<dl class=\"items\"><dt>Emulation</dt><dd>Raster</dd><dt>Media&#32;Status</dt><dd>Not&#32;Empty</dd><dt>Media&#32;Type</dt><dd>36mm(1.4\")</dd></dl>";
    const DEVICE_ID: &[u8] = b"MFG:Brother;CMD:PJL;MDL:PT-E850TKW;CLS:PRINTER;";

    fn snmp_response(id: i32, values: &[(&str, SnmpValue)]) -> Vec<u8> {
        let mut varbinds = Vec::new();
        for (oid, value) in values {
            let mut pair = encode_oid(oid).unwrap();
            pair.extend(match value {
                SnmpValue::Int(v) => ber_int(*v),
                SnmpValue::Bytes(b) => ber_tlv(0x04, b),
                _ => vec![0x05, 0x00],
            });
            varbinds.extend(ber_tlv(0x30, &pair));
        }
        let mut pdu = ber_int(i64::from(id));
        pdu.extend(ber_int(0));
        pdu.extend(ber_int(0));
        pdu.extend(ber_tlv(0x30, &varbinds));
        let mut message = ber_int(0);
        message.extend(ber_tlv(0x04, b"public"));
        message.extend(ber_tlv(0xA2, &pdu));
        ber_tlv(0x30, &message)
    }

    type Received = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

    /// Fake printer: web page, SNMP responder with a scripted state sequence,
    /// and an LPD server that records the received data file.
    struct FakePrinter {
        endpoints: Endpoints,
        received: Received,
        states: Arc<Mutex<VecDeque<i64>>>,
        error_flags: Arc<Mutex<u8>>,
    }

    fn fake_printer(page: &'static str, states: &[i64]) -> FakePrinter {
        let http = TcpListener::bind("127.0.0.1:0").unwrap();
        let lpr = TcpListener::bind("127.0.0.1:0").unwrap();
        let snmp = UdpSocket::bind("127.0.0.1:0").unwrap();
        let endpoints = Endpoints {
            lpr: lpr.local_addr().unwrap(),
            snmp: snmp.local_addr().unwrap(),
            http: http.local_addr().unwrap(),
        };
        let received = Arc::new(Mutex::new(Vec::new()));
        let states = Arc::new(Mutex::new(states.iter().copied().collect::<VecDeque<_>>()));
        let error_flags = Arc::new(Mutex::new(0u8));

        thread::spawn(move || {
            for stream in http.incoming() {
                let Ok(mut stream) = stream else { break };
                let mut request = Vec::new();
                let mut byte = [0u8; 1];
                while !request.ends_with(b"\r\n\r\n") && stream.read_exact(&mut byte).is_ok() {
                    request.push(byte[0]);
                }
                let _ = stream.write_all(page.as_bytes());
                // like the real printer, do not close right away
                thread::spawn(move || {
                    thread::sleep(Duration::from_secs(3));
                    drop(stream);
                });
            }
        });

        let (st, er) = (states.clone(), error_flags.clone());
        thread::spawn(move || {
            let mut buf = [0u8; 2048];
            while let Ok((n, from)) = snmp.recv_from(&mut buf) {
                let (id, oids) = {
                    // reuse the parser: requests carry NULL values
                    let (id, vbs) = snmp_parse(&buf[..n]).unwrap();
                    (id, vbs.into_iter().map(|(o, _)| o).collect::<Vec<_>>())
                };
                let values: Vec<(&str, SnmpValue)> = oids
                    .iter()
                    .map(|oid| match oid.as_str() {
                        OID_DEVICE_ID => (OID_DEVICE_ID, SnmpValue::Bytes(DEVICE_ID.to_vec())),
                        OID_ERROR_STATE => {
                            (OID_ERROR_STATE, SnmpValue::Bytes(vec![*er.lock().unwrap()]))
                        }
                        _ => {
                            let mut q = st.lock().unwrap();
                            let v = if q.len() > 1 {
                                q.pop_front().unwrap()
                            } else {
                                *q.front().unwrap_or(&3)
                            };
                            (OID_PRINTER_STATUS, SnmpValue::Int(v))
                        }
                    })
                    .collect();
                let _ = snmp.send_to(&snmp_response(id, &values), from);
            }
        });

        let rec = received.clone();
        thread::spawn(move || {
            for stream in lpr.incoming() {
                let Ok(mut stream) = stream else { break };
                let mut files = Vec::new();
                let mut byte = [0u8; 1];
                // queue line
                let mut line = Vec::new();
                loop {
                    stream.read_exact(&mut byte).unwrap();
                    if byte[0] == b'\n' {
                        break;
                    }
                    line.push(byte[0]);
                }
                stream.write_all(&[0]).unwrap();
                for _ in 0..2 {
                    let mut header = Vec::new();
                    loop {
                        stream.read_exact(&mut byte).unwrap();
                        if byte[0] == b'\n' {
                            break;
                        }
                        header.push(byte[0]);
                    }
                    stream.write_all(&[0]).unwrap();
                    let text = String::from_utf8_lossy(&header[1..]).into_owned();
                    let (len, name) = text.split_once(' ').unwrap();
                    let mut body = vec![0u8; len.parse::<usize>().unwrap() + 1];
                    stream.read_exact(&mut body).unwrap();
                    body.pop();
                    stream.write_all(&[0]).unwrap();
                    files.push((format!("{}{}", char::from(header[0]), name), body));
                }
                rec.lock().unwrap().extend(files);
                rec.lock().unwrap().push((
                    format!("queue:{}", String::from_utf8_lossy(&line[1..])),
                    vec![],
                ));
            }
        });

        FakePrinter {
            endpoints,
            received,
            states,
            error_flags,
        }
    }

    fn lines(n: usize) -> Vec<Vec<u8>> {
        vec![vec![0x80; 70]; n]
    }

    #[test]
    fn oid_encoding_matches_the_captured_bytes() {
        // hrPrinterStatus.1 as sent by the Windows driver (capture 2)
        assert_eq!(
            encode_oid(OID_PRINTER_STATUS).unwrap(),
            [
                0x06, 0x0B, 0x2B, 0x06, 0x01, 0x02, 0x01, 0x19, 0x03, 0x05, 0x01, 0x01, 0x01
            ]
        );
        assert!(encode_oid("1.x.3").is_err());
        assert!(encode_oid("7.1").is_err());
    }

    #[test]
    fn snmp_request_round_trips() {
        let request =
            snmp_get_request("public", 1234, &[OID_PRINTER_STATUS, OID_DEVICE_ID]).unwrap();
        let (id, values) = snmp_parse(&request).unwrap();
        assert_eq!(id, 1234);
        assert_eq!(values[0].0, OID_PRINTER_STATUS);
        assert_eq!(values[1].0, OID_DEVICE_ID);
    }

    #[test]
    fn snmp_rejects_garbage_and_errors() {
        assert!(snmp_parse(&[0x30, 0x05, 0x02]).is_err());
        assert!(snmp_parse(&[]).is_err());
    }

    #[test]
    fn media_width_is_read_from_the_real_page_markup() {
        assert_eq!(parse_media_width_mm(PAGE), Some(36));
        assert_eq!(
            parse_media_width_mm("<dt>Media&#32;Type</dt><dd>9mm(0.35\")</dd>"),
            Some(9)
        );
        assert_eq!(parse_media_width_mm("<dd>nothing here</dd>"), None);
    }

    #[test]
    fn control_file_matches_the_capture_layout() {
        let control = lpr_control_file("PC", "admin", "Layout2", "dfA005PC");
        assert_eq!(
            control,
            b"HPC\nPadmin\nJLayout2\nldfA005PC\nUdfA005PC\nNLayout2\n"
        );
    }

    #[test]
    fn printer_state_and_error_description() {
        assert_eq!(PrinterState::from(3), PrinterState::Idle);
        assert_eq!(PrinterState::from(1), PrinterState::Other);
        let status = NetworkStatus {
            state: PrinterState::Idle,
            error_flags: 0x08,
        };
        assert!(!status.is_ready());
        assert!(status.describe().contains("cover open"));
        assert!(
            NetworkStatus {
                state: PrinterState::Idle,
                error_flags: 0
            }
            .is_ready()
        );
    }

    #[test]
    fn open_reads_identity_and_media() {
        let fake = fake_printer(PAGE, &[3]);
        let printer = NetworkPrinter::open_with(fake.endpoints).unwrap();
        assert_eq!(printer.model_name(), "PT-E850TKW");
        assert_eq!(printer.media_width_mm(), 36);
        assert_eq!(printer.tape_width_px(), Some(454));
        assert_eq!(printer.band_left_px(), Some(61));
    }

    #[test]
    fn print_submits_the_exact_job_via_lpr_and_waits_for_idle() {
        let fake = fake_printer(PAGE, &[3, 3, 4, 4, 3]);
        let mut printer = NetworkPrinter::open_with(fake.endpoints).unwrap();
        printer.set_job_timeout(Duration::from_secs(20));
        let raster = lines(60);
        printer
            .print_raster(&raster, false, true, PrintQuality::Standard)
            .unwrap();

        let received = fake.received.lock().unwrap().clone();
        let data = received
            .iter()
            .find(|(n, _)| n.starts_with('\u{3}'))
            .expect("data file");
        let control = received
            .iter()
            .find(|(n, _)| n.starts_with('\u{2}'))
            .expect("control file");
        assert!(String::from_utf8_lossy(&control.1).starts_with("Hptouch\nPptouch\n"));
        assert!(received.iter().any(|(n, _)| n == "queue:BINARY_P1"));
        // job = 200 zeros + ESC @ + header exactly as the Brother capture orders it
        assert_eq!(&data.1[..200], &[0u8; 200][..]);
        assert_eq!(&data.1[200..202], &[0x1B, 0x40]);
        assert_eq!(&data.1[202..206], &[0x1B, 0x69, 0x61, 0x01]);
        assert_eq!(&data.1[206..209], &[0x1B, 0x69, 0x55]);
        assert_eq!(*data.1.last().unwrap(), 0x1A);
        let k = data
            .1
            .windows(4)
            .position(|w| w == [0x1B, 0x69, 0x4B, 0x0C]);
        assert!(k.is_some(), "half-cut + cut-at-end expected");
    }

    #[test]
    fn print_refuses_when_not_idle() {
        let fake = fake_printer(PAGE, &[3]);
        let mut printer = NetworkPrinter::open_with(fake.endpoints).unwrap();
        *fake.error_flags.lock().unwrap() = 0x08;
        let err = printer
            .print_raster(&lines(10), false, true, PrintQuality::Standard)
            .unwrap_err();
        assert!(err.to_string().contains("not ready"), "{err}");
        assert!(
            fake.received.lock().unwrap().is_empty(),
            "nothing may reach the queue"
        );
        fake.states.lock().unwrap().clear();
        fake.states.lock().unwrap().push_back(1); // other(1)
        *fake.error_flags.lock().unwrap() = 0;
        assert!(
            printer
                .print_raster(&lines(10), false, true, PrintQuality::Standard)
                .is_err()
        );
        assert!(fake.received.lock().unwrap().is_empty());
    }

    #[test]
    fn print_refuses_chain_bad_lines_quality_and_unknown_media() {
        let fake = fake_printer(PAGE, &[3]);
        let mut printer = NetworkPrinter::open_with(fake.endpoints).unwrap();
        assert!(matches!(
            printer.print_raster(&lines(5), true, true, PrintQuality::Standard),
            Err(PtouchError::UnsupportedOperation(_))
        ));
        assert!(
            printer
                .print_raster(&[vec![0u8; 16]], false, true, PrintQuality::Standard)
                .is_err()
        );
        assert!(
            printer
                .print_raster(&[], false, true, PrintQuality::Standard)
                .is_err()
        );
        assert!(matches!(
            printer.print_raster(&lines(5), false, true, PrintQuality::HighRes),
            Err(PtouchError::UnsupportedQuality(_))
        ));
        assert!(fake.received.lock().unwrap().is_empty());

        let odd = fake_printer(
            "HTTP/1.0 200 OK\r\n\r\n<dt>Media&#32;Type</dt><dd>21mm</dd>",
            &[3],
        );
        let mut printer = NetworkPrinter::open_with(odd.endpoints).unwrap();
        assert!(
            printer
                .print_raster(&lines(5), false, true, PrintQuality::Standard)
                .is_err()
        );
        assert!(odd.received.lock().unwrap().is_empty());
    }

    #[test]
    fn open_rejects_other_models() {
        let fake = fake_printer(PAGE, &[3]);
        // a responder that does not identify as the E850: point SNMP at the HTTP fake's UDP-less port
        let dead = UdpSocket::bind("127.0.0.1:0").unwrap();
        let endpoints = Endpoints {
            snmp: dead.local_addr().unwrap(),
            ..fake.endpoints
        };
        assert!(NetworkPrinter::open_with(endpoints).is_err());
    }
}
