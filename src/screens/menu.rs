//! The landing tab: a simple menu demonstrating up/down + enter handling.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use uefi::Char16;
use uefi::proto::console::text::{Key, ScanCode};

use super::{Action, Screen};

const ITEMS: [&str; 3] = ["Reset counter", "Say hello", "Do nothing"];

pub struct MenuScreen {
    list_state: ListState,
    counter: i32,
    message: String,
}

impl MenuScreen {
    pub fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            list_state,
            counter: 0,
            message: "Use \u{2191}/\u{2193} to move, Enter to select".to_string(),
        }
    }
}

impl Screen for MenuScreen {
    fn title(&self) -> &'static str {
        "Menu"
    }

    fn on_key(&mut self, key: Key) -> Action {
        match key {
            Key::Special(ScanCode::UP) => {
                let i = self.list_state.selected().unwrap_or(0);
                self.list_state.select(Some(i.saturating_sub(1)));
            }
            Key::Special(ScanCode::DOWN) => {
                let i = self.list_state.selected().unwrap_or(0);
                self.list_state.select(Some((i + 1).min(ITEMS.len() - 1)));
            }
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
                    _ => {
                        self.message = "Nothing happened".to_string();
                    }
                }
            }
            _ => {}
        }
        Action::Continue
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        let list_items: Vec<ListItem> = ITEMS.iter().map(|i| ListItem::new(*i)).collect();

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

        let [list_area, status_area] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(4)]).areas(area);

        frame.render_stateful_widget(list, list_area, &mut self.list_state);

        let status = Paragraph::new(alloc::format!("counter: {}\n{}", self.counter, self.message))
            .block(
                Block::default()
                    .title(" status ")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Yellow)),
            );
        frame.render_widget(status, status_area);
    }
}
