//! Platform-agnostic core of an Intel HD Audio (HDA) controller/codec driver.
//!
//! This crate knows the register layout, verb encoding, and codec node model
//! defined by the Intel High Definition Audio Specification (see
//! `docs/hda-register-map.typ` in the repo root for a distilled reference).
//! It has no idea how to map a PCI BAR, allocate DMA memory, or wait for a
//! microsecond -- those are supplied by a [`Platform`] implementation, so the
//! same [`Controller`] logic runs unmodified under UEFI boot services, a
//! from-scratch kernel, or a hosted test harness.
#![no_std]

pub mod codec;
pub mod controller;
pub mod format;
pub mod platform;
pub mod regs;
pub mod target;
pub mod verbs;

pub use controller::{Controller, Error};
pub use format::PcmFormat;
pub use platform::{DmaBuffer, Platform};
pub use target::{
    StreamPlan, Target, configure_and_play_output, find_output_target, unmute_output_at_zero_db,
};
