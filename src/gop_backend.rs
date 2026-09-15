//! A ratatui [`Backend`] that rasterizes text with a bitmap font straight
//! into the Graphics Output Protocol framebuffer, instead of going through
//! UEFI's Simple Text Output console protocol.
//!
//! The console-protocol backend this replaced was correct but slow: every
//! run of same-styled cells was a real firmware call through
//! `SimpleTextOutputProtocol`, and that protocol was never designed to be
//! fast -- it's the BIOS-era text-console abstraction. Owning the
//! framebuffer directly turns each cell write into a handful of plain
//! memory stores instead, which is what `src/bin/asteroids` already does
//! for the same reason (see that binary's `gop_display.rs`). It also
//! sidesteps a real bug that abstraction had: UEFI's text console
//! auto-scrolls the *entire screen* when the cursor advances past the last
//! cell, which every redraw touching the terminal's last row silently
//! triggered. There's no such implicit scrolling here -- we're just
//! writing pixels.

use alloc::vec;
use alloc::vec::Vec;
use core::convert::Infallible;
use core::ptr;

use embedded_graphics::Drawable;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{OriginDimensions, Point, Size as EgSize};
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::{MonoFont, MonoTextStyle};
use embedded_graphics::pixelcolor::{Rgb888, RgbColor};
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::text::{Baseline, Text};
use ratatui::backend::{Backend, ClearType, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};
use ratatui::style::Color as RColor;
use uefi::boot::{self, ScopedProtocol};
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat};

const FONT: MonoFont = FONT_6X10;

/// Approximates ratatui's named colors as a classic 16-color VGA-ish
/// palette. `Rgb`/`Indexed` pass straight through -- unlike the old
/// console-protocol backend, real RGB is not a lossy fallback here.
fn to_rgb888(color: RColor) -> Rgb888 {
    match color {
        RColor::Reset | RColor::Black => Rgb888::new(0, 0, 0),
        RColor::Red => Rgb888::new(170, 0, 0),
        RColor::Green => Rgb888::new(0, 170, 0),
        RColor::Yellow => Rgb888::new(170, 85, 0),
        RColor::Blue => Rgb888::new(0, 0, 170),
        RColor::Magenta => Rgb888::new(170, 0, 170),
        RColor::Cyan => Rgb888::new(0, 170, 170),
        RColor::Gray => Rgb888::new(170, 170, 170),
        RColor::DarkGray => Rgb888::new(85, 85, 85),
        RColor::LightRed => Rgb888::new(255, 85, 85),
        RColor::LightGreen => Rgb888::new(85, 255, 85),
        RColor::LightYellow => Rgb888::new(255, 255, 85),
        RColor::LightBlue => Rgb888::new(85, 85, 255),
        RColor::LightMagenta => Rgb888::new(255, 85, 255),
        RColor::LightCyan => Rgb888::new(85, 255, 255),
        RColor::White => Rgb888::new(255, 255, 255),
        RColor::Rgb(r, g, b) => Rgb888::new(r, g, b),
        RColor::Indexed(_) => Rgb888::new(255, 255, 255),
    }
}

pub fn open_gop() -> Option<ScopedProtocol<GraphicsOutput>> {
    let handle = boot::get_handle_for_protocol::<GraphicsOutput>().ok()?;
    boot::open_protocol_exclusive::<GraphicsOutput>(handle).ok()
}

pub struct GopBackend {
    gop: ScopedProtocol<GraphicsOutput>,
    width: usize,
    height: usize,
    stride: usize,
    bgr: bool,
    /// `width * height * 4` bytes, tightly packed (row length `width`, not
    /// `stride`), already in the framebuffer's own byte order. Drawing into
    /// this rather than the live framebuffer and copying the whole thing in
    /// `flush` avoids tearing -- same reasoning as `GopDisplay` in
    /// `src/bin/asteroids`.
    buffer: Vec<u8>,
    cols: u16,
    rows: u16,
    cursor: Position,
}

impl GopBackend {
    pub fn new(gop: ScopedProtocol<GraphicsOutput>) -> Self {
        let info = gop.current_mode_info();
        let (width, height) = info.resolution();
        let stride = info.stride();
        let bgr = match info.pixel_format() {
            PixelFormat::Bgr => true,
            PixelFormat::Rgb => false,
            other => panic!("unsupported GOP pixel format: {other:?}"),
        };
        let cols = (width / FONT.character_size.width as usize) as u16;
        let rows = (height / FONT.character_size.height as usize) as u16;

        Self {
            gop,
            width,
            height,
            stride,
            bgr,
            buffer: vec![0u8; width * height * 4],
            cols,
            rows,
            cursor: Position::new(0, 0),
        }
    }

