//! Audio tab: opens the Intel HD Audio controller (if any), finds a codec
//! output path, and can play a test tone through it.

use alloc::format;
use alloc::string::String;

use intel_hda::verbs::WidgetType;
use intel_hda::{Controller, DmaBuffer, PcmFormat};
use intel_hda_uefi::UefiPlatform;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use uefi::Char16;
use uefi::proto::console::text::Key;

use super::{Action, Screen};

const STREAM_INDEX: u8 = 0;
const STREAM_TAG: u8 = 1;
const TONE_HZ: f32 = 440.0;
const TONE_SECONDS: f32 = 2.0;
const TONE_AMPLITUDE: f32 = 8000.0;

/// A codec output path this screen has found and can play through: `dac_nid`
/// is the first entry in `pin_nid`'s connection list, i.e. what the pin is
/// currently wired to accept audio from.
struct Target {
    codec_addr: u8,
    vendor_id: u16,
    device_id: u16,
    dac_nid: u8,
    pin_nid: u8,
}

/// The sample buffer and buffer descriptor list backing a playing tone --
/// kept alive for as long as the stream might still be reading them.
struct ToneResources {
    #[allow(dead_code)]
    samples: DmaBuffer,
    #[allow(dead_code)]
    bdl: DmaBuffer,
}

pub struct HdaScreen {
    controller: Option<Controller<UefiPlatform>>,
    target: Option<Target>,
    tone: Option<ToneResources>,
    playing: bool,
    status: String,
}

impl HdaScreen {
    pub fn new() -> Self {
        match intel_hda_uefi::open() {
            Ok(mut controller) => {
                let target = discover_target(&mut controller);
                let status = match &target {
                    Some(t) => format!(
                        "Codec {:04x}:{:04x} at address {} -- DAC nid {}, pin nid {}",
                        t.vendor_id, t.device_id, t.codec_addr, t.dac_nid, t.pin_nid
                    ),
                    None => String::from("Controller found, but no usable output path"),
                };
                Self {
                    controller: Some(controller),
                    target,
                    tone: None,
                    playing: false,
                    status,
                }
            }
            Err(err) => Self {
                controller: None,
                target: None,
                tone: None,
                playing: false,
                status: format!("No HD Audio controller: {err:?}"),
            },
        }
    }

    fn toggle_play(&mut self) {
        let (Some(controller), Some(target)) = (&mut self.controller, &self.target) else {
            return;
        };

        if self.playing {
            controller.stop_output_stream(STREAM_INDEX);
            self.playing = false;
            return;
        }

        let total_bytes = (TONE_SECONDS * 48_000.0) as usize * 4;
        let mut samples = controller.alloc_dma(total_bytes, 128);
        // SAFETY: `samples` was just allocated with exactly `total_bytes`
        // bytes and nothing else holds a reference to it yet.
        fill_tone(unsafe { samples.as_slice_mut() });

        let half = (total_bytes / 2) as u32;
        let mut bdl = controller.alloc_dma(32, 128);
        write_bdl_entry(&mut bdl, 0, samples.phys_addr, half);
        write_bdl_entry(&mut bdl, 1, samples.phys_addr + half as u64, half);

        configure_and_play(controller, target, &bdl, total_bytes as u32);

        self.tone = Some(ToneResources { samples, bdl });
        self.playing = true;
    }
}

impl Screen for HdaScreen {
    fn title(&self) -> &'static str {
        "Audio"
    }

    fn on_key(&mut self, key: Key) -> Action {
        if let Key::Printable(c) = key
            && c == Char16::try_from('\r').unwrap()
        {
            self.toggle_play();
        }
        Action::Continue
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        let mut text = String::new();
        text.push_str(&self.status);
        text.push('\n');
        if self.target.is_some() {
            text.push_str(if self.playing {
                "\nPlaying a 440 Hz test tone -- Enter to stop."
            } else {
                "\nEnter to play a 440 Hz test tone."
            });
        }

        let block = Block::default()
            .title(" HD Audio ")
            .borders(Borders::ALL)
            .border_set(super::ASCII_BORDER)
            .style(Style::default().fg(Color::Cyan));
        frame.render_widget(Paragraph::new(text).block(block), area);
    }
}

/// Finds the first codec with a widget path this driver knows how to drive:
/// an Audio Function Group, an output-capable Pin Complex under it, and a
/// DAC in that pin's connection list.
fn discover_target(controller: &mut Controller<UefiPlatform>) -> Option<Target> {
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

fn configure_and_play(
    controller: &mut Controller<UefiPlatform>,
    target: &Target,
    bdl: &DmaBuffer,
    total_bytes: u32,
) {
    controller.set_converter_format(target.codec_addr, target.dac_nid, PcmFormat::CD_STEREO);
    controller.set_converter_stream_channel(target.codec_addr, target.dac_nid, STREAM_TAG, 0);
    unmute_at_zero_db(controller, target.codec_addr, target.dac_nid);

    controller.set_pin_widget_control(target.codec_addr, target.pin_nid, true, false, true);
    unmute_at_zero_db(controller, target.codec_addr, target.pin_nid);
    controller.set_eapd(target.codec_addr, target.pin_nid, true);

    let _ = controller.configure_output_stream(
        STREAM_INDEX,
        STREAM_TAG,
        PcmFormat::CD_STEREO,
        bdl,
        1,
        total_bytes,
    );
    controller.start_output_stream(STREAM_INDEX);
}

/// Unmutes `nid`'s output amplifier at its reported 0 dB point. If the amp
/// doesn't report any usable step info (either it has no amp there, or --
/// like QEMU's built-in HDA codec -- it just doesn't implement the
/// Amplifier Capabilities parameter and reports all-zero), leaves the amp
/// alone rather than programming an explicit gain of 0, which would be the
/// minimum (near-silent) setting on hardware that *does* implement it.
fn unmute_at_zero_db(controller: &mut Controller<UefiPlatform>, codec_addr: u8, nid: u8) {
    if let Ok(caps) = controller.output_amp_caps(codec_addr, nid)
        && caps.num_steps > 0
    {
        controller.set_output_amp_gain_mute(codec_addr, nid, false, caps.offset);
    }
}

/// Fills `buf` (48 kHz / 16-bit / stereo, interleaved) with a sine tone.
fn fill_tone(buf: &mut [u8]) {
    for (i, frame) in buf.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let t = i as f32 / 48_000.0;
        let value = (libm::sinf(2.0 * core::f32::consts::PI * TONE_HZ * t) * TONE_AMPLITUDE) as i16;
        let bytes = value.to_le_bytes();
        frame[0..2].copy_from_slice(&bytes);
        frame[2..4].copy_from_slice(&bytes);
    }
}

fn write_bdl_entry(bdl: &mut DmaBuffer, index: usize, addr: u64, length: u32) {
    // SAFETY: `bdl` was allocated with room for at least `index + 1`
    // 16-byte entries by every caller in this file.
    unsafe {
        let entry = bdl.ptr.add(index * 16);
        (entry as *mut u64).write_volatile(addr);
        (entry.add(8) as *mut u32).write_volatile(length);
        (entry.add(12) as *mut u32).write_volatile(0);
    }
}
