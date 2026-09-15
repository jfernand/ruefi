//! Features tab: which UEFI/PI protocols this firmware actually implements,
//! checked against the static catalog in `explore::protocol_catalog`.

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
use uefi::Char16;
use uefi::proto::console::text::{Key, ScanCode};

use super::{Action, Screen, move_selection};
use crate::explore::protocols::{self, ProtocolStatus};

pub struct ProtocolsScreen {
    protocols: Vec<ProtocolStatus>,
    table_state: TableState,
    detail_open: bool,
}

impl ProtocolsScreen {
    pub fn new() -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        Self {
            protocols: protocols::scan(),
            table_state,
            detail_open: false,
        }
    }
}

impl Screen for ProtocolsScreen {
    fn title(&self) -> &'static str {
        "Features"
    }

    fn on_key(&mut self, key: Key) -> Action {
        match key {
            Key::Special(ScanCode::UP) => {
                let sel = move_selection(self.table_state.selected(), -1, self.protocols.len());
                self.table_state.select(sel);
                self.detail_open = false;
            }
            Key::Special(ScanCode::DOWN) => {
                let sel = move_selection(self.table_state.selected(), 1, self.protocols.len());
                self.table_state.select(sel);
                self.detail_open = false;
            }
            Key::Printable(c) if c == Char16::try_from('\r').unwrap() => {
                self.detail_open = !self.detail_open;
            }
            _ => {}
        }
        Action::Continue
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        let present_count = self.protocols.iter().filter(|p| p.present()).count();

        let header = Row::new(vec!["Category", "Protocol", "Status"])
            .style(Style::default().add_modifier(Modifier::BOLD));

        let rows = self.protocols.iter().map(|p| {
            let (status, color) = if p.present() {
                (format!("present ({} handle{})", p.handle_count, if p.handle_count == 1 { "" } else { "s" }), Color::Green)
            } else {
                (String::from("absent"), Color::DarkGray)
            };
            Row::new(vec![
                Cell::new(p.category),
                Cell::new(p.name),
                Cell::new(status).style(Style::default().fg(color)),
            ])
        });

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(38),
                Constraint::Percentage(32),
                Constraint::Percentage(30),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .title(format!(
                    " Features ({present_count}/{} present) -- Enter: details ",
                    self.protocols.len()
                ))
                .borders(Borders::ALL)
                .border_set(super::ASCII_BORDER)
                .style(Style::default().fg(Color::Cyan)),
        )
        .row_highlight_style(Style::default().bg(Color::Blue).fg(Color::White))
        .highlight_symbol("> ");

        frame.render_stateful_widget(table, area, &mut self.table_state);

        if self.detail_open
            && let Some(protocol) = self
                .table_state
                .selected()
                .and_then(|i| self.protocols.get(i))
        {
            let text = detail_text(protocol);
            super::render_dialog(frame, area, " protocol details -- Enter to close ", |frame, inner| {
                frame.render_widget(Paragraph::new(text).wrap(ratatui::widgets::Wrap { trim: false }), inner);
            });
        }
    }
}

fn detail_text(p: &ProtocolStatus) -> String {
    use core::fmt::Write;

    let mut text = String::new();
    let _ = writeln!(text, "Category:    {}", p.category);
    let _ = writeln!(text, "Protocol:    {}", p.name);
    let _ = writeln!(text, "GUID:        {}", p.guid);
    let _ = writeln!(
        text,
        "Status:      {}",
        if p.present() {
            format!("present -- {} handle(s) implement it", p.handle_count)
        } else {
            String::from("absent -- no handle in the protocol database implements it")
        }
    );
    text.push('\n');
    text.push_str(p.description);
    text
}
