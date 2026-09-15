//! The HDA controller itself: reset/bring-up, the CORB/RIRB command
//! channel, codec/widget discovery, and stream descriptor programming.

use crate::codec::{AmpCaps, PinCaps, WidgetCaps};
use crate::format::PcmFormat;
use crate::platform::{DmaBuffer, Platform};
use crate::regs::{self, Gcap};
use crate::verbs::{self, param_id};

#[derive(Debug, Clone, Copy)]
pub enum Error {
    /// GCTL.CRST didn't reach the expected value in time.
    ControllerResetTimeout,
    /// A stream's SRST didn't reach the expected value in time.
    StreamResetTimeout,
    /// Sent a verb but never saw a matching RIRB response.
    CommandTimeout,
}

const RESET_POLL_ITERATIONS: u32 = 10_000;
const RESET_POLL_DELAY_US: u32 = 10;
const COMMAND_POLL_ITERATIONS: u32 = 20_000;
const COMMAND_POLL_DELAY_US: u32 = 5;

/// 256 entries is the largest (and simplest to just always pick) CORB/RIRB
/// size every controller is required to support the *capability* bit for;
/// software still has to confirm the size is actually selectable, which
/// [`Controller::new`] does before committing to it.
const RING_ENTRIES: usize = 256;

pub struct Controller<P: Platform> {
    platform: P,
    gcap: Gcap,
    corb: DmaBuffer,
    rirb: DmaBuffer,
    /// Last CORB index we wrote a command to.
    corb_wp: u8,
    /// Last RIRB index we've consumed a response from.
    rirb_rp: u8,
}

impl<P: Platform> Controller<P> {
    /// Takes a platform with the HDA controller's PCI command register
    /// already configured (Memory Space + Bus Master enabled) and its BAR0
    /// already resolved to whatever base `platform`'s MMIO accessors use,
    /// resets the controller, and brings up the CORB/RIRB command channel.
    pub fn new(mut platform: P) -> Result<Self, Error> {
        let gcap = Gcap::decode(platform.mmio_read16(regs::GCAP));

        reset_controller(&mut platform)?;

        let corb = platform.alloc_dma(RING_ENTRIES * 4, 128);
        let rirb = platform.alloc_dma(RING_ENTRIES * 8, 128);

        let mut controller = Self {
            platform,
            gcap,
            corb,
            rirb,
            corb_wp: 0,
            rirb_rp: 0,
        };
        controller.init_corb();
        controller.init_rirb();
        Ok(controller)
    }

    /// Bitmap from STATESTS: bit `n` set means a codec responded on SDIN
    /// `n` during link bring-up.
    pub fn codec_presence(&self) -> u16 {
        self.platform.mmio_read16(regs::STATESTS) & 0x7FFF
    }

    /// Allocates a DMA-capable buffer through this controller's platform --
    /// for sample data and buffer descriptor lists a caller builds itself.
    pub fn alloc_dma(&mut self, len: usize, align: usize) -> DmaBuffer {
        self.platform.alloc_dma(len, align)
    }

    fn init_corb(&mut self) {
        let p = &mut self.platform;
        p.mmio_write8(regs::CORBCTL, 0);
        p.mmio_write32(regs::CORBLBASE, self.corb.phys_addr as u32);
        p.mmio_write32(regs::CORBUBASE, (self.corb.phys_addr >> 32) as u32);
        p.mmio_write8(regs::CORBSIZE, 0b10); // 256 entries
        p.mmio_write16(regs::CORBRP, regs::corbrp::RESET);
        p.mmio_write16(regs::CORBRP, 0);
        p.mmio_write16(regs::CORBWP, 0);
        p.mmio_write8(regs::CORBCTL, regs::corbctl::CORBRUN);
        self.corb_wp = 0;
    }

