#![no_main]
#![no_std]

extern crate alloc;

mod app;
mod explore;
mod gop_backend;
mod screens;
mod widgets;

use core::time::Duration;

use app::App;
use uefi::boot;
use uefi::prelude::*;

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();

    let Some(gop) = gop_backend::open_gop() else {
        uefi::println!("ruefi: no Graphics Output Protocol found");
        boot::stall(Duration::from_secs(3));
        return Status::UNSUPPORTED;
    };

    App::new(gop_backend::GopBackend::new(gop)).run()
}
