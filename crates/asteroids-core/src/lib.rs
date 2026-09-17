//! Platform-agnostic Asteroids game state, physics, and rendering.
//!
//! Nothing here knows about UEFI, a desktop window, or any other output
//! device -- [`Game::update`] is pure math, and [`Game::draw`] is generic
//! over any [`embedded_graphics::draw_target::DrawTarget`], so the same
//! logic runs unmodified under a UEFI GOP framebuffer, a desktop window, or
//! any other `embedded-graphics` backend.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;

use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::mono_font::ascii::FONT_10X20;
use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Circle, Polyline, PrimitiveStyle, Triangle};
use embedded_graphics::text::Text;

const ROT_SPEED: f32 = 3.5; // radians/sec
const THRUST_ACCEL: f32 = 220.0; // px/sec^2
const DRAG: f32 = 0.35; // fraction of velocity lost per second
const SHIP_RADIUS: f32 = 12.0;
const BULLET_SPEED: f32 = 420.0; // px/sec
const BULLET_TTL: f32 = 0.9; // seconds
const FIRE_COOLDOWN: f32 = 0.22; // seconds

/// A tiny xorshift PRNG -- good enough for scattering asteroid shapes and
/// spawn points, and avoids pulling in a `rand` dependency for it.
struct Rng(u32);

impl Rng {
    fn new(seed: u32) -> Self {
        Self(seed | 1)
    }

    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// Uniform float in `[lo, hi)`.
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        let frac = (self.next_u32() % 100_000) as f32 / 100_000.0;
        lo + frac * (hi - lo)
    }
}

#[derive(Clone, Copy, Default)]
struct Vec2 {
    x: f32,
    y: f32,
}

impl Vec2 {
    const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }

    fn scale(self, s: f32) -> Vec2 {
        Vec2::new(self.x * s, self.y * s)
    }

    fn dist(self, o: Vec2) -> f32 {
        libm::hypotf(self.x - o.x, self.y - o.y)
    }

    fn wrap(self, width: f32, height: f32) -> Vec2 {
        let mut v = self;
        if v.x < 0.0 {
            v.x += width;
        } else if v.x >= width {
            v.x -= width;
        }
        if v.y < 0.0 {
            v.y += height;
        } else if v.y >= height {
            v.y -= height;
        }
        v
    }

    fn point(self) -> Point {
        Point::new(self.x as i32, self.y as i32)
    }
}

#[derive(Clone, Copy)]
pub enum AsteroidSize {
    Large,
    Medium,
    Small,
}

impl AsteroidSize {
    fn radius(self) -> f32 {
        match self {
            AsteroidSize::Large => 42.0,
            AsteroidSize::Medium => 24.0,
            AsteroidSize::Small => 12.0,
        }
    }

    fn score(self) -> u32 {
        match self {
            AsteroidSize::Large => 20,
            AsteroidSize::Medium => 50,
            AsteroidSize::Small => 100,
        }
    }

    fn smaller(self) -> Option<AsteroidSize> {
        match self {
            AsteroidSize::Large => Some(AsteroidSize::Medium),
            AsteroidSize::Medium => Some(AsteroidSize::Small),
            AsteroidSize::Small => None,
        }
    }
}

struct Asteroid {
    pos: Vec2,
    vel: Vec2,
    size: AsteroidSize,
    /// Jagged silhouette: per-vertex radius multipliers, giving each rock a
    /// distinct, non-circular outline like the real thing.
    jitter: [f32; 10],
    spin: f32,
    rotation: f32,
}

impl Asteroid {
    fn spawn(rng: &mut Rng, pos: Vec2, size: AsteroidSize) -> Self {
        let speed = match size {
            AsteroidSize::Large => rng.range(15.0, 45.0),
            AsteroidSize::Medium => rng.range(30.0, 70.0),
            AsteroidSize::Small => rng.range(50.0, 110.0),
        };
        let angle = rng.range(0.0, core::f32::consts::TAU);
        let mut jitter = [1.0f32; 10];
        for j in jitter.iter_mut() {
            *j = rng.range(0.7, 1.15);
        }
        Self {
            pos,
            vel: Vec2::new(libm::cosf(angle), libm::sinf(angle)).scale(speed),
            size,
            jitter,
            spin: rng.range(-1.0, 1.0),
            rotation: rng.range(0.0, core::f32::consts::TAU),
        }
    }