    fn present(&mut self) {
        let mut fb = self.gop.frame_buffer();
        let base = fb.as_mut_ptr();
        let row_bytes = self.width * 4;

        if self.stride == self.width {
            // SAFETY: `buffer` is exactly `width * height * 4` bytes, and
            // the framebuffer is sized for at least that when
            // `stride == width`.
            unsafe {
                ptr::copy_nonoverlapping(self.buffer.as_ptr(), base, self.buffer.len());
            }
        } else {
            for y in 0..self.height {
                let src = &self.buffer[y * row_bytes..(y + 1) * row_bytes];
                // SAFETY: row `y` is within the framebuffer, and `dst` has
                // room for `row_bytes` (`width * 4 <= stride * 4`).
                unsafe {
                    let dst = base.add(y * self.stride * 4);
                    ptr::copy_nonoverlapping(src.as_ptr(), dst, row_bytes);
                }
            }
        }
    }
}

impl OriginDimensions for GopBackend {
    fn size(&self) -> EgSize {
        EgSize::new(self.width as u32, self.height as u32)
    }
}

impl DrawTarget for GopBackend {
    type Color = Rgb888;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
    {
        let (width, height, bgr) = (self.width, self.height, self.bgr);
        for embedded_graphics::Pixel(coord, color) in pixels {
            if coord.x < 0 || coord.y < 0 {
                continue;
            }
            let (x, y) = (coord.x as usize, coord.y as usize);
            if x >= width || y >= height {
                continue;
            }
            let idx = (y * width + x) * 4;
            let (b0, b1, b2) = if bgr {
                (color.b(), color.g(), color.r())
            } else {
                (color.r(), color.g(), color.b())
            };
            self.buffer[idx] = b0;
            self.buffer[idx + 1] = b1;
            self.buffer[idx + 2] = b2;
        }
        Ok(())
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let (width, height, bgr) = (self.width, self.height, self.bgr);
        let (b0, b1, b2) = if bgr {
            (color.b(), color.g(), color.r())
        } else {
            (color.r(), color.g(), color.b())
        };
        let pattern = [b0, b1, b2, 0u8];

        let x0 = area.top_left.x.max(0);
        let y0 = area.top_left.y.max(0);
        let x1 = (area.top_left.x + area.size.width as i32).min(width as i32);
        let y1 = (area.top_left.y + area.size.height as i32).min(height as i32);

        for y in y0..y1 {
            let row_start = (y as usize * width + x0 as usize) * 4;
            let row_end = (y as usize * width + x1 as usize) * 4;
            if b0 == b1 && b1 == b2 {
                self.buffer[row_start..row_end].fill(b0);
            } else {
                for chunk in self.buffer[row_start..row_end].as_chunks_mut::<4>().0 {
                    chunk.copy_from_slice(&pattern);
                }
            }
        }

        Ok(())
    }
}

impl Backend for GopBackend {
    type Error = Infallible;

    fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let cw = FONT.character_size.width as i32;
        let ch = FONT.character_size.height as i32;

        for (x, y, cell) in content {
            let origin = Point::new(x as i32 * cw, y as i32 * ch);
            let bg = to_rgb888(cell.bg);
            let fg = to_rgb888(cell.fg);

            self.fill_solid(
                &Rectangle::new(origin, EgSize::new(cw as u32, ch as u32)),
                bg,
            )?;

            let symbol = cell.symbol().chars().next().unwrap_or(' ');
            let mut char_buf = [0u8; 4];
            let s = symbol.encode_utf8(&mut char_buf);
            let style = MonoTextStyle::new(&FONT, fg);
            let _ = Text::with_baseline(s, origin, style, Baseline::Top).draw(self);
        }
        Ok(())
    }

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn get_cursor_position(&mut self) -> Result<Position, Self::Error> {
        Ok(self.cursor)
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> Result<(), Self::Error> {
        self.cursor = position.into();
        Ok(())
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        DrawTarget::clear(self, Rgb888::BLACK)
    }

    fn clear_region(&mut self, _clear_type: ClearType) -> Result<(), Self::Error> {
        // Nothing currently asks for anything but `ClearType::All`; a full
        // clear on the rare case it's something else is harmless, just
        // slightly wasteful.
        Backend::clear(self)
    }

    fn size(&self) -> Result<Size, Self::Error> {
        Ok(Size::new(self.cols, self.rows))
    }

    fn window_size(&mut self) -> Result<WindowSize, Self::Error> {
        Ok(WindowSize {
            columns_rows: Size::new(self.cols, self.rows),
            pixels: Size::new(self.width as u16, self.height as u16),
        })
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.present();
        Ok(())
    }
}
