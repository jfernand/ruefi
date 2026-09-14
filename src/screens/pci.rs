//! PCI tab: every function found scanning bus 0 across each root bridge.

use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Row, Table, TableState};
use uefi::proto::console::text::{Key, ScanCode};

use super::{Action, Screen, move_selection};
use crate::explore::pci;

pub struct PciScreen {
    devices: Vec<pci::PciDevice>,
    table_state: TableState,
}

impl PciScreen {
    pub fn new() -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        Self {
            devices: pci::scan(),
            table_state,
        }
    }
}

impl Screen for PciScreen {
    fn title(&self) -> &'static str {
        "PCI"
    }

    fn on_key(&mut self, key: Key) -> Action {
        match key {
            Key::Special(ScanCode::UP) => {
                let sel = move_selection(self.table_state.selected(), -1, self.devices.len());
                self.table_state.select(sel);
            }
            Key::Special(ScanCode::DOWN) => {
                let sel = move_selection(self.table_state.selected(), 1, self.devices.len());
                self.table_state.select(sel);
            }
            _ => {}
        }
        Action::Continue
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        let header = Row::new(vec!["Location", "Vendor:Device", "Class", "Rev"])
            .style(Style::default().add_modifier(Modifier::BOLD));

        let rows = self.devices.iter().map(|d| {
            Row::new(vec![
                Cell::new(format!("{:02x}:{:02x}.{}", d.bus, d.device, d.function)),
                Cell::new(format!("{:04x}:{:04x}", d.vendor_id, d.device_id)),
                Cell::new(d.class_name()),
                Cell::new(format!("{:#04x}", d.revision)),
            ])
        });

        let table = Table::new(
            rows,
            [
                Constraint::Length(10),
                Constraint::Length(14),
                Constraint::Percentage(50),
                Constraint::Length(6),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .title(format!(" PCI ({} devices) ", self.devices.len()))
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan)),
        )
        .row_highlight_style(Style::default().bg(Color::Blue).fg(Color::White))
        .highlight_symbol("> ");

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }
}
