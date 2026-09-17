//! Audio tab: opens the Intel HD Audio controller (if any), finds a codec
//! output path, and can play a test tone through it.

use alloc::format;
use alloc::string::String;

use intel_hda::{Controller, DmaBuffer, PcmFormat, Target};
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
                let target = intel_hda::find_output_target(&mut controller);
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
        // SAFETY: `bdl` was just allocated with room for 2 16-byte entries.
        unsafe {
            bdl.write_bdl_entry(0, samples.phys_addr, half);
            bdl.write_bdl_entry(1, samples.phys_addr + half as u64, half);
        }

        let _ = intel_hda::configure_and_play_output(
            controller,
            target,
            &bdl,
            intel_hda::StreamPlan {
                index: STREAM_INDEX,
                tag: STREAM_TAG,
                format: PcmFormat::CD_STEREO,
                last_valid_index: 1,
                cyclic_buffer_len: total_bytes as u32,
            },
        );

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
