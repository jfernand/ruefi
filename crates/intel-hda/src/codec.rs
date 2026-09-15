//! Decoded shapes of the codec parameter responses relevant to routing audio
//! to an output pin. See `docs/hda-register-map.typ` section 4 for the raw
//! bit layouts these are built from.

use crate::verbs::WidgetType;

#[derive(Debug, Clone, Copy)]
pub struct WidgetCaps {
    pub wtype: WidgetType,
    pub stereo: bool,
    pub in_amp_present: bool,
    pub out_amp_present: bool,
    pub amp_param_override: bool,
    pub format_override: bool,
    pub conn_list: bool,
    pub power_cntrl: bool,
    pub digital: bool,
    pub unsol_capable: bool,
}

impl WidgetCaps {
    pub fn decode(raw: u32) -> Self {
        Self {
            wtype: WidgetType::decode(((raw >> 20) & 0xF) as u8),
            stereo: raw & 1 != 0,
            in_amp_present: raw & (1 << 1) != 0,
            out_amp_present: raw & (1 << 2) != 0,
            amp_param_override: raw & (1 << 3) != 0,
            format_override: raw & (1 << 4) != 0,
            conn_list: raw & (1 << 8) != 0,
            power_cntrl: raw & (1 << 10) != 0,
            digital: raw & (1 << 9) != 0,
            unsol_capable: raw & (1 << 7) != 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PinCaps {
    pub input_capable: bool,
    pub output_capable: bool,
    pub headphone_drive_capable: bool,
    pub presence_detect_capable: bool,
    pub eapd_capable: bool,
    pub hdmi: bool,
}

impl PinCaps {
    pub fn decode(raw: u32) -> Self {
        Self {
            input_capable: raw & (1 << 5) != 0,
            output_capable: raw & (1 << 4) != 0,
            headphone_drive_capable: raw & (1 << 3) != 0,
            presence_detect_capable: raw & (1 << 2) != 0,
            eapd_capable: raw & (1 << 16) != 0,
            hdmi: raw & (1 << 7) != 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AmpCaps {
    pub mute_capable: bool,
    pub num_steps: u8,
    pub offset: u8,
}

impl AmpCaps {
    pub fn decode(raw: u32) -> Self {
        Self {
            mute_capable: raw & (1 << 31) != 0,
            num_steps: ((raw >> 8) & 0x7F) as u8,
            offset: (raw & 0x7F) as u8,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Codec {
    pub addr: u8,
    pub vendor_id: u16,
    pub device_id: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct Widget {
    pub nid: u8,
    pub caps: WidgetCaps,
}
