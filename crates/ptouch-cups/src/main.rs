// SPDX-License-Identifier: GPL-3.0-or-later

//! `rastertoptouch`: CUPS filter for the Brother PT-E850TKW.
//!
//! CUPS runs it as `rastertoptouch job-id user title copies options [file]`
//! with the CUPS raster on stdin (or `file`) and `DEVICE_URI` set, and passes
//! its stdout to the backend (`lpd://<printer>/BINARY_P1`). Before it writes a
//! single byte it asks the printer for its state and loaded tape and refuses
//! on any mismatch, so a wrong cassette fails the job instead of wasting tape.
//!
//! `rastertoptouch --dry-run in.ras out.bin [options]` only converts a raster
//! file to a job file. It never contacts a printer.

use ptouch_core::{NetworkPrinter, protocol::PrintQuality};
use ptouch_cups::{
    CutMode, Label, Options, Result, build_job_offline, host_from_device_uri, page_to_label,
    parse_options, read_raster,
};
use std::{
    env,
    fs::File,
    io::{self, Read, Write},
    process::ExitCode,
};

fn load_label(input: impl Read) -> Result<Label> {
    let mut pages = read_raster(input)?;
    match pages.len() {
        0 => Err("the document has no pages".into()),
        1 => page_to_label(&pages.remove(0)),
        n => Err(format!(
            "the document has {n} pages; print one label at a time (set Pages to 1)"
        )),
    }
}

fn dry_run(args: &[String]) -> Result<()> {
    let [input, output, rest @ ..] = args else {
        return Err("usage: rastertoptouch --dry-run in.ras out.bin [options]".into());
    };
    let options = parse_options(&rest.join(" "));
    let label = load_label(File::open(input).map_err(|e| format!("{input}: {e}"))?)?;
    let job = build_job_offline(&label, &options)?;
    std::fs::write(output, &job).map_err(|e| format!("{output}: {e}"))?;
    eprintln!(
        "INFO: {} lines for {} mm tape, {} bytes written to {output}",
        label.lines.len(),
        label.tape_mm,
        job.len()
    );
    Ok(())
}

fn filter(args: &[String]) -> Result<()> {
    if args.len() < 5 {
        return Err("usage: rastertoptouch job-id user title copies options [file]".into());
    }
    let options: Options = parse_options(&args[4]);
    if options.copies > 1 {
        return Err("print one copy at a time; multiple copies are not supported yet".into());
    }
    let uri = env::var("DEVICE_URI").map_err(|_| "DEVICE_URI is not set".to_string())?;
    let host = host_from_device_uri(&uri)
        .ok_or_else(|| format!("cannot find a printer address in {uri}"))?;

    let label = match args.get(5) {
        Some(path) => load_label(File::open(path).map_err(|e| format!("{path}: {e}"))?)?,
        None => load_label(io::stdin().lock())?,
    };

    let mut printer = NetworkPrinter::open(&host)
        .map_err(|e| format!("cannot reach the printer at {host}: {e}"))?;
    printer.set_half_cut(options.cut == CutMode::Half);
    let job = printer
        .prepare_job(
            &label.lines,
            false,
            true,
            PrintQuality::Standard,
            Some(label.tape_mm),
        )
        .map_err(|e| e.to_string())?;
    io::stdout()
        .lock()
        .write_all(&job)
        .map_err(|e| format!("cannot write the job: {e}"))?;
    eprintln!(
        "INFO: label of {} lines for {} mm tape sent to the backend",
        label.lines.len(),
        label.tape_mm
    );
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("--dry-run") => dry_run(&args[1..]),
        _ => filter(&args),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("ERROR: {message}");
            ExitCode::FAILURE
        }
    }
}
