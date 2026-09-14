//! Minimal, read-only ACPI table walker.
//!
//! We don't use UEFI's `AcpiTable` protocol -- that one is for *installing*
//! tables, not reading them. Instead we find the RSDP through the system
//! table's configuration table, then walk the XSDT/RSDT ourselves. This
//! works because UEFI applications run before `ExitBootServices` with an
//! identity-mapped address space, so a physical address from a firmware
//! table can be read directly as a pointer.

use alloc::string::String;
use alloc::vec::Vec;
use core::ptr;

use uefi::system;
use uefi::table::cfg::ConfigTableEntry;

/// One ACPI table found in the XSDT/RSDT, plus the top-level RSDP/XSDT
/// entries themselves.
#[derive(Debug, Clone)]
pub struct AcpiTableSummary {
    pub signature: String,
    pub address: u64,
    pub length: u32,
    pub oem_id: String,
}

#[derive(Debug, Clone)]
pub struct AcpiInfo {
    /// ACPI revision (0 = ACPI 1.0 / RSDT only, 2+ = XSDT available).
    pub revision: u8,
    pub tables: Vec<AcpiTableSummary>,
}

#[repr(C, packed)]
struct RawSdtHeader {
    signature: [u8; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32,
}

#[repr(C, packed)]
struct RawRsdp {
    signature: [u8; 8],
    checksum: u8,
    oem_id: [u8; 6],
    revision: u8,
    rsdt_address: u32,
    // ACPI 2.0+ fields:
    length: u32,
    xsdt_address: u64,
    extended_checksum: u8,
    reserved: [u8; 3],
}

fn ascii(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '.' })
        .collect()
}

/// Reads the `RawSdtHeader` at the given physical address and returns a
/// summary. `addr` must point at a valid ACPI table.
unsafe fn read_header(addr: u64) -> AcpiTableSummary {
    // SAFETY: caller guarantees `addr` points at a valid ACPI SDT header,
    // and UEFI pre-ExitBootServices addresses are identity-mapped.
    let header = unsafe { ptr::read_unaligned(addr as *const RawSdtHeader) };
    AcpiTableSummary {
        signature: ascii(&header.signature),
        address: addr,
        length: header.length,
        oem_id: ascii(&header.oem_id),
    }
}

/// Locates the RSDP via the UEFI configuration table and walks the
/// XSDT (or RSDT, for ACPI 1.0 firmware) to list every top-level table.
/// Returns `None` if no ACPI configuration table entry is present at all.
pub fn discover() -> Option<AcpiInfo> {
    let rsdp_addr = system::with_config_table(|entries| {
        entries
            .iter()
            .find(|e| e.guid == ConfigTableEntry::ACPI2_GUID)
            .or_else(|| entries.iter().find(|e| e.guid == ConfigTableEntry::ACPI_GUID))
            .map(|e| e.address as u64)
    })?;

    // SAFETY: `rsdp_addr` comes from the firmware's own configuration
    // table, so it points at a valid RSDP.
    let rsdp = unsafe { ptr::read_unaligned(rsdp_addr as *const RawRsdp) };

    let mut tables = Vec::new();

    if rsdp.revision >= 2 && rsdp.xsdt_address != 0 {
        // SAFETY: xsdt_address points at a valid XSDT per the RSDP.
        let xsdt_header = unsafe { ptr::read_unaligned(rsdp.xsdt_address as *const RawSdtHeader) };
        let entry_count = (xsdt_header.length as usize - size_of::<RawSdtHeader>()) / 8;
        let entries_addr = rsdp.xsdt_address + size_of::<RawSdtHeader>() as u64;
        for i in 0..entry_count {
            // SAFETY: within the bounds of the XSDT entry array.
            let table_addr =
                unsafe { ptr::read_unaligned((entries_addr + i as u64 * 8) as *const u64) };
            if table_addr != 0 {
                // SAFETY: entry points at a valid ACPI table per the XSDT.
                tables.push(unsafe { read_header(table_addr) });
            }
        }
    } else if rsdp.rsdt_address != 0 {
        // SAFETY: rsdt_address points at a valid RSDT per the RSDP.
        let rsdt_header =
            unsafe { ptr::read_unaligned(rsdp.rsdt_address as u64 as *const RawSdtHeader) };
        let entry_count = (rsdt_header.length as usize - size_of::<RawSdtHeader>()) / 4;
        let entries_addr = rsdp.rsdt_address as u64 + size_of::<RawSdtHeader>() as u64;
        for i in 0..entry_count {
            // SAFETY: within the bounds of the RSDT entry array.
            let table_addr =
                unsafe { ptr::read_unaligned((entries_addr + i as u64 * 4) as *const u32) } as u64;
            if table_addr != 0 {
                // SAFETY: entry points at a valid ACPI table per the RSDT.
                tables.push(unsafe { read_header(table_addr) });
            }
        }
    }

    Some(AcpiInfo {
        revision: rsdp.revision,
        tables,
    })
}
