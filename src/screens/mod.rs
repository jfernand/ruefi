//! Each tab in the app is a self-contained [`Screen`]: it owns its own
//! state, renders itself, and maps keys to actions independently of every
//! other screen. `App` only knows how to ask "which tab is active" and
//! forward ticks to it.

use alloc::format;
use alloc::string::String;

use ratatui::Frame;
use ratatui::layout::Rect;
use uefi::proto::console::text::Key;

pub mod acpi;
pub mod disks;
pub mod memory;
pub mod menu;
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
