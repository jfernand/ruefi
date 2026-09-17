//! Turns desktop gamepad (via `gilrs`) and keyboard (via the `minifb`
//! window's own key-state) into an [`asteroids_core::Input`] snapshot.

use asteroids_core::Input;
use gilrs::{Button, Gilrs};
use minifb::{Key, Window};

/// Polls both input sources and OR's them together -- not "gamepad
/// overrides keyboard" -- so a pad that's plugged in but idle never
/// shadows keyboard input, and either source alone is enough to play.
/// Only the first connected gamepad is consulted (no pad-selection UI for
/// a single-player game), and only digital buttons/d-pad, not analog
/// sticks, since `Input` itself is boolean-only.
pub fn poll(window: &Window, gilrs: &mut Gilrs) -> Input {
    // Drain pending events so gilrs's own per-gamepad button state is
    // current before we read it.
    while gilrs.next_event().is_some() {}

    let mut input = Input {
        left: false,
        right: false,
        thrust: false,
        fire: false,
    };

    input.left |= window.is_key_down(Key::Left);
    input.right |= window.is_key_down(Key::Right);
    input.thrust |= window.is_key_down(Key::Up);
    input.fire |= window.is_key_down(Key::Space);

    if let Some((_id, pad)) = gilrs.gamepads().next() {
        input.left |= pad.is_pressed(Button::DPadLeft);
        input.right |= pad.is_pressed(Button::DPadRight);
        input.thrust |= pad.is_pressed(Button::DPadUp) || pad.is_pressed(Button::South);
        input.fire |= pad.is_pressed(Button::East) || pad.is_pressed(Button::RightTrigger2);
    }

    input
}
