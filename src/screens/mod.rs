//! Each tab in the app is a self-contained [`Screen`]: it owns its own
//! state, renders itself, and maps keys to actions independently of every
//! other screen. `App` only knows how to ask "which tab is active" and
//! forward ticks to it.

use alloc::format;
use alloc::string::String;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Clear};
use uefi::proto::console::text::Key;

use crate::widgets::HexDump;

pub mod acpi;
pub mod disks;
pub mod memory;
pub mod pci;
pub mod vars;

/// What a screen wants the app to do after handling one key.
pub enum Action {
    Continue,
    Quit,
}

/// One tab of the UI.
pub trait Screen {
    /// Shown in the tab bar.
    fn title(&self) -> &'static str;

    /// Handles a single keypress that wasn't already claimed by the app's
    /// global bindings (tab switching, quit). Returns what should happen
    /// next.
    fn on_key(&mut self, key: Key) -> Action;

    /// Draws the screen's content into `area` (everything below the tab
    /// bar and above the footer).
    fn render(&mut self, frame: &mut Frame, area: Rect);
}

/// Formats a byte count as a human-friendly size, e.g. `4.0 MB`.
/// Uses 1024 as the step between units (so this is really binary/IEC
/// sizing), but keeps the plain `kB`/`MB`/`GB` labels rather than
/// `KiB`/`MiB`/`GiB`.
pub(crate) fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "kB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[0])
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

/// Moves a table/list selection by `delta`, clamped to `[0, len)`. Shared
/// by every screen with a scrollable table or list.
pub(crate) fn move_selection(selected: Option<usize>, delta: i32, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let current = selected.unwrap_or(0) as i32;
    Some((current + delta).clamp(0, len as i32 - 1) as usize)
}

/// A `percent_x` by `percent_y` rectangle centered within `area` -- the
/// standard ratatui recipe for a modal dialog's bounds.
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let [_, middle, _] = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .areas(area);

    let [_, center, _] = Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .areas(middle);

    center
}

/// Draws a modal dialog centered over `area`, on top of whatever the
/// screen already rendered there, and hands the caller its inner
/// (border-excluded) area to fill in however it likes. Shared by every
/// screen that drills into a row's details -- a raw byte dump, a
/// device's full field breakdown, whatever.
pub(crate) fn render_dialog(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    content: impl FnOnce(&mut Frame, Rect),
) {
    let popup = centered_rect(80, 70, area);

    // Erase whatever the table drew underneath before painting the dialog
    // -- otherwise stray cells (e.g. table borders) can peek through
    // wherever the dialog's own content doesn't fully repaint a cell.
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::Cyan).bg(Color::Black));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    content(frame, inner);
}

/// Draws a hex dump as a modal dialog. See [`render_dialog`].
pub(crate) fn render_hex_dialog(frame: &mut Frame, area: Rect, title: &str, dump: &[u8]) {
    render_dialog(frame, area, title, |frame, inner| {
        frame.render_widget(HexDump::new(dump), inner);
    });
}
