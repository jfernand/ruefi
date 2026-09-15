//! Codec verb IDs and CORB command encoding.
//!
//! A CORB entry is `(codec_addr << 28) | (nid << 20) | payload20`, where
//! `payload20` packs a verb and its argument one of two ways depending on
//! the verb (see `docs/hda-register-map.typ`, "Codec verb encoding"):
//!
//! - most verbs: 12-bit verb in bits 19:8, 8-bit payload in bits 7:0
//! - format/amp/proc-coef/coef-index verbs: 4-bit verb in bits 19:16,
//!   16-bit payload in bits 15:0
//!
//! Both cases reduce to `(verb << shift) | payload`, so [`Verb8`] and
//! [`Verb16`] just carry the pre-shifted verb constant for each family.

/// A verb taking an 8-bit payload (`(verb << 8) | payload`).
pub struct Verb8(pub u32);
/// A verb taking a 16-bit payload (`(verb << 16) | payload`).
pub struct Verb16(pub u32);

impl Verb8 {
    pub const fn payload(&self, arg: u8) -> u32 {
        self.0 | arg as u32
    }
}

impl Verb16 {
    pub const fn payload(&self, arg: u16) -> u32 {
        self.0 | arg as u32
    }
}

pub const GET_PARAMETER: Verb8 = Verb8(0xF00 << 8);
pub const GET_CONNECTION_SELECT: Verb8 = Verb8(0xF01 << 8);
pub const SET_CONNECTION_SELECT: Verb8 = Verb8(0x701 << 8);
pub const GET_CONNECTION_LIST_ENTRY: Verb8 = Verb8(0xF02 << 8);
pub const GET_POWER_STATE: Verb8 = Verb8(0xF05 << 8);
pub const SET_POWER_STATE: Verb8 = Verb8(0x705 << 8);
pub const GET_CONVERTER_STREAM_CHANNEL: Verb8 = Verb8(0xF06 << 8);
pub const SET_CONVERTER_STREAM_CHANNEL: Verb8 = Verb8(0x706 << 8);
pub const GET_PIN_WIDGET_CONTROL: Verb8 = Verb8(0xF07 << 8);
pub const SET_PIN_WIDGET_CONTROL: Verb8 = Verb8(0x707 << 8);
pub const GET_UNSOLICITED_RESPONSE: Verb8 = Verb8(0xF08 << 8);
pub const SET_UNSOLICITED_RESPONSE: Verb8 = Verb8(0x708 << 8);
pub const GET_PIN_SENSE: Verb8 = Verb8(0xF09 << 8);
pub const EXECUTE_PIN_SENSE: Verb8 = Verb8(0x709 << 8);
pub const GET_EAPD_BTL: Verb8 = Verb8(0xF0C << 8);
pub const SET_EAPD_BTL: Verb8 = Verb8(0x70C << 8);

pub const GET_CONVERTER_FORMAT: Verb16 = Verb16(0xA << 16);
pub const SET_CONVERTER_FORMAT: Verb16 = Verb16(0x2 << 16);
pub const GET_AMP_GAIN_MUTE: Verb16 = Verb16(0xB << 16);
pub const SET_AMP_GAIN_MUTE: Verb16 = Verb16(0x3 << 16);

/// Node 0 of every codec: the Root Node.
pub const ROOT_NODE: u8 = 0;

pub mod param_id {
    pub const VENDOR_ID: u8 = 0x00;
    pub const SUBORDINATE_NODE_COUNT: u8 = 0x04;
    pub const FUNCTION_GROUP_TYPE: u8 = 0x05;
    pub const AUDIO_FG_CAPS: u8 = 0x08;
    pub const AUDIO_WIDGET_CAPS: u8 = 0x09;
    pub const SUPPORTED_PCM_SIZE_RATES: u8 = 0x0A;
    pub const SUPPORTED_STREAM_FORMATS: u8 = 0x0B;
    pub const PIN_CAPS: u8 = 0x0C;
    pub const INPUT_AMP_CAPS: u8 = 0x0D;
    pub const CONNECTION_LIST_LENGTH: u8 = 0x0E;
    pub const SUPPORTED_POWER_STATES: u8 = 0x0F;
    pub const OUTPUT_AMP_CAPS: u8 = 0x12;
}

/// Builds a full 32-bit CORB entry addressed to `codec_addr`/`nid` carrying
/// the already-encoded 20-bit `payload` (from [`Verb8::payload`] or
/// [`Verb16::payload`]).
pub const fn command(codec_addr: u8, nid: u8, payload: u32) -> u32 {
    ((codec_addr as u32) << 28) | ((nid as u32) << 20) | (payload & 0xF_FFFF)
}

/// Function Group Type value identifying an Audio Function Group.
pub const NODE_TYPE_AUDIO_FUNCTION_GROUP: u8 = 0x01;

/// Widget `Type` field decoded from Audio Widget Capabilities bits 23:20.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetType {
    AudioOutput,
    AudioInput,
    Mixer,
    Selector,
    PinComplex,
    Power,
    VolumeKnob,
    BeepGenerator,
    Vendor,
    Reserved(u8),
}

impl WidgetType {
    pub fn decode(raw: u8) -> Self {
        match raw {
            0x0 => Self::AudioOutput,
            0x1 => Self::AudioInput,
            0x2 => Self::Mixer,
            0x3 => Self::Selector,
            0x4 => Self::PinComplex,
            0x5 => Self::Power,
            0x6 => Self::VolumeKnob,
            0x7 => Self::BeepGenerator,
            0xF => Self::Vendor,
            other => Self::Reserved(other),
        }
    }
}
