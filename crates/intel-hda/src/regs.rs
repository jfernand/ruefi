//! Controller register offsets and bit fields.
//!
//! See `docs/hda-register-map.typ` for the full reference this was written
//! against (Intel HDA spec rev 1.0a, section 3.3).

pub const GCAP: u32 = 0x00;
pub const VMIN: u32 = 0x02;
pub const VMAJ: u32 = 0x03;
pub const GCTL: u32 = 0x08;
pub const WAKEEN: u32 = 0x0C;
pub const STATESTS: u32 = 0x0E;
pub const GSTS: u32 = 0x10;
pub const INTCTL: u32 = 0x20;
pub const INTSTS: u32 = 0x24;
pub const WALCLK: u32 = 0x30;
pub const SSYNC: u32 = 0x38;
pub const CORBLBASE: u32 = 0x40;
pub const CORBUBASE: u32 = 0x44;
pub const CORBWP: u32 = 0x48;
pub const CORBRP: u32 = 0x4A;
pub const CORBCTL: u32 = 0x4C;
pub const CORBSTS: u32 = 0x4D;
pub const CORBSIZE: u32 = 0x4E;
pub const RIRBLBASE: u32 = 0x50;
pub const RIRBUBASE: u32 = 0x54;
pub const RIRBWP: u32 = 0x58;
pub const RINTCNT: u32 = 0x5A;
pub const RIRBCTL: u32 = 0x5C;
pub const RIRBSTS: u32 = 0x5D;
pub const RIRBSIZE: u32 = 0x5E;
pub const DPIBLBASE: u32 = 0x70;
pub const DPIBUBASE: u32 = 0x74;

/// Byte offset of stream descriptor `n`'s register block, given the number
/// of input streams reported by GCAP (needed because output streams are
/// numbered after all input streams).
pub const fn stream_desc_base(n: u32) -> u32 {
    0x80 + n * 0x20
}

pub mod sd {
    //! Offsets *within* one 0x20-byte stream descriptor block.
    pub const CTL_STS: u32 = 0x00;
    pub const LPIB: u32 = 0x04;
    pub const CBL: u32 = 0x08;
    pub const LVI: u32 = 0x0C;
    pub const FIFOS: u32 = 0x10;
    pub const FMT: u32 = 0x12;
    pub const BDPL: u32 = 0x18;
    pub const BDPU: u32 = 0x1C;
}

pub mod gctl {
    pub const CRST: u32 = 1 << 0;
    pub const FCNTRL: u32 = 1 << 1;
    pub const UNSOL: u32 = 1 << 8;
}

pub mod corbctl {
    pub const CMEIE: u8 = 1 << 0;
    pub const CORBRUN: u8 = 1 << 1;
}

pub mod rirbctl {
    pub const RINTCTL: u8 = 1 << 0;
    pub const RIRBDMAEN: u8 = 1 << 1;
    pub const RIRBOIC: u8 = 1 << 2;
}

pub mod rirbsts {
    pub const RINTFL: u8 = 1 << 0;
    pub const RIRBOIS: u8 = 1 << 2;
}

pub mod rirbwp {
    pub const RESET: u16 = 1 << 15;
}

pub mod corbrp {
    pub const RESET: u16 = 1 << 15;
}

/// Bits within the 32-bit `SDnCTL`/`SDnSTS` combined register.
pub mod sdctl {
    pub const SRST: u32 = 1 << 0;
    pub const RUN: u32 = 1 << 1;
    pub const IOCE: u32 = 1 << 2;
    pub const FEIE: u32 = 1 << 3;
    pub const DEIE: u32 = 1 << 4;
    pub const STRM_SHIFT: u32 = 20;
    pub const STRM_MASK: u32 = 0xF << STRM_SHIFT;

    pub const fn with_stream_tag(tag: u8) -> u32 {
        ((tag as u32) & 0xF) << STRM_SHIFT
    }
}

/// `GCAP` (Global Capabilities) fields.
pub struct Gcap {
    pub output_streams: u8,
    pub input_streams: u8,
    pub bidirectional_streams: u8,
    pub serial_data_out_lines: u8,
    pub supports_64bit: bool,
}

impl Gcap {
    pub fn decode(raw: u16) -> Self {
        Self {
            output_streams: ((raw >> 12) & 0xF) as u8,
            input_streams: ((raw >> 8) & 0xF) as u8,
            bidirectional_streams: ((raw >> 3) & 0x1F) as u8,
            serial_data_out_lines: ((raw >> 1) & 0x3) as u8,
            supports_64bit: raw & 1 != 0,
        }
    }

    /// Byte offset of output stream `n`'s descriptor block (0-based).
    pub fn output_stream_base(&self, n: u8) -> u32 {
        stream_desc_base(self.input_streams as u32 + n as u32)
    }
}
