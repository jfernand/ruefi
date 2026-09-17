//! Desktop input: a `minifb` window's keyboard state OR'd together with the
//! first connected `gilrs` gamepad.

use crate::{InputSource, InputState};
use gilrs::{Button, Gilrs};
use minifb::{Key, Window};

/// Borrows the window and gamepad handle for one poll -- cheap enough to
/// build fresh every frame from the main loop's own `Window`/`Gilrs`, so
/// this crate never has to own either.
pub struct DesktopInput<'a> {
    pub window: &'a Window,
    pub gilrs: &'a mut Gilrs,
}

impl InputSource for DesktopInput<'_> {
    /// OR's keyboard and gamepad together -- not "gamepad overrides
    /// keyboard" -- so a pad that's plugged in but idle never shadows
    /// keyboard input, and either source alone is enough to play. Only the
    /// first connected gamepad is consulted (no pad-selection UI), and only
    /// digital buttons/d-pad, not analog sticks, since `InputState` itself
    /// is boolean-only.
    fn poll(&mut self) -> InputState {
        // Drain pending events so gilrs's own per-gamepad button state is
        // current before we read it.
        while self.gilrs.next_event().is_some() {}

        let mut input = InputState::default();

        input.left |= self.window.is_key_down(Key::Left);
        input.right |= self.window.is_key_down(Key::Right);
        input.thrust |= self.window.is_key_down(Key::Up);
        input.fire |= self.window.is_key_down(Key::Space);

        if let Some((_id, pad)) = self.gilrs.gamepads().next() {
            input.left |= pad.is_pressed(Button::DPadLeft);
            input.right |= pad.is_pressed(Button::DPadRight);
            input.thrust |= pad.is_pressed(Button::DPadUp) || pad.is_pressed(Button::South);
            input.fire |= pad.is_pressed(Button::East) || pad.is_pressed(Button::RightTrigger2);
        }

        input
    }
}