    fn init_rirb(&mut self) {
        let p = &mut self.platform;
        p.mmio_write8(regs::RIRBCTL, 0);
        p.mmio_write32(regs::RIRBLBASE, self.rirb.phys_addr as u32);
        p.mmio_write32(regs::RIRBUBASE, (self.rirb.phys_addr >> 32) as u32);
        p.mmio_write8(regs::RIRBSIZE, 0b10); // 256 entries
        p.mmio_write16(regs::RIRBWP, regs::rirbwp::RESET);
        // A response-interrupt count of 0 would (per spec) mean "256", but
        // some controller emulations (QEMU's ich9-intel-hda included) treat
        // it as a literal 0 and never fetch a single CORB command until a
        // nonzero count is programmed. We don't use the resulting interrupt
        // (`send_verb` polls instead), so any nonzero value works; picking
        // 1 and clearing RIRBSTS after every response (see `send_verb`)
        // keeps the count from ever gating command processing.
        p.mmio_write16(regs::RINTCNT, 1);
        // RINTCTL (interrupt enable) doesn't route anywhere we use -- we
        // poll RIRBWP instead of handling interrupts -- but some emulated
        // controllers (see the RINTCNT comment above) only ever clear
        // RIRBSTS.RINTFL's underlying counter on a 1-to-0 transition of
        // that bit, which requires it to have actually been set. Enabling
        // it here costs nothing and keeps `send_verb`'s per-response clear
        // effective indefinitely rather than for only the first RINTCNT
        // responses.
        p.mmio_write8(
            regs::RIRBCTL,
            regs::rirbctl::RIRBDMAEN | regs::rirbctl::RINTCTL,
        );
        self.rirb_rp = 0;
    }

    /// Sends one verb to `nid` on the codec at `codec_addr` and returns its
    /// 32-bit response. `payload` is a pre-built 20-bit field from
    /// [`crate::verbs::Verb8::payload`] or [`crate::verbs::Verb16::payload`].
    ///
    /// Assumes GCTL.UNSOL is left at its reset value of 0 (unsolicited
    /// responses dropped by the controller), which this driver never
    /// changes -- so every RIRB entry is a solicited response to exactly
    /// the command we most recently sent, in order.
    pub fn send_verb(&mut self, codec_addr: u8, nid: u8, payload: u32) -> Result<u32, Error> {
        let cmd = verbs::command(codec_addr, nid, payload);

        let next_wp = self.corb_wp.wrapping_add(1);
        // SAFETY: `corb` is `RING_ENTRIES * 4` bytes, `next_wp` is a u8 so
        // this offset is always within bounds.
        unsafe {
            let entry = self.corb.ptr.add(next_wp as usize * 4) as *mut u32;
            entry.write_volatile(cmd);
        }
        self.corb_wp = next_wp;
        self.platform
            .mmio_write16(regs::CORBWP, self.corb_wp as u16);

        for _ in 0..COMMAND_POLL_ITERATIONS {
            let wp = (self.platform.mmio_read16(regs::RIRBWP) & 0xFF) as u8;
            if wp != self.rirb_rp {
                let next_rp = self.rirb_rp.wrapping_add(1);
                // SAFETY: `rirb` is `RING_ENTRIES * 8` bytes, `next_rp` is a
                // u8 so this offset is always within bounds.
                let response = unsafe {
                    let entry = self.rirb.ptr.add(next_rp as usize * 8) as *const u32;
                    entry.read_volatile()
                };
                self.rirb_rp = next_rp;
                // Write-1-to-clear: standard status-bit clear on real
                // hardware, and also what resets the emulated
                // "responses since last service" counter that RINTCNT
                // gates against (see `init_rirb`).
                self.platform
                    .mmio_write8(regs::RIRBSTS, regs::rirbsts::RINTFL);
                return Ok(response);
            }
            self.platform.delay_us(COMMAND_POLL_DELAY_US);
        }
        Err(Error::CommandTimeout)
    }

    pub fn get_parameter(&mut self, codec_addr: u8, nid: u8, param: u8) -> Result<u32, Error> {
        self.send_verb(codec_addr, nid, verbs::GET_PARAMETER.payload(param))
    }

    pub fn vendor_device_id(&mut self, codec_addr: u8) -> Result<(u16, u16), Error> {
        let raw = self.get_parameter(codec_addr, verbs::ROOT_NODE, param_id::VENDOR_ID)?;
        Ok(((raw >> 16) as u16, raw as u16))
    }

    /// `(starting_nid, count)` of a node's direct children.
    pub fn subordinate_nodes(&mut self, codec_addr: u8, nid: u8) -> Result<(u8, u8), Error> {
        let raw = self.get_parameter(codec_addr, nid, param_id::SUBORDINATE_NODE_COUNT)?;
        Ok(((raw >> 16) as u8, raw as u8))
    }

    /// Finds the first Audio Function Group under the codec's root node.
    pub fn find_audio_function_group(&mut self, codec_addr: u8) -> Result<Option<u8>, Error> {
        let (start, count) = self.subordinate_nodes(codec_addr, verbs::ROOT_NODE)?;
        for nid in start..start.saturating_add(count) {
            let raw = self.get_parameter(codec_addr, nid, param_id::FUNCTION_GROUP_TYPE)?;
            if (raw as u8) == verbs::NODE_TYPE_AUDIO_FUNCTION_GROUP {
                return Ok(Some(nid));
            }
        }
        Ok(None)
    }

