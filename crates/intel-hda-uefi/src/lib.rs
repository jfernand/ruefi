//! UEFI boot-services [`intel_hda::Platform`] implementation: finds the
//! Intel HD Audio controller on PCI bus 0, enables it, and maps its
//! registers for the platform-agnostic driver core in `intel-hda`.
#![no_std]

use core::time::Duration;

use intel_hda::{Controller, DmaBuffer, Platform};
use uefi::boot::{
    self, AllocateType, MemoryType, OpenProtocolAttributes, OpenProtocolParams, SearchType,
};
use uefi::proto::pci::PciIoAddress;
use uefi::proto::pci::root_bridge::PciRootBridgeIo;

const HDA_CLASS: u8 = 0x04;
const HDA_SUBCLASS: u8 = 0x03;

/// PCI Command register bits this driver needs set before touching the
/// controller's MMIO registers.
const PCI_COMMAND_MEMORY_SPACE: u16 = 1 << 0;
const PCI_COMMAND_BUS_MASTER: u16 = 1 << 2;

#[derive(Debug)]
pub enum OpenError {
    ControllerNotFound,
    Driver(intel_hda::Error),
}

/// Scans PCI bus 0 for an Intel HD Audio controller, enables it (Memory
/// Space + Bus Master), resets it, and brings up its command channel.
///
/// UEFI applications run with the firmware's identity mapping of physical
/// memory, so a BAR's physical address doubles as a directly-dereferenceable
/// pointer -- this is the assumption [`UefiPlatform`]'s MMIO accessors and
/// [`Platform::alloc_dma`] rely on.
pub fn open() -> Result<Controller<UefiPlatform>, OpenError> {
    let mmio_base = find_and_enable_controller().ok_or(OpenError::ControllerNotFound)?;
    // SAFETY: `mmio_base` is BAR0 of a device we just confirmed is an HDA
    // controller (class 0x04 subclass 0x03) with Memory Space enabled.
    let platform = unsafe { UefiPlatform::new(mmio_base) };
    Controller::new(platform).map_err(OpenError::Driver)
}

pub struct UefiPlatform {
    mmio_base: usize,
}

impl UefiPlatform {
    /// # Safety
    /// `mmio_base` must be the physical address of a mapped, enabled HDA
    /// controller's BAR0, valid for the lifetime of this platform.
    unsafe fn new(mmio_base: usize) -> Self {
        Self { mmio_base }
    }
}

impl Platform for UefiPlatform {
    fn mmio_read32(&self, offset: u32) -> u32 {
        // SAFETY: `offset` is one of the fixed HDA register offsets from
        // `intel_hda::regs`, all within BAR0's 16 KB MMIO region.
        unsafe { ((self.mmio_base + offset as usize) as *const u32).read_volatile() }
    }

    fn mmio_write32(&mut self, offset: u32, value: u32) {
        // SAFETY: see `mmio_read32`.
        unsafe { ((self.mmio_base + offset as usize) as *mut u32).write_volatile(value) }
    }

    fn mmio_read16(&self, offset: u32) -> u16 {
        // SAFETY: see `mmio_read32`.
        unsafe { ((self.mmio_base + offset as usize) as *const u16).read_volatile() }
    }

    fn mmio_write16(&mut self, offset: u32, value: u16) {
        // SAFETY: see `mmio_read32`.
        unsafe { ((self.mmio_base + offset as usize) as *mut u16).write_volatile(value) }
    }

    fn mmio_read8(&self, offset: u32) -> u8 {
        // SAFETY: see `mmio_read32`.
        unsafe { ((self.mmio_base + offset as usize) as *const u8).read_volatile() }
    }

    fn mmio_write8(&mut self, offset: u32, value: u8) {
        // SAFETY: see `mmio_read32`.
        unsafe { ((self.mmio_base + offset as usize) as *mut u8).write_volatile(value) }
    }

    fn delay_us(&self, micros: u32) {
        boot::stall(Duration::from_micros(micros as u64));
    }

    fn alloc_dma(&mut self, len: usize, align: usize) -> DmaBuffer {
        debug_assert!(
            align <= 4096,
            "HDA DMA buffers only need up to 128-byte alignment; page-granularity \
             allocation below satisfies anything up to a full page"
        );
        let pages = len.div_ceil(4096).max(1);
        let ptr = boot::allocate_pages(AllocateType::MaxAddress(0xFFFF_FFFF), MemoryType::LOADER_DATA, pages)
            .expect("HDA DMA allocation failed");
        // SAFETY: `ptr` is a freshly returned allocation of `pages` pages,
        // so `pages * 4096` bytes starting there are valid to write.
        unsafe { core::ptr::write_bytes(ptr.as_ptr(), 0, pages * 4096) };
        DmaBuffer {
            ptr: ptr.as_ptr(),
            phys_addr: ptr.as_ptr() as u64,
            len: pages * 4096,
        }
    }
}

/// Scans PCI bus 0 across every root bridge for a function with class 0x04
/// subclass 0x03 (HD Audio), enables Memory Space + Bus Master on it, and
/// returns its BAR0 base address.
fn find_and_enable_controller() -> Option<usize> {
    let handles = boot::locate_handle_buffer(SearchType::from_proto::<PciRootBridgeIo>()).ok()?;

    for &handle in handles.iter() {
        // SAFETY: we only read/write PCI config space here, never memory
        // the storage/boot driver depends on, so non-exclusive access is
        // safe (same reasoning as ruefi's own read-only PCI explorer).
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

        for device in 0..32u8 {
            for function in 0..8u8 {
                let addr = PciIoAddress::new(0, device, function);
                let pci = root_bridge.pci();

                let Ok(id_reg) = pci.read_one::<u32>(addr.with_register(0x00)) else {
                    continue;
                };
                if (id_reg & 0xFFFF) as u16 == 0xFFFF {
                    if function == 0 {
                        break;
                    }
                    continue;
                }

                let header_reg = pci.read_one::<u32>(addr.with_register(0x0C)).unwrap_or(0);
                let multi_function = (header_reg >> 16) as u8 & 0x80 != 0;

                let class_reg = pci.read_one::<u32>(addr.with_register(0x08)).unwrap_or(0);
                let [_, _, subclass, class] = class_reg.to_le_bytes();
                if class == HDA_CLASS && subclass == HDA_SUBCLASS {
                    let bar0 = pci.read_one::<u32>(addr.with_register(0x10)).unwrap_or(0);
                    let is_64bit = (bar0 >> 1) & 0x3 == 2;
                    let base = if is_64bit {
                        let bar1 = pci.read_one::<u32>(addr.with_register(0x14)).unwrap_or(0);
                        ((bar0 & !0xF) as u64) | ((bar1 as u64) << 32)
                    } else {
                        (bar0 & !0xF) as u64
                    };

                    let command = pci.read_one::<u16>(addr.with_register(0x04)).unwrap_or(0);
                    let _ = pci.write_one::<u16>(
                        addr.with_register(0x04),
                        command | PCI_COMMAND_MEMORY_SPACE | PCI_COMMAND_BUS_MASTER,
                    );

                    return Some(base as usize);
                }

                if function == 0 && !multi_function {
                    break;
                }
            }
        }
    }
    None
}
