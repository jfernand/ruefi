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
    /// Not shown by the PCI screen yet, but kept for a possible future
    /// detail view (e.g. distinguishing an AHCI vs. IDE-native-mode SATA
    /// controller, which differ only in `prog_if`).
    #[allow(dead_code)]
    pub prog_if: u8,
    pub revision: u8,
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
                    });

                    if function == 0 {
                        // Only multi-function devices have anything beyond
                        // function 0; check the header type bit to decide
                        // whether to keep scanning functions 1-7.
                        let header_reg = pci
                            .read_one::<u32>(addr.with_register(0x0C))
                            .unwrap_or(0);
                        let header_type = (header_reg >> 16) as u8;
                        if header_type & 0x80 == 0 {
                            break;
                        }
                    }
                }
            }
        }
    }

    devices
}