    pub fn widget_caps(&mut self, codec_addr: u8, nid: u8) -> Result<WidgetCaps, Error> {
        let raw = self.get_parameter(codec_addr, nid, param_id::AUDIO_WIDGET_CAPS)?;
        Ok(WidgetCaps::decode(raw))
    }

    pub fn pin_caps(&mut self, codec_addr: u8, nid: u8) -> Result<PinCaps, Error> {
        let raw = self.get_parameter(codec_addr, nid, param_id::PIN_CAPS)?;
        Ok(PinCaps::decode(raw))
    }

    pub fn output_amp_caps(&mut self, codec_addr: u8, nid: u8) -> Result<AmpCaps, Error> {
        let raw = self.get_parameter(codec_addr, nid, param_id::OUTPUT_AMP_CAPS)?;
        Ok(AmpCaps::decode(raw))
    }

    /// Walks every widget directly under the Audio Function Group `afg_nid`,
    /// calling `f(nid, caps)` for each. No allocation: the caller decides
    /// whether/how to remember what it finds.
    pub fn for_each_widget(
        &mut self,
        codec_addr: u8,
        afg_nid: u8,
        mut f: impl FnMut(&mut Self, u8, WidgetCaps),
    ) -> Result<(), Error> {
        let (start, count) = self.subordinate_nodes(codec_addr, afg_nid)?;
        for nid in start..start.saturating_add(count) {
            let caps = self.widget_caps(codec_addr, nid)?;
            f(self, nid, caps);
        }
        Ok(())
    }

    /// Fills `out` with up to `out.len()` connection-list entries (node
    /// IDs) for widget `nid`, returning how many the widget actually
    /// reports (which may exceed `out.len()`).
    pub fn connection_list(
        &mut self,
        codec_addr: u8,
        nid: u8,
        out: &mut [u8],
    ) -> Result<usize, Error> {
        let raw = self.get_parameter(codec_addr, nid, param_id::CONNECTION_LIST_LENGTH)?;
        let long_form = raw & (1 << 7) != 0;
        let length = (raw & 0x7F) as usize;
        let per_entry = if long_form { 2 } else { 4 };

        let mut index = 0usize;
        while index < length {
            let raw = self.send_verb(
                codec_addr,
                nid,
                verbs::GET_CONNECTION_LIST_ENTRY.payload(index as u8),
            )?;
            for slot in 0..per_entry {
                let list_pos = index + slot;
                if list_pos >= length {
                    break;
                }
                if let Some(dst) = out.get_mut(list_pos) {
                    *dst = if long_form {
                        (raw >> (slot * 16)) as u8
                    } else {
                        (raw >> (slot * 8)) as u8
                    };
                }
            }
            index += per_entry;
        }
        Ok(length)
    }

    pub fn set_converter_format(&mut self, codec_addr: u8, nid: u8, format: PcmFormat) {
        if let Some(encoded) = format.encode() {
            let _ = self.send_verb(
                codec_addr,
                nid,
                verbs::SET_CONVERTER_FORMAT.payload(encoded),
            );
        }
    }

    pub fn set_converter_stream_channel(
        &mut self,
        codec_addr: u8,
        nid: u8,
        stream_tag: u8,
        channel: u8,
    ) {
        let payload = ((stream_tag & 0xF) << 4) | (channel & 0xF);
        let _ = self.send_verb(
            codec_addr,
            nid,
            verbs::SET_CONVERTER_STREAM_CHANNEL.payload(payload),
        );
    }

    pub fn set_pin_widget_control(
        &mut self,
        codec_addr: u8,
        nid: u8,
        out_enable: bool,
        in_enable: bool,
        headphone_enable: bool,
    ) {
        let mut payload = 0u8;
        if headphone_enable {
            payload |= 1 << 7;
        }
        if out_enable {
            payload |= 1 << 6;
        }
        if in_enable {
            payload |= 1 << 5;
        }
        let _ = self.send_verb(
            codec_addr,
            nid,
            verbs::SET_PIN_WIDGET_CONTROL.payload(payload),
        );
    }

    /// Sets the output amplifier's gain/mute on both channels at once
    /// (there is no per-input index for a widget's own output amp).
    pub fn set_output_amp_gain_mute(&mut self, codec_addr: u8, nid: u8, mute: bool, gain: u8) {
        let mut payload: u16 = (1 << 15) | (1 << 13) | (1 << 12); // output, left, right
        if mute {
            payload |= 1 << 7;
        }
        payload |= (gain & 0x7F) as u16;
        let _ = self.send_verb(codec_addr, nid, verbs::SET_AMP_GAIN_MUTE.payload(payload));
    }

