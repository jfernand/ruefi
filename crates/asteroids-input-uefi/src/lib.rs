//! Turns UEFI keypress events into an [`asteroids_core::Input`] snapshot.
#![no_std]

use asteroids_core::Input;
use uefi::Char16;
use uefi::proto::console::text::{Key, ScanCode};

/// How long a held direction/thrust key is considered still "held" after
/// its most recent keystroke event, in seconds. UEFI's text input only
/// reports discrete keypress events (no key-up), but a held key on a real
/// (or emulated) keyboard produces repeated events via typematic repeat --
/// so we treat "still getting events" as "still held", and let the state
/// decay shortly after the repeats stop. This is the standard trick for
/// approximating held-key state from a keypress-only input source.
pub const HOLD_GRACE: f32 = 0.18;

pub struct HeldKeys {
    pub left_until: f32,
    pub right_until: f32,
    pub thrust_until: f32,
    pub fire_pressed: bool,
    pub restart_pressed: bool,
    pub quit: bool,
}

impl HeldKeys {
    pub fn new() -> Self {
        Self {
            left_until: 0.0,
            right_until: 0.0,
            thrust_until: 0.0,
            fire_pressed: false,
            restart_pressed: false,
            quit: false,
        }
    }

    pub fn apply(&mut self, key: Key, now: f32) {
        match key {
            Key::Special(ScanCode::LEFT) => self.left_until = now + HOLD_GRACE,
            Key::Special(ScanCode::RIGHT) => self.right_until = now + HOLD_GRACE,
            Key::Special(ScanCode::UP) => self.thrust_until = now + HOLD_GRACE,
            Key::Special(ScanCode::ESCAPE) => self.quit = true,
            Key::Printable(c) if c == Char16::try_from(' ').unwrap() => self.fire_pressed = true,
            Key::Printable(c) if c == Char16::try_from('\r').unwrap() => {
                self.restart_pressed = true;
            }
            Key::Printable(c) if c == Char16::try_from('q').unwrap() => self.quit = true,
            _ => {}
        }
    }

    pub fn input(&self, now: f32) -> Input {
        Input {
            left: now < self.left_until,
            right: now < self.right_until,
            thrust: now < self.thrust_until,
            fire: self.fire_pressed,
        }
    }
}

impl Default for HeldKeys {
    fn default() -> Self {
        Self::new()
    }
}
