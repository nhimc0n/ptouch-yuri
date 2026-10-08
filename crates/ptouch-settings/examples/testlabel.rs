// SPDX-License-Identifier: GPL-3.0-or-later

//! Writes the test label PDF for a tape width to a file. Sends nothing to the printer.
//! `cargo run -p ptouch-settings --example testlabel -- 36 out.pdf`

use ptouch_settings::testprint;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(tape), Some(path)) = (args.first().and_then(|t| t.parse().ok()), args.get(1)) else {
        eprintln!("usage: testlabel <tape mm> <out.pdf>");
        std::process::exit(2);
    };
    let Some(plan) = testprint::plan_for(tape) else {
        eprintln!("no test label for {tape} mm tape");
        std::process::exit(1);
    };
    std::fs::write(path, testprint::pdf(&plan)).expect("write the PDF");
    println!("{} {}", plan.page_size, path);
}
