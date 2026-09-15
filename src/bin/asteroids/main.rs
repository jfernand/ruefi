#![no_main]
#![no_std]

extern crate alloc;

mod game;
mod gop_display;

use core::time::Duration;

use game::{Game, Input, State};
use gop_display::GopDisplay;
use uefi::Char16;
use uefi::boot::{self, EventType, ScopedProtocol, TimerTrigger, Tpl};
use uefi::prelude::*;
use uefi::proto::console::gop::GraphicsOutput;
use uefi::proto::console::text::{Key, ScanCode};
use uefi::system;

/// How long a held direction/thrust key is considered still "held" after
/// its most recent keystroke event, in seconds. UEFI's text input only
/// reports discrete keypress events (no key-up), but a held key on a real
/// (or emulated) keyboard produces repeated events via typematic repeat --
/// so we treat "still getting events" as "still held", and let the state
/// decay shortly after the repeats stop. This is the standard trick for
/// approximating held-key state from a keypress-only input source.
const HOLD_GRACE: f32 = 0.18;
const TICK_INTERVAL: Duration = Duration::from_millis(16);

struct HeldKeys {
    left_until: f32,
    right_until: f32,
    thrust_until: f32,
    fire_pressed: bool,
    restart_pressed: bool,
    quit: bool,
}

impl HeldKeys {
    fn new() -> Self {
        Self {
            left_until: 0.0,
            right_until: 0.0,
            thrust_until: 0.0,
            fire_pressed: false,
            restart_pressed: false,
            quit: false,
        }
    }

    fn apply(&mut self, key: Key, now: f32) {
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

    fn input(&self, now: f32) -> Input {
        Input {
            left: now < self.left_until,
            right: now < self.right_until,
            thrust: now < self.thrust_until,
            fire: self.fire_pressed,
        }
    }
}

fn open_gop() -> Option<ScopedProtocol<GraphicsOutput>> {
    let handle = boot::get_handle_for_protocol::<GraphicsOutput>().ok()?;
    boot::open_protocol_exclusive::<GraphicsOutput>(handle).ok()
}

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();

    let Some(mut gop) = open_gop() else {
        uefi::println!("asteroids: no Graphics Output Protocol found");
        boot::stall(Duration::from_secs(3));
        return Status::UNSUPPORTED;
    };

    let (width, height) = gop.current_mode_info().resolution();

    let key_event = system::with_stdin(|stdin| stdin.wait_for_key_event()).unwrap();
    // SAFETY: no notify function, so there's nothing to worry about across
    // boot service transitions.
    let timer_event =
        unsafe { boot::create_event(EventType::TIMER, Tpl::APPLICATION, None, None) }.unwrap();
    boot::set_timer(&timer_event, TimerTrigger::Periodic(TICK_INTERVAL)).unwrap();

    let mut held = HeldKeys::new();
    let mut clock = 0.0f32;
    let dt = TICK_INTERVAL.as_secs_f32();

    let mut game = Game::new(width as u32, height as u32, 0x2545F4);
    let mut display = GopDisplay::new(&mut gop);

    loop {
        game.draw(&mut display);
        display.present();

        let mut events = [
            // SAFETY: both events remain valid for the lifetime of this loop.
            unsafe { key_event.unsafe_clone() },
            unsafe { timer_event.unsafe_clone() },
        ];
        let index = boot::wait_for_event(&mut events).unwrap();

        if index == 0
            && let Some(key) = system::with_stdin(|stdin| stdin.read_key()).unwrap()
        {
            held.apply(key, clock);
        } else {
            clock += dt;
            let input = held.input(clock);
            game.update(dt, &input);
            held.fire_pressed = false;

            if held.restart_pressed {
                if matches!(game.state, State::GameOver) {
                    game = Game::new(width as u32, height as u32, clock.to_bits());
                }
                held.restart_pressed = false;
            }
        }

        if held.quit {
            break;
        }
    }

    boot::set_timer(&timer_event, TimerTrigger::Cancel).unwrap();
    Status::SUCCESS
}
