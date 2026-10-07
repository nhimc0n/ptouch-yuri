// SPDX-License-Identifier: GPL-3.0-or-later

//! Read and write the queue's driver defaults with the system's own tools
//! (`lpoptions`, `lpstat`, `lpadmin`). Members of the `_lpadmin` group, which
//! includes macOS administrators, can change them without a password.
//!
//! The parsing and the argument building are pure functions so they can be
//! tested without a print queue.

use serde::{Deserialize, Serialize};
use std::process::Command;

/// Name of the print queue created by `scripts/install-cups-macos.sh`.
pub const QUEUE: &str = "PT-E850TKW";
/// LPR queue on the printer (E850-verified).
pub const LPR_QUEUE: &str = "BINARY_P1";

/// Errors are shown to the user, so they are plain sentences.
pub type Result<T> = std::result::Result<T, String>;

/// Print quality, the `LabelQuality` driver option.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Quality {
    /// 360 x 360 dpi.
    Normal,
    /// Priority to print quality: slower, same resolution.
    High,
    /// 360 x 720 dpi: slowest.
    HiRes,
}

impl Quality {
    /// The PPD choice keyword.
    pub fn keyword(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::High => "High",
            Self::HiRes => "HiRes",
        }
    }

    fn from_keyword(keyword: &str) -> Option<Self> {
        match keyword {
            "Normal" => Some(Self::Normal),
            "High" => Some(Self::High),
            "HiRes" => Some(Self::HiRes),
            _ => None,
        }
    }
}

/// The driver settings stored in the queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverSettings {
    /// Cut the label but keep the backing paper.
    pub half_cut: bool,
    /// Cut through label and backing. Half cut wins when both are on; neither
    /// means no cut.
    pub full_cut: bool,
    /// Chain printing (refused by the filter until it is verified).
    pub chain: bool,
    /// Mirror print.
    pub mirror: bool,
    /// Print quality.
    pub quality: Quality,
    /// Default label size (a PPD page size keyword such as `Auto`).
    pub page_size: String,
    /// Page sizes the queue offers, in PPD order, without the custom entry.
    pub page_sizes: Vec<String>,
}

/// One line of `lpoptions -l`: `Key/Label: choice *default choice`.
fn option_line(line: &str) -> Option<(&str, Vec<&str>, Option<&str>)> {
    let (head, choices) = line.rsplit_once(": ")?;
    let key = head.split('/').next()?.trim();
    let choices: Vec<&str> = choices.split_whitespace().collect();
    let default = choices.iter().find_map(|c| c.strip_prefix('*'));
    Some((key, choices, default))
}

/// Parse the output of `lpoptions -p <queue> -l`.
///
/// An option the queue does not have yet (an older PPD) keeps its built-in
/// default, so the app still opens before the driver is reinstalled.
pub fn parse_lpoptions(text: &str) -> Result<DriverSettings> {
    let mut settings = DriverSettings {
        half_cut: true,
        full_cut: false,
        chain: false,
        mirror: false,
        quality: Quality::Normal,
        page_size: String::new(),
        page_sizes: Vec::new(),
    };
    let mut seen = false;
    for line in text.lines() {
        let Some((key, choices, default)) = option_line(line) else {
            continue;
        };
        let on = default == Some("True");
        match key {
            "HalfCut" => settings.half_cut = on,
            "FullCut" => settings.full_cut = on,
            "Chain" => settings.chain = on,
            "MirrorPrint" => settings.mirror = on,
            "LabelQuality" => {
                if let Some(quality) = default.and_then(Quality::from_keyword) {
                    settings.quality = quality;
                }
            }
            "PageSize" => {
                settings.page_size = default.unwrap_or_default().to_string();
                settings.page_sizes = choices
                    .iter()
                    .map(|c| c.trim_start_matches('*'))
                    .filter(|c| !c.starts_with("Custom."))
                    .map(str::to_string)
                    .collect();
            }
            _ => continue,
        }
        seen = true;
    }
    if !seen {
        return Err(format!(
            "The print queue {QUEUE} was not found. Install the driver first."
        ));
    }
    Ok(settings)
}

