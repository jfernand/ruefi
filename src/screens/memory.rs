//! Memory map tab: a summary of the UEFI memory map, grouped by type.

use alloc::format;
use alloc::vec;
use alloc::string::ToString;
use alloc::string::String;
use alloc::vec::Vec;

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Row, Table, TableState};
use uefi::proto::console::text::{Key, ScanCode};

use super::{Action, Screen, human_size, move_selection};
use crate::explore::memmap;

pub struct MemoryScreen {
    summary: Vec<(String, u64, u64)>,
    table_state: TableState,
}

impl MemoryScreen {
    pub fn new() -> Self {
        let regions = memmap::snapshot();
        let summary = memmap::summarize(&regions);
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        Self {
            summary,
            table_state,
        }
    }
}

impl Screen for MemoryScreen {
    fn title(&self) -> &'static str {
        "Memory"
    }

    fn on_key(&mut self, key: Key) -> Action {
        match key {
            Key::Special(ScanCode::UP) => {
                let sel = move_selection(self.table_state.selected(), -1, self.summary.len());
                self.table_state.select(sel);
            }
            Key::Special(ScanCode::DOWN) => {
                let sel = move_selection(self.table_state.selected(), 1, self.summary.len());
                self.table_state.select(sel);
            }
            _ => {}
        }
        Action::Continue
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        let header = Row::new(vec!["Type", "Regions", "Pages", "Size"])
            .style(Style::default().add_modifier(Modifier::BOLD));

        let rows = self.summary.iter().map(|(ty, pages, regions)| {
            Row::new(vec![
                Cell::new(ty.clone()),
                Cell::new(regions.to_string()),
                Cell::new(pages.to_string()),
                Cell::new(human_size(pages * 4096)),
            ])
        });

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(40),
                Constraint::Length(9),
                Constraint::Length(12),
                Constraint::Length(10),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .title(format!(" memory map ({} regions) ", self.summary.len()))
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan)),
        )
        .row_highlight_style(Style::default().bg(Color::Blue).fg(Color::White))
        .highlight_symbol("> ");

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }
}