    pub fn set_power_state(&mut self, codec_addr: u8, nid: u8, state: u8) {
        let _ = self.send_verb(
            codec_addr,
            nid,
            verbs::SET_POWER_STATE.payload(state & 0xF),
        );
    }

    pub fn set_eapd(&mut self, codec_addr: u8, nid: u8, enable: bool) {
        let _ = self.send_verb(
            codec_addr,
            nid,
            verbs::SET_EAPD_BTL.payload(if enable { 1 << 1 } else { 0 }),
        );
    }

    /// Resets output stream `index` (0-based among output streams), sets
    /// its format/stream-tag/buffer-descriptor-list, and leaves it stopped
    /// (RUN = 0) -- call [`Self::start_output_stream`] once the codec side
    /// is also configured.
    pub fn configure_output_stream(
        &mut self,
        index: u8,
        stream_tag: u8,
        format: PcmFormat,
        bdl: &DmaBuffer,
        last_valid_index: u16,
        cyclic_buffer_len: u32,
    ) -> Result<(), Error> {
        let Some(encoded_format) = format.encode() else {
            return Ok(());
        };
        let base = self.gcap.output_stream_base(index);
        self.reset_stream(base)?;

        let p = &mut self.platform;
        p.mmio_write32(base + regs::sd::BDPL, bdl.phys_addr as u32);
        p.mmio_write32(base + regs::sd::BDPU, (bdl.phys_addr >> 32) as u32);
        p.mmio_write32(base + regs::sd::CBL, cyclic_buffer_len);
        p.mmio_write16(base + regs::sd::LVI, last_valid_index);
        p.mmio_write16(base + regs::sd::FMT, encoded_format);
        p.mmio_write32(base + regs::sd::CTL_STS, regs::sdctl::with_stream_tag(stream_tag));
        Ok(())
    }

    pub fn start_output_stream(&mut self, index: u8) {
        let base = self.gcap.output_stream_base(index);
        let ctl = self.platform.mmio_read32(base + regs::sd::CTL_STS);
        self.platform
            .mmio_write32(base + regs::sd::CTL_STS, ctl | regs::sdctl::RUN);
    }

    pub fn stop_output_stream(&mut self, index: u8) {
        let base = self.gcap.output_stream_base(index);
        let ctl = self.platform.mmio_read32(base + regs::sd::CTL_STS);
        self.platform
            .mmio_write32(base + regs::sd::CTL_STS, ctl & !regs::sdctl::RUN);
    }

    fn reset_stream(&mut self, base: u32) -> Result<(), Error> {
        let p = &mut self.platform;
        p.mmio_write32(base + regs::sd::CTL_STS, regs::sdctl::SRST);
        for _ in 0..RESET_POLL_ITERATIONS {
            if p.mmio_read32(base + regs::sd::CTL_STS) & regs::sdctl::SRST != 0 {
                break;
            }
            p.delay_us(RESET_POLL_DELAY_US);
        }
        p.mmio_write32(base + regs::sd::CTL_STS, 0);
        for _ in 0..RESET_POLL_ITERATIONS {
            if p.mmio_read32(base + regs::sd::CTL_STS) & regs::sdctl::SRST == 0 {
                return Ok(());
            }
            p.delay_us(RESET_POLL_DELAY_US);
        }
        Err(Error::StreamResetTimeout)
    }
}

fn reset_controller<P: Platform>(platform: &mut P) -> Result<(), Error> {
    platform.mmio_write32(regs::GCTL, 0);
    for _ in 0..RESET_POLL_ITERATIONS {
        if platform.mmio_read32(regs::GCTL) & regs::gctl::CRST == 0 {
            break;
        }
        platform.delay_us(RESET_POLL_DELAY_US);
    }

    platform.mmio_write32(regs::GCTL, regs::gctl::CRST);
    for _ in 0..RESET_POLL_ITERATIONS {
        if platform.mmio_read32(regs::GCTL) & regs::gctl::CRST != 0 {
            // Codecs may take up to 25 frames (~521us) to request their
            // addresses after the link comes out of reset.
            platform.delay_us(600);
            return Ok(());
        }
        platform.delay_us(RESET_POLL_DELAY_US);
    }
    Err(Error::ControllerResetTimeout)
}
