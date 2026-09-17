mod minifb_display;

use std::time::Duration;

use asteroids_core::{Game, State};
use gilrs::Gilrs;
use minifb::{Key, Window, WindowOptions};
use minifb_display::MinifbDisplay;

const WIDTH: usize = 1024;
const HEIGHT: usize = 768;
const TICK_INTERVAL: Duration = Duration::from_millis(16);

fn main() {
    let mut window = Window::new("Asteroids", WIDTH, HEIGHT, WindowOptions::default())
        .expect("failed to open a window");
    // minifb has no wait-for-event model like UEFI's `wait_for_event`; this
    // paces the loop to a fixed timestep instead, playing the same role
    // `TICK_INTERVAL`'s periodic timer event does on the UEFI backend.
    window.limit_update_rate(Some(TICK_INTERVAL));

    let mut gilrs = Gilrs::new().expect("failed to initialize gamepad input");
    let dt = TICK_INTERVAL.as_secs_f32();
    let mut clock = 0.0f32;

    let mut game = Game::new(WIDTH as u32, HEIGHT as u32, 0x2545F4);
    let mut display = MinifbDisplay::new(WIDTH, HEIGHT);

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let input = asteroids_input_desktop::poll(&window, &mut gilrs);
        clock += dt;
        game.update(dt, &input);
        game.draw(&mut display);
        window
            .update_with_buffer(display.buffer(), WIDTH, HEIGHT)
            .expect("failed to present the frame");

        if window.is_key_down(Key::Enter) && matches!(game.state, State::GameOver) {
            game = Game::new(WIDTH as u32, HEIGHT as u32, clock.to_bits());
        }
    }
}
