#![no_main]
#![no_std]

extern crate alloc;

use core::time::Duration;

use asteroids_core::{Game, State};
use game_input::{InputSource, UefiInput};
use gop_display::GopDisplay;
use uefi::Char16;
use uefi::boot::{self, EventType, ScopedProtocol, TimerTrigger, Tpl};
use uefi::prelude::*;
use uefi::proto::console::gop::GraphicsOutput;
use uefi::proto::console::text::{Key, ScanCode};
use uefi::system;

const TICK_INTERVAL: Duration = Duration::from_millis(16);

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

    let mut input_source = UefiInput::new();
    let mut restart_pressed = false;
    let mut quit = false;
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
            // Quit/restart are game-loop concerns, not input-state concerns,
            // so they're matched here directly rather than through
            // `UefiInput` -- everything else goes to it for hold-tracking.
            match key {
                Key::Special(ScanCode::ESCAPE) => quit = true,
                Key::Printable(c) if c == Char16::try_from('q').unwrap() => quit = true,
                Key::Printable(c) if c == Char16::try_from('\r').unwrap() => {
                    restart_pressed = true;
                }
                other => input_source.key_event(other),
            }
        } else {
            clock += dt;
            let input = input_source.poll();
            game.update(dt, &input);

            if restart_pressed {
                if matches!(game.state, State::GameOver) {
                    game = Game::new(width as u32, height as u32, clock.to_bits());
                }
                restart_pressed = false;
            }
        }

        if quit {
            break;
        }
    }

    boot::set_timer(&timer_event, TimerTrigger::Cancel).unwrap();
    Status::SUCCESS
}
