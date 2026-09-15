//! The ruefi application shell: owns the terminal, the UEFI event loop,
//! and the tab bar. Everything screen-specific -- state, rendering, key
//! handling -- lives in [`crate::screens`].

use alloc::boxed::Box;
use alloc::format;
use alloc::vec::Vec;
use core::time::Duration;

use ratatui::Terminal;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Paragraph, Tabs};
use uefi::boot::{self, EventType, TimerTrigger, Tpl};
use uefi::prelude::*;
use uefi::proto::console::text::{Key, ScanCode};
use uefi::{Char16, Event, system};

use crate::screens::{self, Action, Screen};
use crate::uefi_backend::UefiBackend;

const TICK_INTERVAL: Duration = Duration::from_millis(250);
const SPINNER: [char; 4] = ['|', '/', '-', '\\'];

/// A tick of the event loop: either a key was pressed, or the periodic
/// timer fired with no key available.
enum Tick {
    Key(Key),
    Timer,
}

pub struct App {
    terminal: Terminal<UefiBackend>,
    key_event: Event,
    timer_event: Event,
    spinner_frame: usize,
    tab: usize,
    screens: Vec<Box<dyn Screen>>,
}

impl App {
    /// Sets up the terminal, backend, firmware events, and every screen
    /// (which each gather their own platform data on construction). Does
    /// not draw anything yet -- that happens on the first iteration of
    /// `run`.
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

        let screens: Vec<Box<dyn Screen>> = alloc::vec![
            Box::new(screens::memory::MemoryScreen::new()),
            Box::new(screens::acpi::AcpiScreen::new()),
            Box::new(screens::pci::PciScreen::new()),
            Box::new(screens::disks::DisksScreen::new()),
            Box::new(screens::vars::VarsScreen::new()),
            Box::new(screens::hda::HdaScreen::new()),
        ];

        Self {
            terminal,
            key_event,
            timer_event,
            spinner_frame: 0,
            tab: 0,
            screens,
        }
    }

    /// Runs the app's event loop until the user quits, then returns to
    /// firmware with `Status::SUCCESS`.
    pub fn run(mut self) -> Status {
        loop {
            self.draw();

            match self.next_tick() {
                Tick::Timer => self.spinner_frame = self.spinner_frame.wrapping_add(1),
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
        let tab = self.tab;
        let spinner = SPINNER[self.spinner_frame % SPINNER.len()];
        let titles: Vec<&'static str> = self.screens.iter().map(|s| s.title()).collect();
        let screen = &mut self.screens[tab];

        self.terminal
            .draw(|frame| {
                let area = frame.area();
                let [tabs_area, content_area, footer_area] = Layout::vertical([
                    Constraint::Length(1),
                    Constraint::Min(0),
                    Constraint::Length(1),
                ])
                .areas(area);

                let tabs = Tabs::new(titles)
                    .select(tab)
                    .style(Style::default().fg(Color::Gray))
                    .highlight_style(
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    );
                frame.render_widget(tabs, tabs_area);

                screen.render(frame, content_area);

                let footer = Paragraph::new(format!(
                    " {spinner}  \u{2190}/\u{2192} tabs   \u{2191}/\u{2193} select   Enter act   Esc/q quit"
                ))
                .style(Style::default().fg(Color::DarkGray));
                frame.render_widget(footer, footer_area);
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

    /// Global key bindings -- tab switching and quit -- that apply no
    /// matter which screen is active. Anything else is handed to the
    /// active screen's own `on_key`.
    fn on_key(&mut self, key: Key) -> Action {
        match key {
            Key::Special(ScanCode::LEFT) => {
                self.tab = (self.tab + self.screens.len() - 1) % self.screens.len();
                Action::Continue
            }
            Key::Special(ScanCode::RIGHT) => {
                self.tab = (self.tab + 1) % self.screens.len();
                Action::Continue
            }
            Key::Special(ScanCode::ESCAPE) => Action::Quit,
            Key::Printable(c) if c == Char16::try_from('q').unwrap() => Action::Quit,
            other => self.screens[self.tab].on_key(other),
        }
    }
}
