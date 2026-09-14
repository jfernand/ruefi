//! A ratatui [`Backend`] implementation on top of the UEFI Simple Text Output protocol.

use core::fmt;

use ratatui::backend::{Backend, ClearType, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};
use ratatui::style::Color as RColor;
use uefi::proto::console::text::Color as UColor;
use uefi::{CStr16, system};

/// Error type for [`UefiBackend`]. Wraps the `uefi::Error` produced by the
/// Simple Text Output protocol.
#[derive(Debug)]
pub struct UefiBackendError(uefi::Error);

impl fmt::Display for UefiBackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UEFI text output error: {:?}", self.0.status())
    }
}

impl core::error::Error for UefiBackendError {}

impl From<uefi::Error> for UefiBackendError {
    fn from(e: uefi::Error) -> Self {
        Self(e)
    }
}

fn to_uefi_color(color: RColor) -> UColor {
    match color {
        RColor::Reset | RColor::Black => UColor::Black,
        RColor::Red => UColor::Red,
        RColor::Green => UColor::Green,
        RColor::Yellow => UColor::Brown,
        RColor::Blue => UColor::Blue,
        RColor::Magenta => UColor::Magenta,
        RColor::Cyan => UColor::Cyan,
        RColor::Gray => UColor::LightGray,
        RColor::DarkGray => UColor::DarkGray,
        RColor::LightRed => UColor::LightRed,
        RColor::LightGreen => UColor::LightGreen,
        RColor::LightYellow => UColor::Yellow,
        RColor::LightBlue => UColor::LightBlue,
        RColor::LightMagenta => UColor::LightMagenta,
        RColor::LightCyan => UColor::LightCyan,
        RColor::White => UColor::White,
        // The UEFI console has no true-color support; fall back to white.
        RColor::Rgb(..) | RColor::Indexed(_) => UColor::White,
    }
}

/// A ratatui backend that renders through the firmware's Simple Text Output
/// protocol (i.e. the UEFI boot-time console).
///
/// Since the UEFI console is cell/attribute based (no true color, no
/// sub-cell graphics), this is a natural match for ratatui's model.
pub struct UefiBackend {
    size: Size,
    last_fg: Option<UColor>,
    last_bg: Option<UColor>,
}

impl UefiBackend {
    /// Creates a new backend, reading the current text mode's dimensions
    /// from the firmware.
    pub fn new() -> Self {
        // Mode 0 (80x25) is the one mode every UEFI text console is
        // required to support. Larger modes reported by `current_mode()`
        // (e.g. when the console is a ConSplitter multiplexing GOP and a
        // serial/terminal device) can advertise dimensions that
        // `set_cursor_position` then refuses past a much narrower real
        // limit, so pin to mode 0 for reliable cursor addressing.
        let (columns, rows) = system::with_stdout(|stdout| {
            let mode0 = stdout
                .modes()
                .next()
                .expect("console must support at least one text mode");
            let _ = stdout.set_mode(mode0);
            stdout
                .current_mode()
                .ok()
                .flatten()
                .map_or((80, 25), |m| (m.columns(), m.rows()))
        });

        Self {
            size: Size::new(columns as u16, rows as u16),
            last_fg: None,
            last_bg: None,
        }
    }

    fn set_color(&mut self, fg: UColor, bg: UColor) -> Result<(), UefiBackendError> {
        if self.last_fg.map(|c| c as usize) != Some(fg as usize)
            || self.last_bg.map(|c| c as usize) != Some(bg as usize)
        {
            system::with_stdout(|stdout| stdout.set_color(fg, bg))?;
            self.last_fg = Some(fg);
            self.last_bg = Some(bg);
        }
        Ok(())
    }
}

impl Default for UefiBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend for UefiBackend {
    type Error = UefiBackendError;

    #[allow(unused_assignments)]
    fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        // UCS-2 line buffer, plus null terminator, used to batch consecutive
        // same-row/same-style cells into a single `output_string` call.
        let mut buf = [0u16; 256];

        let mut run_len = 0usize;
        let mut run_x = 0u16;
        let mut run_y = 0u16;
        let mut run_fg = UColor::White;
        let mut run_bg = UColor::Black;
        let mut run_open = false;

        macro_rules! flush_run {
            () => {
                if run_open && run_len > 0 {
                    system::with_stdout(|stdout| {
                        stdout.set_cursor_position(run_x as usize, run_y as usize)
                    })?;
                    self.set_color(run_fg, run_bg)?;
                    buf[run_len] = 0;
                    let s = CStr16::from_u16_with_nul(&buf[..=run_len])
                        .map_err(|_| UefiBackendError(uefi::Status::INVALID_PARAMETER.into()))?;
                    system::with_stdout(|stdout| stdout.output_string_lossy(s))?;
                }
                run_len = 0;
                run_open = false;
            };
        }

        for (x, y, cell) in content {
            let fg = to_uefi_color(cell.fg);
            let bg = to_uefi_color(cell.bg);
            let ch = cell.symbol().chars().next().unwrap_or(' ');
            let code = if (ch as u32) < 0x10000 { ch as u16 } else { b'?' as u16 };

            let contiguous = run_open
                && y == run_y
                && x == run_x + run_len as u16
                && fg as usize == run_fg as usize
                && bg as usize == run_bg as usize
                && run_len < buf.len() - 1;

            if !contiguous {
                flush_run!();
                run_open = true;
                run_x = x;
                run_y = y;
                run_fg = fg;
                run_bg = bg;
            }

            buf[run_len] = code;
            run_len += 1;
        }
        flush_run!();

        Ok(())
    }

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        // Not all firmware/console combinations support cursor visibility
        // toggling (OVMF's text console notably returns `UNSUPPORTED`).
        // Treat that as a no-op rather than a hard error.
        let _ = system::with_stdout(|stdout| stdout.enable_cursor(false));
        Ok(())
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        let _ = system::with_stdout(|stdout| stdout.enable_cursor(true));
        Ok(())
    }

    fn get_cursor_position(&mut self) -> Result<Position, Self::Error> {
        let (x, y) = system::with_stdout(|stdout| stdout.cursor_position());
        Ok(Position::new(x as u16, y as u16))
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> Result<(), Self::Error> {
        let position = position.into();
        system::with_stdout(|stdout| {
            stdout.set_cursor_position(position.x as usize, position.y as usize)
        })?;
        Ok(())
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        system::with_stdout(|stdout| stdout.clear())?;
        self.last_fg = None;
        self.last_bg = None;
        Ok(())
    }

    fn clear_region(&mut self, clear_type: ClearType) -> Result<(), Self::Error> {
        match clear_type {
            ClearType::All => self.clear(),
            _ => Err(UefiBackendError(uefi::Status::UNSUPPORTED.into())),
        }
    }

    fn size(&self) -> Result<Size, Self::Error> {
        Ok(self.size)
    }

    fn window_size(&mut self) -> Result<WindowSize, Self::Error> {
        Ok(WindowSize {
            columns_rows: self.size,
            pixels: Size::new(0, 0),
        })
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
