//! Block device enumeration and raw sector reads, via the Block I/O
//! protocol.

use alloc::vec;
use alloc::vec::Vec;

use uefi::Handle;
use uefi::boot::{self, OpenProtocolAttributes, OpenProtocolParams, ScopedProtocol, SearchType};
use uefi::proto::media::block::BlockIO;

/// Opens the Block I/O protocol non-exclusively (`GetProtocol`). Using
/// `open_protocol_exclusive` here would force the firmware to disconnect
/// whatever driver currently owns the device with `ByDriver` -- for the
/// disk we booted from, that's the very driver serving our own running
/// image, which hangs. We only read, so non-exclusive access is safe and
/// sufficient.
fn open(handle: Handle) -> uefi::Result<ScopedProtocol<BlockIO>> {
    // SAFETY: we only call read-only methods (`media()`, `read_blocks`)
    // through the interface, so the "no conflicting concurrent access"
    // requirement of `GetProtocol` is satisfied.
    unsafe {
        boot::open_protocol::<BlockIO>(
            OpenProtocolParams {
                handle,
                agent: boot::image_handle(),
                controller: None,
            },
            OpenProtocolAttributes::GetProtocol,
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DiskDevice {
    pub handle: Handle,
    pub media_id: u32,
    pub removable: bool,
    pub logical_partition: bool,
    pub read_only: bool,
    pub block_size: u32,
    pub last_block: u64,
}

impl DiskDevice {
    pub fn size_bytes(&self) -> u64 {
        (self.last_block + 1) * self.block_size as u64
    }
}

/// Lists every device exposing the Block I/O protocol: physical disks,
/// optical drives, and any logical block devices layered on top (e.g. one
/// per partition).
pub fn scan() -> Vec<DiskDevice> {
    let Ok(handles) = boot::locate_handle_buffer(SearchType::from_proto::<BlockIO>()) else {
        return Vec::new();
    };

    handles
        .iter()
        .filter_map(|&handle| {
            let block_io = open(handle).ok()?;
            let media = block_io.media();
            if !media.is_media_present() {
                return None;
            }
            Some(DiskDevice {
                handle,
                media_id: media.media_id(),
                removable: media.is_removable_media(),
                logical_partition: media.is_logical_partition(),
                read_only: media.is_read_only(),
                block_size: media.block_size(),
                last_block: media.last_block(),
            })
        })
        .collect()
}

/// Reads the first block (LBA 0) of a device -- for a whole-disk device
/// this is the MBR/GPT protective header, handy to eyeball as a hex dump.
pub fn read_first_block(device: &DiskDevice) -> Option<Vec<u8>> {
    let block_io = open(device.handle).ok()?;
    let mut buf = vec![0u8; device.block_size as usize];
    block_io.read_blocks(device.media_id, 0, &mut buf).ok()?;
    Some(buf)
}
