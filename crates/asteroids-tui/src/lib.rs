//! SCAFFOLD -- not yet implemented.
//!
//! `ratatui` isn't a dependency here yet, deliberately: this workspace's
//! ambient default build target is `x86_64-unknown-uefi` (no_std), and
//! ratatui's terminal backend needs a real OS/terminal (`std`) -- adding it
//! now would break `cargo build --workspace` the same way the desktop
//! crates need an explicit `--target` override to build at all. Add it,
//! and switch this crate to `std`, when actually implementing.
//!
//! [`asteroids_core::Game::draw`] is generic over
//! `embedded_graphics::draw_target::DrawTarget`, which every other backend
//! (UEFI GOP, minifb, and eventually HUB75) can implement directly.
//! ratatui's terminal canvas cannot: its
//! `ratatui::widgets::canvas::{Canvas, Context, Painter, Shape}` system is
//! a wholly separate drawing trait surface with no `embedded-graphics`
//! compatibility shim. Concretely:
//!
//! - `Shape::draw(&self, painter: &mut Painter)` is ratatui's equivalent of
//!   `Drawable::draw`, but `Painter` only exposes `paint(x, y, color)` (set
//!   one cell) and `get_point(x, y) -> Option<(usize, usize)>`
//!   (world-to-cell mapping) -- there is no `DrawTarget` impl to write,
//!   since ratatui doesn't know about `embedded-graphics` at all.
//! - ratatui ships `Line`, `FilledLine`, `Points`, `Circle`, and
//!   `Rectangle` shapes out of the box, but **no `Polyline` or
//!   `Triangle`** -- the asteroid outlines (`Polyline` today) and the ship
//!   (`Triangle` today) will need to be hand-rolled as a small `impl
//!   Shape` that draws each edge as a `ratatui::widgets::canvas::Line`
//!   segment.
//! - Text (score, "GAME OVER") isn't a `Canvas` concern in ratatui at all
//!   -- it would be a separate `Paragraph` widget overlaid near the
//!   `Canvas`, not drawn through the same trait.
//!
//! The bigger blocker: this backend can't call `Game::draw` at all, since
//! that method's generic bound is `DrawTarget`, not `Shape`/`Painter`. It
//! needs `Game`'s renderable state (ship position + angle, bullet
//! positions, per-asteroid outline points, score, game-over flag) exposed
//! as public read-only data or via a small backend-agnostic "scene"
//! struct/trait -- today all of those fields are private and visible only
//! inside `draw`. This is a real API design decision for `asteroids-core`,
//! deferred past this scaffold, not something to improvise here.
#![no_std]

use asteroids_core::Game;

/// Not a real API yet -- exists so this crate has a genuine (if unused)
/// dependency edge on `asteroids-core` for `cargo build` to exercise, per
/// the design notes above.
pub fn placeholder(_game: &Game) {}
