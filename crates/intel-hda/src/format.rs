//! The 16-bit PCM Format Structure used by both `SDnFMT` and the codec's
//! Converter Format control -- they must be programmed identically.

#[derive(Debug, Clone, Copy)]
pub enum BitsPerSample {
    Eight,
    Sixteen,
    Twenty,
    TwentyFour,
    ThirtyTwo,
}

impl BitsPerSample {
    const fn code(self) -> u16 {
        match self {
            Self::Eight => 0b000,
            Self::Sixteen => 0b001,
            Self::Twenty => 0b010,
            Self::TwentyFour => 0b011,
            Self::ThirtyTwo => 0b100,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PcmFormat {
    pub sample_rate_hz: u32,
    pub bits: BitsPerSample,
    pub channels: u8,
}

impl PcmFormat {
    /// Common CD-quality stereo format: 48 kHz, 16-bit, 2 channels.
    pub const CD_STEREO: Self = Self {
        sample_rate_hz: 48_000,
        bits: BitsPerSample::Sixteen,
        channels: 2,
    };

    /// Encodes this format into the 16-bit structure from spec section 3.7.1.
    /// Returns `None` for a sample rate this encoding can't represent (only
    /// the base 44.1/48 kHz rates and their listed multiples/divisors are
    /// representable).
    pub fn encode(&self) -> Option<u16> {
        let (base, mult, div) = rate_factors(self.sample_rate_hz)?;
        if self.channels == 0 || self.channels > 16 {
            return None;
        }
        let mut value: u16 = 0;
        value |= base << 14;
        value |= mult << 11;
        value |= div << 8;
        value |= self.bits.code() << 4;
        value |= (self.channels as u16 - 1) & 0xF;
        Some(value)
    }
}

/// Finds `(base, mult, div)` such that `base_hz(base) * (mult+1) / (div+1) ==
/// hz`, per the base-rate table in spec section 3.7.1.
fn rate_factors(hz: u32) -> Option<(u16, u16, u16)> {
    const MULTS: [u32; 4] = [1, 2, 3, 4];
    for (base_code, base_hz) in [(0u16, 48_000u32), (1u16, 44_100u32)] {
        for (div_code, div) in (1u32..=8).enumerate() {
            for (mult_idx, &mult) in MULTS.iter().enumerate() {
                if base_hz * mult / div == hz && base_hz * mult % div == 0 {
                    return Some((base_code, mult_idx as u16, div_code as u16));
                }
            }
        }
    }
    None
}
