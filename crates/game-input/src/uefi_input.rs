//! UEFI input: turns discrete keypress events (UEFI's text input protocol
//! has no key-up) into held-key state via the standard "still getting
//! repeat events == still held" trick.

use crate::{InputSource, InputState};
use uefi::Char16;
use uefi::proto::console::text::{Key, ScanCode};

/// How many [`UefiInput::poll`] calls a held direction/thrust key is
/// considered still "held" after its most recent keystroke event. A held
/// key on a real (or emulated) keyboard produces repeated events via
/// typematic repeat, so "still getting events" approximates "still held",
/// decaying shortly after the repeats stop. Expressed in ticks rather than
/// wall-clock time since `poll` takes no timestamp -- callers are expected
/// to poll at a roughly constant rate (as any fixed-timestep game loop
/// does); at a 16ms tick this is ~180ms of grace.
pub const HOLD_GRACE_TICKS: u32 = 11;

/// Tracks held-key state from a stream of UEFI keypress events.
///
/// Only recognizes the four movement/fire keys (arrows + space); anything
/// else -- a quit or restart binding, say -- is a game-loop concern, not an
/// input-state concern, and is left for the caller to match on directly
/// from the same `Key` values passed to [`UefiInput::key_event`].
#[derive(Default)]
pub struct UefiInput {
    left_until: u32,
    right_until: u32,
    thrust_until: u32,
    fire_pressed: bool,
    ticks: u32,
}

impl UefiInput {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed every keypress event here, in event order.
    pub fn key_event(&mut self, key: Key) {
        match key {
            Key::Special(ScanCode::LEFT) => self.left_until = self.ticks + HOLD_GRACE_TICKS,
            Key::Special(ScanCode::RIGHT) => self.right_until = self.ticks + HOLD_GRACE_TICKS,
            Key::Special(ScanCode::UP) => self.thrust_until = self.ticks + HOLD_GRACE_TICKS,
            Key::Printable(c) if c == Char16::try_from(' ').unwrap() => self.fire_pressed = true,
            _ => {}
        }
    }
}

impl InputSource for UefiInput {
    /// Advances the internal tick counter and reads off the current held
    /// state. `fire` is edge-triggered: reading it here also clears it, so
    /// each fire keypress is consumed by exactly one poll.
    fn poll(&mut self) -> InputState {
        self.ticks += 1;
        InputState {
            left: self.ticks < self.left_until,
            right: self.ticks < self.right_until,
            thrust: self.ticks < self.thrust_until,
            fire: core::mem::take(&mut self.fire_pressed),
        }
    }
}
