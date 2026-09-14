#![no_main]
#![no_std]

extern crate alloc;

mod app;
mod uefi_backend;

use app::App;
use uefi::prelude::*;

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();

    App::new().run()
}
