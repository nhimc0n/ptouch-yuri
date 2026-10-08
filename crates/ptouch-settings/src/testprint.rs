// SPDX-License-Identifier: GPL-3.0-or-later

//! Test print for the settings app.
//!
//! The label goes through the print queue (`lp`), exactly like a print from a
//! browser, so it uses the driver settings saved in the queue and proves that
//! the filter reads them. Before sending, the app (which is not sandboxed,
//! unlike the CUPS filter) asks the printer for its state and loaded tape, and
//! picks a page size that matches that tape: a 36 mm label sent to a 9 mm
//! cassette is rejected by the printer and leaves it in an error state.

use crate::{
    queue::{self, Result},
    status,
};
use std::{path::PathBuf, process::Command};

/// A test label for one tape width.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plan {
    /// Tape the label is for.
    pub tape_mm: u8,
    /// PPD page size keyword; the PDF is exactly this size.
    pub page_size: &'static str,
    /// Page width (label length) in points.
    pub width_pt: f64,
    /// Page height (tape width) in points.
    pub height_pt: f64,
}

/// The test label for the loaded tape, if there is one.
pub fn plan_for(tape_mm: u8) -> Option<Plan> {
    match tape_mm {
        36 => Some(Plan {
            tape_mm,
            page_size: "L50",
            width_pt: 141.73,
            height_pt: 102.05,
        }),
        9 => Some(Plan {
            tape_mm,
            page_size: "S9L50",
            width_pt: 141.73,
            height_pt: 25.51,
        }),
        _ => None,
    }
}

/// A one-page PDF with the test label: the model name, the tape, and on the
/// wide tape a frame that shows the centring and the margins.
pub fn pdf(plan: &Plan) -> Vec<u8> {
    let (w, h) = (plan.width_pt, plan.height_pt);
    let content = if plan.tape_mm >= 24 {
        format!(
            "1 w 8 8 {fw:.2} {fh:.2} re S\n\
             BT /F1 18 Tf 18 {y1:.2} Td (PT-E850TKW) Tj ET\n\
             BT /F1 10 Tf 18 {y2:.2} Td (Test print, {mm} mm tape) Tj ET\n",
            fw = w - 16.0,
            fh = h - 16.0,
            y1 = h / 2.0 + 2.0,
            y2 = h / 2.0 - 16.0,
            mm = plan.tape_mm,
        )
    } else {
        format!(
            "BT /F1 10 Tf 6 {y:.2} Td (PT-E850TKW test, {mm} mm) Tj ET\n",
            y = h / 2.0 - 3.5,
            mm = plan.tape_mm,
        )
    };
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w:.2} {h:.2}] /Contents 4 0 R \
             /Resources << /Font << /F1 5 0 R >> >> >>"
        ),
        format!(
            "<< /Length {} >>\nstream\n{content}endstream",
            content.len()
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold >>".to_string(),
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n{body}\nendobj\n", index + 1).bytes());
    }
    let xref_at = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).bytes());
    for offset in offsets {
        out.extend(format!("{offset:010} 00000 n \n").bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n",
            objects.len() + 1
        )
        .bytes(),
    );
    out
}

/// Job id from `lp`'s answer: `request id is PT-E850TKW-12 (1 file(s))`.
pub fn parse_job_id(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix("request id is "))
        .and_then(|rest| rest.split_whitespace().next())
        .map(str::to_string)
}

fn temp_file() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    std::env::temp_dir().join(format!("ptouch-test-{}-{nanos}.pdf", std::process::id()))
}

/// Print the test label. Returns the print job id.
///
/// Refuses when the printer cannot be reached, is not idle, or has a tape
/// without a test label. Every other driver setting comes from the queue.
pub fn print_test_label(queue_name: &str) -> Result<String> {
    let host = queue::read_printer_host(queue_name)?;
    let printer = status::read_status(&host)?;
    if !printer.ready {
        return Err(format!(
            "The printer is not ready (state: {}). Check it and try again.",
            printer.state
        ));
    }
    let plan = plan_for(printer.tape_mm).ok_or_else(|| {
        format!(
            "A test label is not available for {} mm tape.",
            printer.tape_mm
        )
    })?;

    let path = temp_file();
    std::fs::write(&path, pdf(&plan))
        .map_err(|e| format!("Could not write the test label: {e}"))?;
    let output = Command::new("lp")
        .args(["-d", queue_name, "-t", "PT-E850TKW test print", "-o"])
        .arg(format!("PageSize={}", plan.page_size))
        .arg(&path)
        .output();
    // lp has copied the file into the spool by the time it returns.
    let _ = std::fs::remove_file(&path);
    let output = output.map_err(|e| format!("Could not send the test label: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        return Err(format!(
            "Could not send the test label: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(parse_job_id(&stdout).unwrap_or_else(|| queue_name.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_exist_for_the_verified_tapes_only() {
        assert_eq!(plan_for(36).unwrap().page_size, "L50");
        assert_eq!(plan_for(9).unwrap().page_size, "S9L50");
        assert!(plan_for(12).is_none());
        assert!(plan_for(0).is_none());
    }

    #[test]
    fn pdf_is_well_formed_and_has_the_page_size() {
        for tape in [36, 9] {
            let plan = plan_for(tape).unwrap();
            let bytes = pdf(&plan);
            let text = String::from_utf8(bytes.clone()).unwrap();
            assert!(text.starts_with("%PDF-1.4\n"));
            assert!(text.ends_with("%%EOF\n"));
            assert!(text.contains(&format!(
                "/MediaBox [0 0 {:.2} {:.2}]",
                plan.width_pt, plan.height_pt
            )));
            // every xref entry points at "<n> 0 obj"
            let xref = text.rfind("xref\n").unwrap();
            let entries: Vec<&str> = text[xref..].lines().skip(3).take(5).collect();
            for (index, entry) in entries.iter().enumerate() {
                let offset: usize = entry[..10].parse().unwrap();
                assert!(
                    text[offset..].starts_with(&format!("{} 0 obj", index + 1)),
                    "object {} is not at {offset}",
                    index + 1
                );
            }
            // the declared stream length is the real one
            let declared: usize = text
                .split("/Length ")
                .nth(1)
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap()
                .parse()
                .unwrap();
            let start = text.find("stream\n").unwrap() + "stream\n".len();
            let end = text.find("endstream").unwrap();
            assert_eq!(end - start, declared);
        }
    }

    #[test]
    fn reads_the_job_id() {
        assert_eq!(
            parse_job_id("request id is PT-E850TKW-12 (1 file(s))\n").as_deref(),
            Some("PT-E850TKW-12")
        );
        assert_eq!(parse_job_id("lp: error"), None);
    }
}
