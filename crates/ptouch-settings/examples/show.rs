// SPDX-License-Identifier: GPL-3.0-or-later

//! Print the driver settings stored in the queue and the printer's live state.
//! Read-only: `cargo run -p ptouch-settings --example show`.

use ptouch_settings::{queue, status};

fn main() {
    match queue::read_settings(queue::QUEUE) {
        Ok(s) => println!(
            "driver settings: half_cut={} full_cut={} chain={} mirror={} quality={} page_size={} ({} sizes)",
            s.half_cut,
            s.full_cut,
            s.chain,
            s.mirror,
            s.quality.keyword(),
            s.page_size,
            s.page_sizes.len()
        ),
        Err(e) => println!("driver settings: {e}"),
    }
    match queue::read_printer_host(queue::QUEUE) {
        Ok(host) => {
            println!("printer address: {host}");
            match status::read_status(&host) {
                Ok(st) => println!(
                    "printer: {} state={} ready={} tape={} mm (supported={})",
                    st.model, st.state, st.ready, st.tape_mm, st.tape_supported
                ),
                Err(e) => println!("printer: {e}"),
            }
        }
        Err(e) => println!("printer address: {e}"),
    }
}
