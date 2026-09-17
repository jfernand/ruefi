//! An `embedded-graphics` [`DrawTarget`] backed by UEFI's Graphics Output
//! Protocol, double-buffered.
//!
//! Drawing writes into an owned, in-memory buffer (in the framebuffer's
//! own byte order, so [`GopDisplay::present`] is a straight memory copy);
//! writing straight into the live, scanned-out framebuffer (an earlier
//! version of this did) causes visible flicker/tearing, since the display
//! can read out a partially-drawn frame mid-update.
//!
//! `present` copies the buffer to video with `core::ptr::copy_nonoverlapping`
//! rather than `GraphicsOutput::blt`'s `BufferToVideo` operation: OVMF's
//! (software) `Blt` implementation turned out to be dramatically slower
//! than a native memcpy for a full-frame transfer done every tick, to the
//! point a real-time game driving it every frame was unplayably slow.
//! `blt`'s solid-color `VideoFill` is a different, hardware-acceleratable
//! operation and isn't affected by this -- but it isn't needed either,
//! since clears just fill the owned buffer like any other draw.

#![no_std]

extern crate alloc;

use core::convert::Infallible;
use core::ptr;

use alloc::vec;
use alloc::vec::Vec;
use embedded_graphics::Pixel;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{OriginDimensions, Size};
use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::pixelcolor::RgbColor;
use embedded_graphics::primitives::Rectangle;
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat};

pub struct GopDisplay<'a> {
    gop: &'a mut GraphicsOutput,
    width: usize,
    height: usize,
    stride: usize,
    bgr: bool,
    /// `width * height * 4` bytes, tightly packed (row length `width`,
    /// *not* `stride`), already in the framebuffer's own byte order.
    buffer: Vec<u8>,
}

impl<'a> GopDisplay<'a> {
    pub fn new(gop: &'a mut GraphicsOutput) -> Self {
        let info = gop.current_mode_info();
        let (width, height) = info.resolution();
        let stride = info.stride();
        let bgr = match info.pixel_format() {
            PixelFormat::Bgr => true,
            PixelFormat::Rgb => false,
            other => panic!("unsupported GOP pixel format: {other:?}"),
        };
        Self {
            gop,
            width,
            height,
            stride,
            bgr,
            buffer: vec![0u8; width * height * 4],
        }
    }

    /// Copies the completed frame to the screen. Call once per frame,
    /// after all drawing for that frame is done.
    pub fn present(&mut self) {
        let mut fb = self.gop.frame_buffer();
        let base = fb.as_mut_ptr();
        let row_bytes = self.width * 4;

        if self.stride == self.width {
            // Common case: no row padding, so the whole frame is one
            // contiguous copy.
            // SAFETY: `buffer` is exactly `width * height * 4` bytes, and
            // the framebuffer is sized for at least that when
            // `stride == width`.
            unsafe {
                ptr::copy_nonoverlapping(self.buffer.as_ptr(), base, self.buffer.len());
            }
        } else {
            for y in 0..self.height {
                let src = &self.buffer[y * row_bytes..(y + 1) * row_bytes];
                // SAFETY: row `y` is within the framebuffer, and `dst`
                // has room for `row_bytes` (`width * 4 <= stride * 4`).
                unsafe {
                    let dst = base.add(y * self.stride * 4);
                    ptr::copy_nonoverlapping(src.as_ptr(), dst, row_bytes);
                }
            }
        }
    }
}

impl OriginDimensions for GopDisplay<'_> {
    fn size(&self) -> Size {
        Size::new(self.width as u32, self.height as u32)
    }
}

impl DrawTarget for GopDisplay<'_> {
    type Color = Rgb888;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let (width, height, bgr) = (self.width, self.height, self.bgr);
        for Pixel(coord, color) in pixels {
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
                // A gray/black/white fill is byte-uniform regardless of
                // BGR/RGB order, so it can use a real memset instead of a
                // per-pixel copy -- which matters a lot for `clear()`,
                // called on the whole (typically 1M+ pixel) screen every
                // single frame.
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
