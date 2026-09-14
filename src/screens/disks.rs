//! Disks tab: every Block I/O device found, with an on-demand hex dump of
//! LBA 0 for whichever one is selected.

use alloc::format;
use alloc::vec;
use alloc::string::ToString;
use alloc::vec::Vec;

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Row, Table, TableState};
use uefi::Char16;
use uefi::proto::console::text::{Key, ScanCode};

use super::{Action, Screen, human_size, move_selection};
use crate::explore::disks;

pub struct DisksScreen {
    devices: Vec<disks::DiskDevice>,
    table_state: TableState,
    hex_dump: Option<Vec<u8>>,
}

impl DisksScreen {
    pub fn new() -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        Self {
            devices: disks::scan(),
            table_state,
            hex_dump: None,
        }
    }

    fn selected_device(&self) -> Option<&disks::DiskDevice> {
        self.table_state
            .selected()
            .and_then(|i| self.devices.get(i))
    }
}

impl Screen for DisksScreen {
    fn title(&self) -> &'static str {
        "Disks"
    }

    fn on_key(&mut self, key: Key) -> Action {
        match key {
            Key::Special(ScanCode::UP) => {
                let sel = move_selection(self.table_state.selected(), -1, self.devices.len());
                self.table_state.select(sel);
                self.hex_dump = None;
            }
            Key::Special(ScanCode::DOWN) => {
                let sel = move_selection(self.table_state.selected(), 1, self.devices.len());
                self.table_state.select(sel);
                self.hex_dump = None;
            }
            Key::Printable(c) if c == Char16::try_from('\r').unwrap() => {
                if self.hex_dump.is_some() {
                    self.hex_dump = None;
                } else if let Some(device) = self.selected_device() {
                    self.hex_dump = disks::read_first_block(device);
                }
            }
            _ => {}
        }
        Action::Continue
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        let header = Row::new(vec!["#", "Kind", "RO", "Block", "Blocks", "Size"])
            .style(Style::default().add_modifier(Modifier::BOLD));

        let rows = self.devices.iter().enumerate().map(|(i, d)| {
            let kind = if d.logical_partition {
                "partition"
            } else if d.removable {
                "removable"
            } else {
                "disk"
            };
            Row::new(vec![
                Cell::new(i.to_string()),
                Cell::new(kind),
                Cell::new(if d.read_only { "yes" } else { "no" }),
                Cell::new(d.block_size.to_string()),
                Cell::new((d.last_block + 1).to_string()),
                Cell::new(human_size(d.size_bytes())),
            ])
        });

        let table = Table::new(
            rows,
            [
                Constraint::Length(4),
                Constraint::Length(10),
                Constraint::Length(4),
                Constraint::Length(7),
                Constraint::Length(12),
                Constraint::Length(10),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .title(format!(
                    " disks ({} devices) -- Enter: hex-dump LBA 0 ",
                    self.devices.len()
                ))
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan)),
        )
        .row_highlight_style(Style::default().bg(Color::Blue).fg(Color::White))
        .highlight_symbol("> ");

        frame.render_stateful_widget(table, area, &mut self.table_state);

        if let Some(dump) = &self.hex_dump {
            super::render_hex_dialog(frame, area, " LBA 0 -- Enter to close ", dump);
        }
    }
}
