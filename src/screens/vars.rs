//! Variables tab: every UEFI NVRAM variable, with an on-demand hex dump of
//! the selected one's raw value.

use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
use uefi::Char16;
use uefi::proto::console::text::{Key, ScanCode};

use super::{Action, Screen, human_size, move_selection};
use crate::explore::vars;

pub struct VarsScreen {
    variables: Vec<vars::UefiVariable>,
    table_state: TableState,
    hex_dump: Option<Vec<u8>>,
}

impl VarsScreen {
    pub fn new() -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        Self {
            variables: vars::list(),
            table_state,
            hex_dump: None,
        }
    }
}

impl Screen for VarsScreen {
    fn title(&self) -> &'static str {
        "Variables"
    }

    fn on_key(&mut self, key: Key) -> Action {
        match key {
            Key::Special(ScanCode::UP) => {
                let sel = move_selection(self.table_state.selected(), -1, self.variables.len());
                self.table_state.select(sel);
                self.hex_dump = None;
            }
            Key::Special(ScanCode::DOWN) => {
                let sel = move_selection(self.table_state.selected(), 1, self.variables.len());
                self.table_state.select(sel);
                self.hex_dump = None;
            }
            Key::Printable(c) if c == Char16::try_from('\r').unwrap() => {
                if self.hex_dump.is_some() {
                    self.hex_dump = None;
                } else if let Some(variable) = self
                    .table_state
                    .selected()
                    .and_then(|i| self.variables.get(i))
                {
                    self.hex_dump = variable.read();
                }
            }
            _ => {}
        }
        Action::Continue
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        let [table_area, description_area] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).areas(area);

        let header = Row::new(vec!["Name", "Vendor", "Attrs", "Size"])
            .style(Style::default().add_modifier(Modifier::BOLD));

        let rows = self.variables.iter().map(|v| {
            Row::new(vec![
                Cell::new(v.name.clone()),
                Cell::new(v.vendor.clone()),
                Cell::new(v.attributes.clone()),
                Cell::new(human_size(v.size as u64)),
            ])
        });

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(40),
                Constraint::Length(10),
                Constraint::Length(8),
                Constraint::Length(10),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .title(format!(
                    " NVRAM variables ({}) -- Enter: hex-dump value ",
                    self.variables.len()
                ))
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan)),
        )
        .row_highlight_style(Style::default().bg(Color::Blue).fg(Color::White))
        .highlight_symbol("> ");

        frame.render_stateful_widget(table, table_area, &mut self.table_state);

        let selected = self.table_state.selected().and_then(|i| self.variables.get(i));
        let description = match selected {
            Some(v) => match v.description {
                Some(desc) => desc,
                None => "Vendor-specific variable -- not part of the UEFI spec's global namespace, so no standard meaning to show.",
            },
            None => "",
        };
        let description = Paragraph::new(description).block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::DarkGray)),
        );
        frame.render_widget(description, description_area);

        if let Some(dump) = &self.hex_dump {
            super::render_hex_dialog(frame, area, " variable value -- Enter to close ", dump);
        }
    }
}
