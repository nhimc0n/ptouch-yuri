// SPDX-License-Identifier: GPL-3.0-or-later

//! `rastertoptouch`: CUPS filter for the Brother PT-E850TKW.
//!
//! CUPS runs it as `rastertoptouch job-id user title copies options [file]`
//! with the CUPS raster on stdin (or `file`) and passes its stdout to the
//! backend (`lpd://<printer>/BINARY_P1`).
//!
//! It does NOT contact the printer: macOS runs CUPS filters in a sandbox that
//! forbids network access (connect fails with EPERM). The tape check is left to
//! the printer, which refuses a job whose width flag does not match the loaded
//! cassette (Yuri's decision, 2026-10-06). `ptouch print --host` keeps the full
//! pre-flight check.
//!
//! `rastertoptouch --dry-run in.ras out.bin [options]` only converts a raster
//! file to a job file.

use ptouch_cups::{
    AUTO_MARGIN_DOTS, Label, Options, Result, build_job_offline, page_to_label, read_raster,
    resolve_options,
};
use std::{
    env,
    fs::File,
    io::{self, Read, Write},
    process::ExitCode,
};

fn load_label(input: impl Read, options: &Options) -> Result<Label> {
    let mut pages = read_raster(input)?;
    match pages.len() {
        0 => Err("the document has no pages".into()),
        1 => {
            let mut label = page_to_label(&pages.remove(0))?;
            if options.auto_length {
                let margin = AUTO_MARGIN_DOTS * label.scale();
                label.trim_to_content(margin, margin)?;
            } else {
                // The printer adds ~1 mm feed margin at each end; take it out of the
                // page so the label is as long as the size the user chose.
                label.compensate_feed_margin()?;
            }
            if options.flip() {
                label.mirror();
            }
            Ok(label)
        }
        n => Err(format!(
            "the job has {n} labels (pages or copies); print one label at a time"
        )),
    }
}

/// Options for this job: its own options over the queue defaults. CUPS gives the
/// filter the queue's PPD in `$PPD`; its `*Default...` lines are the driver
/// settings the settings app writes. If the file cannot be read (it always
/// should be), say so and fall back to the built-in defaults.
fn options_for(job_options: &str) -> Options {
    let ppd = env::var("PPD").ok().and_then(|path| {
        std::fs::read_to_string(&path)
            .map_err(|e| eprintln!("INFO: cannot read the queue settings in {path}: {e}"))
            .ok()
    });
    resolve_options(ppd.as_deref(), job_options)
}

/// Chain printing has no verified byte sequence yet; refuse instead of guessing,
/// since a wrong guess can leave the printer waiting for a page that never comes.
fn refuse_chain(options: &Options) -> Result<()> {
    if options.chain {
        return Err(
            "Chain printing is not available yet: it still has to be captured from \
                    P-touch Editor. Untick Chain printing."
                .into(),
        );
    }
    Ok(())
}

fn dry_run(args: &[String]) -> Result<()> {
    let [input, output, rest @ ..] = args else {
        return Err("usage: rastertoptouch --dry-run in.ras out.bin [options]".into());
    };
    let options = options_for(&rest.join(" "));
    refuse_chain(&options)?;
    let label = load_label(
        File::open(input).map_err(|e| format!("{input}: {e}"))?,
        &options,
    )?;
    let job = build_job_offline(&label, &options, Some(1))?;
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
    let mut options: Options = options_for(&args[4]);
    // CUPS passes the copy count as the 4th argument, not in the option string.
    options.copies = options.copies.max(args[3].parse().unwrap_or(1));
    refuse_chain(&options)?;
    if options.copies > 1 {
        return Err("print one copy at a time; multiple copies are not supported yet".into());
    }
    // The job tag carries a number like P-touch Editor's; any value 1..=255 works.
    let job_number = u8::try_from(args[0].parse::<u32>().unwrap_or(1) % 255 + 1).unwrap_or(1);

    let label = match args.get(5) {
        Some(path) => load_label(
            File::open(path).map_err(|e| format!("{path}: {e}"))?,
            &options,
        )?,
        None => load_label(io::stdin().lock(), &options)?,
    };
    let job = build_job_offline(&label, &options, Some(job_number))?;
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
