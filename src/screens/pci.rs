//! PCI tab: every function found scanning bus 0 across each root bridge.

use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
use uefi::Char16;
use uefi::proto::console::text::{Key, ScanCode};

use super::{Action, Screen, move_selection};
use crate::explore::pci;

pub struct PciScreen {
    devices: Vec<pci::PciDevice>,
    table_state: TableState,
    detail_open: bool,
}

impl PciScreen {
    pub fn new() -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        Self {
            devices: pci::scan(),
            table_state,
            detail_open: false,
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
                self.detail_open = false;
            }
            Key::Special(ScanCode::DOWN) => {
                let sel = move_selection(self.table_state.selected(), 1, self.devices.len());
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
                .title(format!(
                    " PCI ({} devices) -- Enter: details ",
                    self.devices.len()
                ))
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan)),
        )
        .row_highlight_style(Style::default().bg(Color::Blue).fg(Color::White))
        .highlight_symbol("> ");

        frame.render_stateful_widget(table, area, &mut self.table_state);

        if self.detail_open
            && let Some(device) = self
                .table_state
                .selected()
                .and_then(|i| self.devices.get(i))
        {
            let text = device_detail_text(device);
            super::render_dialog(frame, area, " device details -- Enter to close ", |frame, inner| {
                frame.render_widget(Paragraph::new(text), inner);
            });
        }
    }
}

fn device_detail_text(d: &pci::PciDevice) -> alloc::string::String {
    use alloc::string::ToString;
    use core::fmt::Write;

    let mut text = alloc::string::String::new();
    let _ = writeln!(text, "Location:     {:02x}:{:02x}.{}", d.bus, d.device, d.function);
    let _ = writeln!(text, "Vendor:Device {:04x}:{:04x}", d.vendor_id, d.device_id);
    let _ = writeln!(text, "Class:        {} ({:#04x})", d.class_name(), d.class);
    let _ = writeln!(text, "Subclass:     {:#04x}", d.subclass);
    let _ = writeln!(text, "Prog IF:      {:#04x}", d.prog_if);
    let _ = writeln!(text, "Revision:     {:#04x}", d.revision);
    let _ = writeln!(
        text,
        "Header type:  {:#04x} ({})",
        d.header_type,
        if d.header_type & 0x7f == 0 {
            "normal device"
        } else if d.header_type & 0x7f == 1 {
            "PCI-to-PCI bridge"
        } else {
            "CardBus bridge"
        }
    );
    text.push('\n');
    text.push_str("Base Address Registers:\n");
    let mut any_bar = false;
    for i in 0..6 {
        if let Some(desc) = d.decode_bar(i) {
            any_bar = true;
            let _ = writeln!(text, "  BAR{i}: {desc}");
        }
    }
    if !any_bar {
        text.push_str("  (none)\n");
    }
    text.trim_end().to_string()
}
