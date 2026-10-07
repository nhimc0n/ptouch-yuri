// SPDX-License-Identifier: GPL-3.0-or-later

//! Live state of the printer for the settings app. The app is not sandboxed,
//! so unlike the CUPS filter it can ask the printer directly (SNMP and the
//! printer's web page, through `ptouch-core`). Read-only: nothing is sent to
//! the print queue.

use crate::queue;
use ptouch_core::{NetworkPrinter, network::PrinterState};
use serde::Serialize;

/// What the app shows about the printer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PrinterStatus {
    /// Model name.
    pub model: String,
    /// Idle with no error: a job can be sent.
    pub ready: bool,
    /// Short state keyword: `idle`, `printing`, `error`, `warmup` or `unknown`.
    pub state: &'static str,
    /// Width of the loaded tape in mm.
    pub tape_mm: u8,
    /// Whether this tape width has a verified print band.
    pub tape_supported: bool,
}

/// State keyword for the UI. The printer reports `other` while it shows an
/// error on its panel.
pub fn state_keyword(state: PrinterState, error_flags: u8) -> &'static str {
    match state {
        _ if error_flags != 0 => "error",
        PrinterState::Idle => "idle",
        PrinterState::Printing => "printing",
        PrinterState::Warmup => "warmup",
        PrinterState::Other => "error",
        PrinterState::Unknown | PrinterState::Unrecognized(_) => "unknown",
    }
}

/// Ask the printer at `host` for its state and loaded tape.
pub fn read_status(host: &str) -> Result<PrinterStatus, String> {
    let printer = NetworkPrinter::open(host)
        .map_err(|e| format!("Cannot reach the printer at {host}: {e}"))?;
    let status = printer
        .status()
        .map_err(|e| format!("The printer at {host} did not report its state: {e}"))?;
    Ok(PrinterStatus {
        model: printer.model_name().to_string(),
        ready: status.is_ready(),
        state: state_keyword(status.state, status.error_flags),
        tape_mm: printer.media_width_mm(),
        tape_supported: printer.band_left_px().is_some(),
    })
}

/// Everything the status card needs, in one value the UI can show as it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PrinterInfo {
    /// Printer address the queue sends to, if the queue exists.
    pub host: Option<String>,
    /// Live state, if the printer answered.
    pub status: Option<PrinterStatus>,
    /// Plain-language reason when something is missing.
    pub error: Option<String>,
}

/// Read the queue's printer address and ask that printer for its state.
/// Never fails: problems come back in `error` so the app can show them.
pub fn read_printer(queue_name: &str) -> PrinterInfo {
    let host = match queue::read_printer_host(queue_name) {
        Ok(host) => host,
        Err(error) => {
            return PrinterInfo {
                host: None,
                status: None,
                error: Some(error),
            };
        }
    };
    match read_status(&host) {
        Ok(status) => PrinterInfo {
            host: Some(host),
            status: Some(status),
            error: None,
        },
        Err(error) => PrinterInfo {
            host: Some(host),
            status: None,
            error: Some(error),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_keywords() {
        assert_eq!(state_keyword(PrinterState::Idle, 0), "idle");
        assert_eq!(state_keyword(PrinterState::Printing, 0), "printing");
        // the printer says "other" while its panel shows an error
        assert_eq!(state_keyword(PrinterState::Other, 0), "error");
        assert_eq!(state_keyword(PrinterState::Idle, 0x08), "error");
        assert_eq!(state_keyword(PrinterState::Unknown, 0), "unknown");
    }
}
