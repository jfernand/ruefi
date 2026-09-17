//! Codec output-path discovery and bring-up, shared by every caller of this
//! driver that just wants to find a speaker/line-out and play PCM through
//! it -- originally written once for the `ruefi` Audio tab's test tone, and
//! promoted here so other callers (e.g. game sound effects) don't have to
//! duplicate it.

use crate::controller::{Controller, Error};
use crate::format::PcmFormat;
use crate::platform::{DmaBuffer, Platform};
use crate::verbs::WidgetType;

/// A codec output path this driver knows how to drive: `dac_nid` is the
/// first entry in `pin_nid`'s connection list, i.e. what the pin is
/// currently wired to accept audio from.
#[derive(Debug, Clone, Copy)]
pub struct Target {
    pub codec_addr: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub dac_nid: u8,
    pub pin_nid: u8,
}

/// Finds the first codec with a widget path this driver knows how to drive:
/// an Audio Function Group, an output-capable Pin Complex under it, and a
/// DAC in that pin's connection list.
pub fn find_output_target<P: Platform>(controller: &mut Controller<P>) -> Option<Target> {
    let presence = controller.codec_presence();
    for codec_addr in 0..15u8 {
        if presence & (1 << codec_addr) == 0 {
            continue;
        }
        let Ok((vendor_id, device_id)) = controller.vendor_device_id(codec_addr) else {
            continue;
        };
        let Ok(Some(afg)) = controller.find_audio_function_group(codec_addr) else {
            continue;
        };

        let mut pin_nid = None;
        let _ = controller.for_each_widget(codec_addr, afg, |controller, nid, caps| {
            if pin_nid.is_some() || caps.wtype != WidgetType::PinComplex {
                return;
            }
            if let Ok(pin_caps) = controller.pin_caps(codec_addr, nid)
                && pin_caps.output_capable
            {
                pin_nid = Some(nid);
            }
        });

        let Some(pin_nid) = pin_nid else { continue };
        let mut connections = [0u8; 4];
        let count = controller
            .connection_list(codec_addr, pin_nid, &mut connections)
            .unwrap_or(0);
        if count == 0 {
            continue;
        }

        return Some(Target {
            codec_addr,
            vendor_id,
            device_id,
            dac_nid: connections[0],
            pin_nid,
        });
    }
    None
}

/// Unmutes `nid`'s output amplifier at its reported 0 dB point. If the amp
/// doesn't report any usable step info (either it has no amp there, or --
/// like QEMU's built-in HDA codec -- it just doesn't implement the
/// Amplifier Capabilities parameter and reports all-zero), leaves the amp
/// alone rather than programming an explicit gain of 0, which would be the
/// minimum (near-silent) setting on hardware that *does* implement it.
pub fn unmute_output_at_zero_db<P: Platform>(controller: &mut Controller<P>, codec_addr: u8, nid: u8) {
    if let Ok(caps) = controller.output_amp_caps(codec_addr, nid)
        && caps.num_steps > 0
    {
        controller.set_output_amp_gain_mute(codec_addr, nid, false, caps.offset);
    }
}

/// Everything [`configure_and_play_output`] needs about the stream it's
/// starting, beyond the [`Target`] and the buffer itself.
#[derive(Debug, Clone, Copy)]
pub struct StreamPlan {
    pub index: u8,
    pub tag: u8,
    pub format: PcmFormat,
    pub last_valid_index: u16,
    pub cyclic_buffer_len: u32,
}

/// Wires up `target`'s codec side (converter format/stream-channel, pin
/// widget control, EAPD, amp unmute) and starts `plan`'s stream playing
/// `bdl`. Every caller of this driver that plays a precomputed buffer
/// through a discovered [`Target`] wants exactly this sequence -- only the
/// stream and buffer contents differ per caller (e.g. one stream per
/// concurrent game sound).
pub fn configure_and_play_output<P: Platform>(
    controller: &mut Controller<P>,
    target: &Target,
    bdl: &DmaBuffer,
    plan: StreamPlan,
) -> Result<(), Error> {
    controller.set_converter_format(target.codec_addr, target.dac_nid, plan.format);
    controller.set_converter_stream_channel(target.codec_addr, target.dac_nid, plan.tag, 0);
    unmute_output_at_zero_db(controller, target.codec_addr, target.dac_nid);

    controller.set_pin_widget_control(target.codec_addr, target.pin_nid, true, false, true);
    unmute_output_at_zero_db(controller, target.codec_addr, target.pin_nid);
    controller.set_eapd(target.codec_addr, target.pin_nid, true);

    controller.configure_output_stream(
        plan.index,
        plan.tag,
        plan.format,
        bdl,
        plan.last_valid_index,
        plan.cyclic_buffer_len,
    )?;
    controller.start_output_stream(plan.index);
    Ok(())
}
