// SPDX-License-Identifier: GPL-3.0-or-later

//! Driver settings app for the Brother PT-E850TKW.
//!
//! A thin window over `ptouch-settings`: it reads and writes the print queue's
//! driver defaults and shows the printer's state. It builds no printer bytes.
//! Every command waits on a subprocess or the network, so each one is
//! `command(async)`; a plain command would run on the main thread and freeze
//! the window.

use ptouch_settings::{
    queue::{self, DriverSettings},
    status::{self, PrinterInfo},
};

#[tauri::command(async)]
fn get_settings() -> Result<DriverSettings, String> {
    queue::read_settings(queue::QUEUE)
}

#[tauri::command(async)]
fn get_printer() -> PrinterInfo {
    status::read_printer(queue::QUEUE)
}

#[tauri::command(async)]
fn apply_settings(settings: DriverSettings) -> Result<(), String> {
    queue::apply_settings(queue::QUEUE, &settings)
}

#[tauri::command(async)]
fn set_printer_host(host: String) -> Result<(), String> {
    queue::set_printer_host(queue::QUEUE, &host)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_settings,
            get_printer,
            apply_settings,
            set_printer_host
        ])
        .run(tauri::generate_context!())
        .expect("error while running the settings app");
}
