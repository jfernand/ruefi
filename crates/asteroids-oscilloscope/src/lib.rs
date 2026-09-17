//! SCAFFOLD -- not yet implemented, and needs real design work before it
//! can be, not just a naive `DrawTarget` impl.
//!
//! An oscilloscope in XY mode has no framebuffer or pixel grid at all: the
//! beam traces whatever (X, Y) voltage pair sequence is fed to its two
//! input channels (typically driven by a two-channel DAC, or a stereo
//! audio interface with X on one channel and Y on the other), and
//! persistence of vision does the rest. This breaks the usual
//! `embedded_graphics::draw_target::DrawTarget` model in a few specific
//! ways:
//!
//! - `clear()`/`fill_solid()` have no sensible meaning -- there is no
//!   "blank the screen" operation, and no filled *areas* at all. A scope
//!   only ever draws lines/points along the beam's path.
//! - `draw_iter` receiving a flood of arbitrary, unordered `Pixel`s (as a
//!   filled `Circle`'s disc rasterization or `Text`'s glyph rendering
//!   produce) is the wrong shape of input entirely -- it would need to be
//!   converted into an *ordered* path the beam can trace continuously,
//!   which per-pixel iteration order does not guarantee.
//! - Of [`asteroids_core::Game::draw`]'s primitives, the *stroked* shapes
//!   (the ship's `Triangle`, each asteroid's `Polyline` outline) are a
//!   natural fit -- they're already vector outlines, and just need their
//!   vertices linearized into an (X, Y) voltage sequence tracing outline
//!   edges as continuous line segments.
//! - The *filled* shapes are not a natural fit: filled bullet `Circle`s
//!   would need to become unfilled rings (or single points, sacrificing
//!   bullet size) to avoid an expensive scan-fill of voltage steps, and
//!   `Text`'s bitmap glyph rendering (score display, "GAME OVER" message)
//!   has no good XY-scope equivalent without a font specifically designed
//!   as stroke outlines (a vector/stroke font, not `FONT_10X20`'s bitmap
//!   glyphs) -- score/game-over text may need to be dropped or replaced
//!   with a simple stroke-segment digit font.
//!
//! Implementing this backend for real likely means *not* implementing
//! `DrawTarget` at all, and instead walking `Game`'s renderable state
//! directly to emit an ordered vector path -- which needs the same kind of
//! public read-only state access flagged in the `asteroids-tui` scaffold's
//! notes, since today that state is private and visible only inside
//! `draw`.
#![no_std]

use asteroids_core::Game;

/// Not a real API yet -- exists so this crate has a genuine (if unused)
/// dependency edge on `asteroids-core` for `cargo build` to exercise, per
/// the design notes above.
pub fn placeholder(_game: &Game) {}