    fn radius(&self) -> f32 {
        self.size.radius()
    }

    fn outline(&self) -> Vec<Point> {
        let n = self.jitter.len();
        let mut pts = Vec::with_capacity(n + 1);
        for (i, &j) in self.jitter.iter().enumerate() {
            let a = self.rotation + core::f32::consts::TAU * (i as f32) / (n as f32);
            let r = self.radius() * j;
            pts.push(self.pos.add(Vec2::new(libm::cosf(a), libm::sinf(a)).scale(r)).point());
        }
        pts.push(pts[0]);
        pts
    }
}

struct Bullet {
    pos: Vec2,
    vel: Vec2,
    ttl: f32,
}

pub struct Input {
    pub left: bool,
    pub right: bool,
    pub thrust: bool,
    pub fire: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum State {
    Playing,
    GameOver,
}

pub struct Game {
    width: f32,
    height: f32,
    rng: Rng,

    ship_pos: Vec2,
    ship_vel: Vec2,
    ship_angle: f32,
    fire_cooldown: f32,

    bullets: Vec<Bullet>,
    asteroids: Vec<Asteroid>,

    pub score: u32,
    pub state: State,
}

impl Game {
    pub fn new(width: u32, height: u32, seed: u32) -> Self {
        let (width, height) = (width as f32, height as f32);
        let mut rng = Rng::new(seed);

        let center = Vec2::new(width / 2.0, height / 2.0);
        let mut asteroids = Vec::new();
        for _ in 0..5 {
            // Spawn away from the ship's starting position so it doesn't
            // start the game already surrounded.
            let angle = rng.range(0.0, core::f32::consts::TAU);
            let dist = rng.range(150.0, width.min(height) / 2.0);
            let pos = center
                .add(Vec2::new(libm::cosf(angle), libm::sinf(angle)).scale(dist))
                .wrap(width, height);
            asteroids.push(Asteroid::spawn(&mut rng, pos, AsteroidSize::Large));
        }

        Self {
            width,
            height,
            rng,
            ship_pos: center,
            ship_vel: Vec2::default(),
            ship_angle: 0.0,
            fire_cooldown: 0.0,
            bullets: Vec::new(),
            asteroids,
            score: 0,
            state: State::Playing,
        }
    }

    pub fn update(&mut self, dt: f32, input: &Input) {
        if matches!(self.state, State::GameOver) {
            return;
        }

        if input.left {
            self.ship_angle -= ROT_SPEED * dt;
        }
        if input.right {
            self.ship_angle += ROT_SPEED * dt;
        }
        let facing = Vec2::new(libm::sinf(self.ship_angle), -libm::cosf(self.ship_angle));
        if input.thrust {
            self.ship_vel = self.ship_vel.add(facing.scale(THRUST_ACCEL * dt));
        }
        self.ship_vel = self.ship_vel.scale(1.0 - DRAG * dt);
        self.ship_pos = self.ship_pos.add(self.ship_vel.scale(dt)).wrap(self.width, self.height);

        self.fire_cooldown = (self.fire_cooldown - dt).max(0.0);
        if input.fire && self.fire_cooldown == 0.0 {
            self.fire_cooldown = FIRE_COOLDOWN;
            self.bullets.push(Bullet {
                pos: self.ship_pos.add(facing.scale(SHIP_RADIUS)),
                vel: self.ship_vel.add(facing.scale(BULLET_SPEED)),
                ttl: BULLET_TTL,
            });
        }

        for b in &mut self.bullets {
            b.pos = b.pos.add(b.vel.scale(dt)).wrap(self.width, self.height);
            b.ttl -= dt;
        }
        self.bullets.retain(|b| b.ttl > 0.0);

        for a in &mut self.asteroids {
            a.pos = a.pos.add(a.vel.scale(dt)).wrap(self.width, self.height);
            a.rotation += a.spin * dt;
        }

        let mut spawned = Vec::new();
        let mut hit_bullets = alloc::vec![false; self.bullets.len()];
        self.asteroids.retain_mut(|a| {
            for (bi, b) in self.bullets.iter().enumerate() {
                if hit_bullets[bi] {
                    continue;
                }
                if a.pos.dist(b.pos) < a.radius() {
                    hit_bullets[bi] = true;
                    self.score += a.size.score();
                    if let Some(smaller) = a.size.smaller() {
                        spawned.push(Asteroid::spawn(&mut self.rng, a.pos, smaller));
                        spawned.push(Asteroid::spawn(&mut self.rng, a.pos, smaller));
                    }
                    return false;
                }
            }
            true
        });
        let mut bi = 0;
        self.bullets.retain(|_| {
            let hit = hit_bullets[bi];
            bi += 1;
            !hit
        });
        self.asteroids.extend(spawned);

        if self
            .asteroids
            .iter()
            .any(|a| a.pos.dist(self.ship_pos) < a.radius() + SHIP_RADIUS)
        {
            self.state = State::GameOver;
        }
    }