/// Arguments for `lpadmin` that store these settings as the queue defaults.
pub fn lpadmin_args(queue: &str, settings: &DriverSettings) -> Vec<String> {
    let flag = |on: bool| if on { "True" } else { "False" };
    let mut args = vec!["-p".to_string(), queue.to_string()];
    let mut option = |key: &str, value: &str| {
        args.push("-o".to_string());
        args.push(format!("{key}={value}"));
    };
    option("HalfCut", flag(settings.half_cut));
    option("FullCut", flag(settings.full_cut));
    option("Chain", flag(settings.chain));
    option("MirrorPrint", flag(settings.mirror));
    option("LabelQuality", settings.quality.keyword());
    if !settings.page_size.is_empty() {
        option("PageSize", &settings.page_size);
    }
    args
}

/// Device URI from `lpstat -v <queue>` (`device for NAME: lpd://host/QUEUE`).
pub fn parse_device_uri(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix("device for "))
        .and_then(|rest| rest.split_once(": "))
        .map(|(_, uri)| uri.trim().to_string())
}

/// Host part of an `lpd://host[:port]/queue` URI.
pub fn host_of_uri(uri: &str) -> Option<String> {
    let rest = uri.split_once("://")?.1;
    let authority = rest.split(['/', '?']).next()?;
    let host = authority.rsplit('@').next()?;
    let host = match host.rsplit_once(':') {
        Some((name, port)) if port.chars().all(|c| c.is_ascii_digit()) => name,
        _ => host,
    };
    (!host.is_empty()).then(|| host.to_string())
}

/// The `lpd://` URI the queue must use for a printer at `host`.
///
/// Rejects anything that is not a plain host name or address, because the
/// value ends up on a command line.
pub fn device_uri_for(host: &str) -> Result<String> {
    let host = host.trim();
    let plain = !host.is_empty()
        && host.len() <= 253
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':'));
    if !plain {
        return Err(
            "Enter the printer's IP address or host name, for example 192.168.99.107.".into(),
        );
    }
    Ok(format!("lpd://{host}/{LPR_QUEUE}"))
}

