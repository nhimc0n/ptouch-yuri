// SPDX-License-Identifier: GPL-3.0-or-later

//! Driver settings for the PT-E850TKW print queue.
//!
//! The settings live in the CUPS queue itself: `lpadmin -p <queue> -o Key=Value`
//! rewrites the `*Default<Key>` line of the queue's PPD, and the `rastertoptouch`
//! filter reads those defaults for every job. Nothing has to keep running.
//! This crate reads and writes them and reports the printer's state; the
//! settings app is a window on top of it.

pub mod queue;
pub mod status;