    pub fn draw<D>(&self, display: &mut D)
    where
        D: DrawTarget<Color = Rgb888>,
    {
        let white = PrimitiveStyle::with_stroke(Rgb888::WHITE, 1);
        let yellow = PrimitiveStyle::with_stroke(Rgb888::YELLOW, 1);
        let text_style = MonoTextStyle::new(&FONT_10X20, Rgb888::WHITE);

        let _ = display.clear(Rgb888::BLACK);

        // Ship: a simple triangle pointing in `ship_angle`.
        let facing = Vec2::new(libm::sinf(self.ship_angle), -libm::cosf(self.ship_angle));
        let side = Vec2::new(facing.y, -facing.x);
        let nose = self.ship_pos.add(facing.scale(SHIP_RADIUS));
        let left = self.ship_pos.add(facing.scale(-SHIP_RADIUS * 0.7)).add(side.scale(-SHIP_RADIUS * 0.6));
        let right = self.ship_pos.add(facing.scale(-SHIP_RADIUS * 0.7)).add(side.scale(SHIP_RADIUS * 0.6));
        if matches!(self.state, State::Playing) {
            let _ = Triangle::new(nose.point(), left.point(), right.point())
                .into_styled(white)
                .draw(display);
        }

        for b in &self.bullets {
            let _ = Circle::with_center(b.pos.point(), 3)
                .into_styled(PrimitiveStyle::with_fill(Rgb888::WHITE))
                .draw(display);
        }

        for a in &self.asteroids {
            let outline = a.outline();
            let _ = Polyline::new(&outline).into_styled(yellow).draw(display);
        }

        let mut score_buf = itoa_buf();
        let score_str = write_score(&mut score_buf, self.score);
        let _ = Text::new(score_str, Point::new(10, 24), text_style).draw(display);

        if matches!(self.state, State::GameOver) {
            let _ = Text::new(
                "GAME OVER -- Enter to restart, Esc to quit",
                Point::new(10, (self.height as i32) / 2),
                text_style,
            )
            .draw(display);
        }
    }
}

/// A fixed-size stack buffer big enough for `"Score: "` plus any `u32` in
/// decimal, so we can format the score without needing `alloc::format!` on
/// the hot draw path.
fn itoa_buf() -> [u8; 24] {
    [0u8; 24]
}

fn write_score(buf: &mut [u8; 24], score: u32) -> &str {
    use core::fmt::Write;

    struct Cursor<'a> {
        buf: &'a mut [u8],
        len: usize,
    }
    impl core::fmt::Write for Cursor<'_> {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            let bytes = s.as_bytes();
            self.buf[self.len..self.len + bytes.len()].copy_from_slice(bytes);
            self.len += bytes.len();
            Ok(())
        }
    }

    let mut cursor = Cursor { buf, len: 0 };
    let _ = write!(cursor, "Score: {score}");
    let len = cursor.len;
    core::str::from_utf8(&buf[..len]).unwrap_or("")
}
