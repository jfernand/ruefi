//! An `embedded-graphics` [`DrawTarget`] backed by a `minifb` window's pixel
//! buffer. Mirrors `gop-display`'s shape, but simpler: `minifb`'s
//! `Window::update_with_buffer` wants one `u32` per pixel in `0RGB` order,
//! so there's no BGR-vs-RGB runtime detection to do the way a real GOP
//! framebuffer needs.

use std::convert::Infallible;

use embedded_graphics::Pixel;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{OriginDimensions, Size};
use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::pixelcolor::RgbColor;
use embedded_graphics::primitives::Rectangle;

pub struct MinifbDisplay {
    width: usize,
    height: usize,
    /// One `0RGB` `u32` per pixel, handed straight to
    /// `Window::update_with_buffer` -- this buffer *is* the "present" step,
    /// there's no separate live-framebuffer copy the way GOP needs one.
    buffer: Vec<u32>,
}

fn pack(color: Rgb888) -> u32 {
    (u32::from(color.r()) << 16) | (u32::from(color.g()) << 8) | u32::from(color.b())
}

impl MinifbDisplay {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            buffer: vec![0u32; width * height],
        }
    }

    pub fn buffer(&self) -> &[u32] {
        &self.buffer
    }
}

impl OriginDimensions for MinifbDisplay {
    fn size(&self) -> Size {
        Size::new(self.width as u32, self.height as u32)
    }
}

impl DrawTarget for MinifbDisplay {
    type Color = Rgb888;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let (width, height) = (self.width, self.height);
        for Pixel(coord, color) in pixels {
            if coord.x < 0 || coord.y < 0 {
                continue;
            }
            let (x, y) = (coord.x as usize, coord.y as usize);
            if x >= width || y >= height {
                continue;
            }
            self.buffer[y * width + x] = pack(color);
        }
        Ok(())
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let (width, height) = (self.width, self.height);
        let packed = pack(color);

        let x0 = area.top_left.x.max(0);
        let y0 = area.top_left.y.max(0);
        let x1 = (area.top_left.x + area.size.width as i32).min(width as i32);
        let y1 = (area.top_left.y + area.size.height as i32).min(height as i32);

        for y in y0..y1 {
            let row_start = y as usize * width + x0 as usize;
            let row_end = y as usize * width + x1 as usize;
            self.buffer[row_start..row_end].fill(packed);
        }

        Ok(())
    }
}
