//! Memory map tab: a summary of the UEFI memory map, grouped by type.

use alloc::format;
use alloc::vec;
use alloc::string::ToString;
use alloc::string::String;
use alloc::vec::Vec;

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
use uefi::Char16;
use uefi::proto::console::text::{Key, ScanCode};

use super::{Action, Screen, human_size, move_selection};
use crate::explore::memmap;

/// Most individual regions to list in the detail dialog before truncating
/// -- a handful of memory types (e.g. `CONVENTIONAL`) can have hundreds of
/// small fragmented regions, more than a dialog can usefully show anyway.
const MAX_DETAIL_REGIONS: usize = 60;

pub struct MemoryScreen {
    regions: Vec<memmap::MemRegion>,
    summary: Vec<(String, u64, u64)>,
    table_state: TableState,
    detail_open: bool,
}

impl MemoryScreen {
    pub fn new() -> Self {
        let regions = memmap::snapshot();
        let summary = memmap::summarize(&regions);
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        Self {
            regions,
            summary,
            table_state,
            detail_open: false,
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
                self.detail_open = false;
            }
            Key::Special(ScanCode::DOWN) => {
                let sel = move_selection(self.table_state.selected(), 1, self.summary.len());
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
                .title(format!(
                    " memory map ({} regions) -- Enter: list regions ",
                    self.summary.len()
                ))
                .borders(Borders::ALL)
                .border_set(super::ASCII_BORDER)
                .style(Style::default().fg(Color::Cyan)),
        )
        .row_highlight_style(Style::default().bg(Color::Blue).fg(Color::White))
        .highlight_symbol("> ");

        frame.render_stateful_widget(table, area, &mut self.table_state);

        if self.detail_open
            && let Some((ty, ..)) = self.table_state.selected().and_then(|i| self.summary.get(i))
        {
            let text = region_list_text(&self.regions, ty);
            let title = alloc::format!(" {ty} regions -- Enter to close ");
            super::render_dialog(frame, area, &title, |frame, inner| {
                frame.render_widget(Paragraph::new(text), inner);
            });
        }
    }
}

fn region_list_text(regions: &[memmap::MemRegion], ty: &str) -> String {
    use core::fmt::Write;

    let matching: Vec<&memmap::MemRegion> = regions.iter().filter(|r| r.ty == ty).collect();

    let mut text = String::new();
    for region in matching.iter().take(MAX_DETAIL_REGIONS) {
        let _ = writeln!(
            text,
            "{:#018x}  {:>8} pages  {}",
            region.phys_start,
            region.page_count,
            human_size(region.size_bytes())
        );
    }
    if matching.len() > MAX_DETAIL_REGIONS {
        let _ = writeln!(
            text,
            "\n... and {} more",
            matching.len() - MAX_DETAIL_REGIONS
        );
    }
    text.trim_end().to_string()
}