fn run(program: &str, args: &[String]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("Could not run {program}: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);
    // lpoptions exits 0 even when the queue is missing, so look at stderr too.
    if !output.status.success() || (stdout.trim().is_empty() && !stderr.trim().is_empty()) {
        return Err(stderr.trim().to_string());
    }
    Ok(stdout)
}

/// Read the driver settings stored in the queue.
pub fn read_settings(queue: &str) -> Result<DriverSettings> {
    let text = run("lpoptions", &["-p".into(), queue.into(), "-l".into()])
        .map_err(|_| format!("The print queue {queue} was not found. Install the driver first."))?;
    parse_lpoptions(&text)
}

/// Store the driver settings in the queue. They apply to the next job.
pub fn apply_settings(queue: &str, settings: &DriverSettings) -> Result<()> {
    run("lpadmin", &lpadmin_args(queue, settings)).map(|_| ())
}

/// Address of the printer the queue sends to.
pub fn read_printer_host(queue: &str) -> Result<String> {
    let text = run("lpstat", &["-v".into(), queue.into()])?;
    parse_device_uri(&text)
        .as_deref()
        .and_then(host_of_uri)
        .ok_or_else(|| format!("The print queue {queue} has no printer address."))
}

/// Point the queue at a printer.
pub fn set_printer_host(queue: &str, host: &str) -> Result<()> {
    let uri = device_uri_for(host)?;
    run("lpadmin", &["-p".into(), queue.into(), "-v".into(), uri]).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real `lpoptions -p PT-E850TKW -l` output (2026-10-07) plus MirrorPrint.
    const LPOPTIONS: &str = "\
PageSize/Label Size: *Auto L50 L75 L100 L150 L200 L300 P50 P75 P100 P150 P200 AutoP Auto9 S9L50 S9L100 S9L200 Custom.WIDTHxHEIGHT
Resolution/Resolution: *360dpi
ColorModel/Color Mode: *Gray
HalfCut/Half cut (cut the label, keep the backing paper): *True False
FullCut/Full cut (cut through the label and the backing paper): True *False
Chain/Chain printing (no feed or cut at the end, for continuous labels): True *False
MirrorPrint/Mirror print (to read through a clear label): True *False
LabelQuality/Print Quality: *Normal High HiRes
";

    #[test]
    fn reads_the_defaults_marked_with_a_star() {
        let settings = parse_lpoptions(LPOPTIONS).unwrap();
        assert!(settings.half_cut && !settings.full_cut && !settings.chain && !settings.mirror);
        assert_eq!(settings.quality, Quality::Normal);
        assert_eq!(settings.page_size, "Auto");
        assert_eq!(settings.page_sizes.len(), 17);
        assert_eq!(settings.page_sizes[0], "Auto");
        assert!(!settings.page_sizes.iter().any(|s| s.starts_with("Custom")));
    }

    #[test]
    fn reads_changed_defaults() {
        let text = LPOPTIONS
            .replace("*True False\nFullCut", "True *False\nFullCut")
            .replace(
                "FullCut/Full cut (cut through the label and the backing paper): True *False",
                "FullCut/Full cut (cut through the label and the backing paper): *True False",
            )
            .replace("*Normal High HiRes", "Normal High *HiRes")
            .replace("*Auto L50", "Auto *L50")
            .replace(
                "MirrorPrint/Mirror print (to read through a clear label): True *False",
                "MirrorPrint/Mirror print (to read through a clear label): *True False",
            );
        let settings = parse_lpoptions(&text).unwrap();
        assert!(!settings.half_cut && settings.full_cut && settings.mirror);
        assert_eq!(settings.quality, Quality::HiRes);
        assert_eq!(settings.page_size, "L50");
    }

    #[test]
    fn an_older_queue_without_new_options_keeps_built_in_defaults() {
        let old = "PageSize/Label Size: *Auto L50\nCutMode/Cut: *Half Full\n";
        let settings = parse_lpoptions(old).unwrap();
        assert!(settings.half_cut && !settings.mirror);
        assert_eq!(settings.quality, Quality::Normal);
    }

    #[test]
    fn a_missing_queue_is_reported_in_plain_words() {
        let err = parse_lpoptions("").unwrap_err();
        assert!(err.contains("Install the driver"), "{err}");
    }

    #[test]
    fn settings_become_lpadmin_arguments_and_round_trip() {
        let mut settings = parse_lpoptions(LPOPTIONS).unwrap();
        settings.full_cut = true;
        settings.half_cut = false;
        settings.mirror = true;
        settings.quality = Quality::High;
        settings.page_size = "AutoP".into();
        assert_eq!(
            lpadmin_args(QUEUE, &settings),
            [
                "-p",
                "PT-E850TKW",
                "-o",
                "HalfCut=False",
                "-o",
                "FullCut=True",
                "-o",
                "Chain=False",
                "-o",
                "MirrorPrint=True",
                "-o",
                "LabelQuality=High",
                "-o",
                "PageSize=AutoP"
            ]
        );
    }

    #[test]
    fn device_uri_parsing_and_building() {
        let uri =
            parse_device_uri("device for PT-E850TKW: lpd://192.168.99.107/BINARY_P1\n").unwrap();
        assert_eq!(uri, "lpd://192.168.99.107/BINARY_P1");
        assert_eq!(host_of_uri(&uri).as_deref(), Some("192.168.99.107"));
        assert_eq!(
            host_of_uri("lpd://printer.local:515/q").as_deref(),
            Some("printer.local")
        );
        assert_eq!(parse_device_uri("nothing"), None);
        assert_eq!(
            device_uri_for(" 192.168.99.107 ").unwrap(),
            "lpd://192.168.99.107/BINARY_P1"
        );
        assert_eq!(
            device_uri_for("BRW123.local").unwrap(),
            "lpd://BRW123.local/BINARY_P1"
        );
    }

    #[test]
    fn a_printer_address_cannot_smuggle_anything_onto_the_command_line() {
        for bad in [
            "",
            "  ",
            "a b",
            "x;rm -rf /",
            "host/../q",
            "-o evil",
            "$(id)",
            "a\nb",
        ] {
            assert!(device_uri_for(bad).is_err(), "{bad:?} must be rejected");
        }
    }
}
