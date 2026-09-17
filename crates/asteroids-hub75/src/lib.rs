//! SCAFFOLD -- not yet implemented.
//!
//! A HUB75 RGB LED matrix panel is driven by shifting row data into a chain
//! of shift registers over a handful of GPIO lines (R1/G1/B1/R2/G2/B2,
//! A/B/C/D row-address lines, CLK/LAT/OE), one panel row-pair at a time,
//! from some MCU or SBC host (e.g. an ESP32, RP2040, or a Raspberry Pi with
//! a dedicated HUB75 HAT) -- there is no memory-mapped framebuffer to write
//! into the way GOP or a desktop window buffer provides one.
//!
//! To implement this backend:
//!
//! 1. Pick a target MCU/host and its HUB75 driver crate (e.g. an
//!    `embedded-hal`-based shift-register driver, or a Raspberry Pi
//!    userspace PWM-timed driver) -- a real hardware decision deferred
//!    past this scaffold.
//! 2. Implement `embedded_graphics::draw_target::DrawTarget<Color =
//!    Rgb888>` over that driver's own framebuffer type, since most HUB75
//!    driver crates already provide one -- panel brightness is done via
//!    binary-coded modulation across multiple sub-frames, not a simple
//!    RGB888 write, so it isn't a plain pixel buffer the way GOP/minifb
//!    are.
//! 3. Typical panel resolutions (32x32, 64x32, 64x64) are far smaller than
//!    the `FONT_10X20` glyphs [`asteroids_core::Game::draw`] uses for the
//!    score/game-over text -- a much smaller embedded-graphics font will
//!    likely be needed, which means `Game`'s font choice may need to
//!    become a parameter rather than staying hardcoded, once this backend
//!    is actually built.
//! 4. `Game::new`'s asteroid/ship physics constants (radii, speeds) are
//!    tuned for typical desktop/UEFI display sizes (hundreds of pixels);
//!    at 32-64px they will likely need rescaling too.
#![no_std]

use asteroids_core::Game;

/// Not a real API yet -- exists so this crate has a genuine (if unused)
/// dependency edge on `asteroids-core` for `cargo build` to exercise, per
/// the design notes above.
pub fn placeholder(_game: &Game) {}
