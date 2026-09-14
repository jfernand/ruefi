//! A minimal PCI bus scanner built on the PCI Root Bridge I/O protocol.

use alloc::vec::Vec;

use uefi::boot::{self, OpenProtocolAttributes, OpenProtocolParams, SearchType};
use uefi::proto::pci::PciIoAddress;
use uefi::proto::pci::root_bridge::PciRootBridgeIo;

#[derive(Debug, Clone, Copy)]
pub struct PciDevice {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class: u8,
    pub subclass: u8,
    pub prog_if: u8,
    pub revision: u8,
    /// Bits 0-6 identify the config space layout (0 = normal device, 1 =
    /// PCI-to-PCI bridge, 2 = CardBus bridge); bit 7 marks a multi-function
    /// device. Only header type 0 has the "6 general-purpose BARs" layout
    /// that [`Self::bars`] assumes.
    pub header_type: u8,
    /// Raw Base Address Registers 0-5 (config space offsets 0x10-0x24),
    /// meaningful only when `header_type & 0x7f == 0`.
    pub bars: [u32; 6],
}

impl PciDevice {
    /// A short human-readable guess at the device class, based on the PCI
    /// class code. Not exhaustive -- just the common ones you'd expect to
    /// see in a VM or on a typical PC.
    pub fn class_name(&self) -> &'static str {
        match (self.class, self.subclass) {
            (0x01, 0x01) => "IDE controller",
            (0x01, 0x06) => "SATA controller",
            (0x01, 0x08) => "NVMe controller",
            (0x01, _) => "Storage controller",
            (0x02, 0x00) => "Ethernet controller",
            (0x02, _) => "Network controller",
            (0x03, _) => "Display controller",
            (0x04, _) => "Multimedia controller",
            (0x06, 0x00) => "Host bridge",
            (0x06, 0x01) => "ISA bridge",
            (0x06, 0x04) => "PCI-to-PCI bridge",
            (0x06, _) => "Bridge",
            (0x0c, 0x03) => "USB controller",
            (0x0c, 0x05) => "SMBus controller",
            (0x0c, _) => "Serial bus controller",
            _ => "Unknown",
        }
    }

    /// A human-readable decode of BAR `index` (0-5), per the PCI spec's Base
    /// Address Register format. Returns `None` if the register is unused
    /// (value 0) or this device doesn't have the type-0 BAR layout.
    pub fn decode_bar(&self, index: usize) -> Option<alloc::string::String> {
        use alloc::format;

        if self.header_type & 0x7f != 0 {
            return None;
        }
        let raw = *self.bars.get(index)?;
        if raw == 0 {
            return None;
        }

        Some(if raw & 0x1 == 1 {
            format!("I/O, base {:#06x}", raw & !0x3)
        } else {
            let kind = match (raw >> 1) & 0x3 {
                0 => "32-bit",
                2 => "64-bit",
                _ => "reserved-width",
            };
            let prefetchable = if raw & 0x8 != 0 {
                "prefetchable"
            } else {
                "non-prefetchable"
            };
            format!("Memory, {kind}, {prefetchable}, base {:#010x}", raw & !0xf)
        })
    }
}

/// Scans PCI bus 0 (and any bridges' secondary buses are *not* followed --
/// this is a flat, single-segment, bus-0 scan, which is enough to see every
/// device in a typical QEMU machine) across all root bridges the firmware
/// exposes, and returns every function that responds.
pub fn scan() -> Vec<PciDevice> {
    let mut devices = Vec::new();

    let Ok(handles) = boot::locate_handle_buffer(SearchType::from_proto::<PciRootBridgeIo>())
    else {
        return devices;
    };

    for &handle in handles.iter() {
        // Open with `GetProtocol` rather than `open_protocol_exclusive`:
        // exclusive access forces the firmware to disconnect any driver
        // currently bound to this root bridge with `ByDriver` -- which, on
        // real firmware, is the PCI bus driver (and transitively, the
        // storage driver serving the very media we booted from), and can
        // hang. We're only reading config space, so non-exclusive access
        // is both sufficient and much safer.
        //
        // SAFETY: we only read config space through the interface, so the
        // "no conflicting concurrent access" requirement of `GetProtocol`
        // is satisfied.
        let Ok(mut root_bridge) = (unsafe {
            boot::open_protocol::<PciRootBridgeIo>(
                OpenProtocolParams {
                    handle,
                    agent: boot::image_handle(),
                    controller: None,
                },
                OpenProtocolAttributes::GetProtocol,
            )
        }) else {
            continue;
        };

        for bus in 0..=0u8 {
            for device in 0..32u8 {
                for function in 0..8u8 {
                    let addr = PciIoAddress::new(bus, device, function);
                    let pci = root_bridge.pci();

                    let Ok(id_reg) = pci.read_one::<u32>(addr.with_register(0x00)) else {
                        continue;
                    };
                    let vendor_id = (id_reg & 0xFFFF) as u16;
                    if vendor_id == 0xFFFF {
                        // No device here.
                        if function == 0 {
                            break;
                        }
                        continue;
                    }
                    let device_id = (id_reg >> 16) as u16;

                    let class_reg = pci
                        .read_one::<u32>(addr.with_register(0x08))
                        .unwrap_or(0);
                    let [revision, prog_if, subclass, class] = class_reg.to_le_bytes();

                    let header_reg = pci
                        .read_one::<u32>(addr.with_register(0x0C))
                        .unwrap_or(0);
                    let header_type = (header_reg >> 16) as u8;

                    let mut bars = [0u32; 6];
                    if header_type & 0x7f == 0 {
                        for (i, bar) in bars.iter_mut().enumerate() {
                            *bar = pci
                                .read_one::<u32>(addr.with_register(0x10 + (i as u8) * 4))
                                .unwrap_or(0);
                        }
                    }

                    devices.push(PciDevice {
                        bus,
                        device,
                        function,
                        vendor_id,
                        device_id,
                        class,
                        subclass,
                        prog_if,
                        revision,
                        header_type,
                        bars,
                    });

                    if function == 0 && header_type & 0x80 == 0 {
                        // Only multi-function devices have anything beyond
                        // function 0.
                        break;
                    }
                }
            }
        }
    }

    devices
}
