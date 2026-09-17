//! A generic, boolean-only "arcade" input abstraction -- left/right/thrust/
//! fire -- plus optional backends that produce it from real hardware.
//!
//! [`InputState`] and [`InputSource`] have no platform dependency at all;
//! they're plain enough to be no_std-safe unconditionally. The two backends
//! behind `desktop` and `uefi` feature flags are what pull in real
//! dependencies (`minifb`+`gilrs`, or `uefi`), and are mutually exclusive in
//! practice -- a binary picks the one matching its target and disables the
//! other via `default-features = false`. `desktop` is the default, since
//! most consumers of this crate as a plain dependency (as opposed to a
//! `#![no_std]` firmware binary) are ordinary desktop programs.
#![cfg_attr(not(feature = "desktop"), no_std)]

#[cfg(feature = "desktop")]
mod desktop;
#[cfg(feature = "uefi")]
mod uefi_input;

#[cfg(feature = "desktop")]
pub use desktop::DesktopInput;
#[cfg(feature = "uefi")]
pub use uefi_input::{HOLD_GRACE_TICKS, UefiInput};

/// A snapshot of which of the four digital inputs are currently held.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InputState {
    pub left: bool,
    pub right: bool,
    pub thrust: bool,
    pub fire: bool,
}

/// Something that can be polled once per game tick for the current
/// [`InputState`].
pub trait InputSource {
    fn poll(&mut self) -> InputState;
}
