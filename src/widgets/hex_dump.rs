//! A reusable hex-dump widget: 16 bytes per row as an address column, hex
//! bytes, and an ASCII gutter -- the standard `xxd`-style layout.

use alloc::format;
use alloc::string::ToString;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget;

pub struct HexDump<'a> {
    data: &'a [u8],
    base: usize,
}

impl<'a> HexDump<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, base: 0 }
    }

    /// The address printed for `data[0]`, so a dump of a disk sector (say)
    /// can show real LBA-relative offsets rather than always starting at 0.
    /// Not used yet (every current caller dumps from offset 0), but part of
    /// the widget's public API for whoever dumps something else next.
    #[allow(dead_code)]
    pub fn base(mut self, base: usize) -> Self {
        self.base = base;
        self
    }
}

const ADDR_STYLE: Style = Style::new().fg(Color::DarkGray);
const HEX_STYLE: Style = Style::new().fg(Color::Cyan);
const ZERO_STYLE: Style = Style::new().fg(Color::DarkGray);
const ASCII_STYLE: Style = Style::new().fg(Color::White);

impl Widget for HexDump<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for (row, chunk) in self.data.chunks(16).enumerate() {
            let y = area.y + row as u16;
            if y >= area.y + area.height {
                break;
            }

            let mut x = area.x;
            let addr = format!("{:06x}  ", self.base + row * 16);
            buf.set_string(x, y, &addr, ADDR_STYLE);
            x += addr.chars().count() as u16;

            for i in 0..16 {
                let style = match chunk.get(i) {
                    Some(0) => ZERO_STYLE,
                    Some(_) => HEX_STYLE,
                    None => ADDR_STYLE,
                };
                let s = match chunk.get(i) {
                    Some(b) => format!("{b:02x} "),
                    None => "   ".into(),
                };
                buf.set_string(x, y, &s, style);
                x += 3;
                if i == 7 {
                    x += 1;
                }
            }

            x += 1;
            for &b in chunk {
                let c = if b.is_ascii_graphic() || b == b' ' {
                    b as char
                } else {
                    '.'
                };
                buf.set_string(x, y, c.to_string(), ASCII_STYLE);
                x += 1;
            }
        }
    }
}
