//! The ruefi demo application: a small interactive menu driven by a real
//! UEFI event loop (keyboard + periodic timer).

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::time::Duration;

use ratatui::Terminal;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use uefi::boot::{self, EventType, TimerTrigger, Tpl};
use uefi::prelude::*;
use uefi::proto::console::text::{Key, ScanCode};
use uefi::{Char16, Event, system};

use crate::uefi_backend::UefiBackend;

const MENU_ITEMS: [&str; 4] = ["Reset counter", "Say hello", "Do nothing", "Quit (Esc or 'q')"];
const SPINNER: [char; 4] = ['|', '/', '-', '\\'];
const TICK_INTERVAL: Duration = Duration::from_millis(250);

/// A tick of the event loop: either a key was pressed, or the periodic
/// timer fired with no key available.
enum Tick {
    Key(Key),
    Timer,
}

/// What the app should do after handling one tick.
enum Action {
    Continue,
    Quit,
}

/// The application's UI + event-loop state.
pub struct App {
    terminal: Terminal<UefiBackend>,
    key_event: Event,
    timer_event: Event,
    list_state: ListState,
    counter: i32,
    message: String,
    spinner_frame: usize,
}

impl App {
    /// Sets up the terminal, backend and firmware events. Does not draw
    /// anything yet -- that happens on the first iteration of `run`.
    pub fn new() -> Self {
        let backend = UefiBackend::new();
        let terminal = Terminal::new(backend).unwrap();

        let key_event = system::with_stdin(|stdin| stdin.wait_for_key_event()).unwrap();

        // SAFETY: no notify function, so there's nothing that needs to be
        // careful about running after boot services exit.
        let timer_event =
            unsafe { boot::create_event(EventType::TIMER, Tpl::APPLICATION, None, None) }
                .unwrap();
        boot::set_timer(&timer_event, TimerTrigger::Periodic(TICK_INTERVAL)).unwrap();

        let mut list_state = ListState::default();
        list_state.select(Some(0));

        Self {
            terminal,
            key_event,
            timer_event,
            list_state,
            counter: 0,
            message: "Use \u{2191}/\u{2193} to move, Enter to select".to_string(),
            spinner_frame: 0,
        }
    }

    /// Runs the app's event loop until the user quits, then returns to
    /// firmware with `Status::SUCCESS`.
    pub fn run(mut self) -> Status {
        loop {
            self.draw();

            match self.next_tick() {
                Tick::Timer => self.on_timer(),
                Tick::Key(key) => match self.on_key(key) {
                    Action::Continue => {}
                    Action::Quit => break,
                },
            }
        }

        boot::set_timer(&self.timer_event, TimerTrigger::Cancel).unwrap();
        Status::SUCCESS
    }

    fn draw(&mut self) {
        let list_state = &mut self.list_state;
        let counter = self.counter;
        let message = &self.message;
        let spinner = SPINNER[self.spinner_frame % SPINNER.len()];

        self.terminal
            .draw(|frame| {
                let area = frame.area();

                let list_items: Vec<ListItem> =
                    MENU_ITEMS.iter().map(|i| ListItem::new(*i)).collect();

                let list = List::new(list_items)
                    .block(
                        Block::default()
                            .title(" ruefi menu ")
                            .borders(Borders::ALL)
                            .style(Style::default().fg(Color::Cyan)),
                    )
                    .highlight_style(
                        Style::default()
                            .bg(Color::Blue)
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol("> ");

                let bottom_h = 4u16.min(area.height);
                let list_area = Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: area.height.saturating_sub(bottom_h),
                };
                let status_area = Rect {
                    x: area.x,
                    y: area.y + list_area.height,
                    width: area.width,
                    height: bottom_h,
                };

                frame.render_stateful_widget(list, list_area, list_state);

                let status = Paragraph::new(format!("counter: {counter}   {spinner}\n{message}"))
                    .block(
                        Block::default()
                            .title(" status ")
                            .borders(Borders::ALL)
                            .style(Style::default().fg(Color::Yellow)),
                    );
                frame.render_widget(status, status_area);
            })
            .unwrap();
    }

    /// Waits for either a keypress or the next timer tick, whichever comes
    /// first, and reports which one it was. This is the standard UEFI
    /// pattern for a non-blocking-feeling event loop: everything is still
    /// driven by `boot::wait_for_event`, but we hand it more than one event
    /// to wait on.
    fn next_tick(&self) -> Tick {
        // `wait_for_event` consumes the events it's given, so pass fresh
        // clones of the handles each time (cloning an `Event` clones the
        // handle, not the underlying firmware object).
        let mut events = [
            // SAFETY: both events remain valid for the lifetime of `self`.
            unsafe { self.key_event.unsafe_clone() },
            unsafe { self.timer_event.unsafe_clone() },
        ];

        let index = boot::wait_for_event(&mut events).unwrap();

        if index == 0 {
            // The key event only tells us a keystroke is *available* --
            // still have to actually read it.
            if let Some(key) = system::with_stdin(|stdin| stdin.read_key()).unwrap() {
                return Tick::Key(key);
            }
        }
        Tick::Timer
    }

    /// The timer is what makes this a *real* event loop rather than a
    /// blocking read: the spinner advances, and anything else time-based (a
    /// clock, an animation, a poll of some other device) would go here too,
    /// alongside the key handling below rather than instead of it.
    fn on_timer(&mut self) {
        self.spinner_frame = self.spinner_frame.wrapping_add(1);
    }

    fn on_key(&mut self, key: Key) -> Action {
        match key {
            Key::Special(ScanCode::UP) => {
                let i = self.list_state.selected().unwrap_or(0);
                self.list_state.select(Some(i.saturating_sub(1)));
            }
            Key::Special(ScanCode::DOWN) => {
                let i = self.list_state.selected().unwrap_or(0);
                self.list_state
                    .select(Some((i + 1).min(MENU_ITEMS.len() - 1)));
            }
            Key::Special(ScanCode::ESCAPE) => return Action::Quit,
            Key::Printable(c) if c == Char16::try_from('q').unwrap() => return Action::Quit,
            Key::Printable(c) if c == Char16::try_from('\r').unwrap() => {
                match self.list_state.selected() {
                    Some(0) => {
                        self.counter = 0;
                        self.message = "Counter reset".to_string();
                    }
                    Some(1) => {
                        self.counter += 1;
                        self.message = "Hello from ruefi!".to_string();
                    }
                    Some(3) => return Action::Quit,
                    _ => {
                        self.message = "Nothing happened".to_string();
                    }
                }
            }
            _ => {}
        }
        Action::Continue
    }
}
