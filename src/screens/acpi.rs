//! ACPI tab: every top-level table found via the RSDP -> XSDT/RSDT walk.

use alloc::format;
use alloc::vec;
use alloc::string::ToString;

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
use uefi::proto::console::text::{Key, ScanCode};

use super::{Action, Screen, move_selection};
use crate::explore::acpi;

pub struct AcpiScreen {
    info: Option<acpi::AcpiInfo>,
    table_state: TableState,
}

impl AcpiScreen {
    pub fn new() -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        Self {
            info: acpi::discover(),
            table_state,
        }
    }
}

impl Screen for AcpiScreen {
    fn title(&self) -> &'static str {
        "ACPI"
    }

    fn on_key(&mut self, key: Key) -> Action {
        let len = self.info.as_ref().map_or(0, |i| i.tables.len());
        match key {
            Key::Special(ScanCode::UP) => {
                let sel = move_selection(self.table_state.selected(), -1, len);
                self.table_state.select(sel);
            }
            Key::Special(ScanCode::DOWN) => {
                let sel = move_selection(self.table_state.selected(), 1, len);
                self.table_state.select(sel);
            }
            _ => {}
        }
        Action::Continue
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        let Some(info) = &self.info else {
            let msg = Paragraph::new("No ACPI configuration table found on this platform.").block(
                Block::default()
                    .title(" ACPI ")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Cyan)),
            );
            frame.render_widget(msg, area);
            return;
        };

        let header = Row::new(vec!["Signature", "Address", "Length", "OEM ID"])
            .style(Style::default().add_modifier(Modifier::BOLD));

        let rows = info.tables.iter().map(|t| {
            Row::new(vec![
                Cell::new(t.signature.clone()),
                Cell::new(format!("{:#010x}", t.address)),
                Cell::new(t.length.to_string()),
                Cell::new(t.oem_id.clone()),
            ])
        });

        let table = Table::new(
            rows,
            [
                Constraint::Length(10),
                Constraint::Length(14),
                Constraint::Length(10),
                Constraint::Length(10),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .title(format!(
                    " ACPI {}.0, {} tables ",
                    info.revision,
                    info.tables.len()
                ))
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan)),
        )
        .row_highlight_style(Style::default().bg(Color::Blue).fg(Color::White))
        .highlight_symbol("> ");

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }
}
