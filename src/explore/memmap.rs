//! A snapshot of the UEFI memory map.

use alloc::string::String;
use alloc::vec::Vec;

use uefi::boot::{self, MemoryType};
use uefi::mem::memory_map::MemoryMap;

#[derive(Debug, Clone)]
pub struct MemRegion {
    pub ty: String,
    pub phys_start: u64,
    pub page_count: u64,
}

impl MemRegion {
    pub fn size_bytes(&self) -> u64 {
        self.page_count * 4096
    }
}

/// Takes a snapshot of the current memory map. Note that, per the UEFI
/// spec, taking the snapshot itself may allocate, which is why the map is
/// read out into owned `MemRegion`s immediately rather than held onto --
/// the raw `MemoryMapOwned` is a one-shot view of the map at a single
/// instant, and easy to invalidate by doing anything else that allocates.
pub fn snapshot() -> Vec<MemRegion> {
    let Ok(map) = boot::memory_map(MemoryType::LOADER_DATA) else {
        return Vec::new();
    };

    map.entries()
        .map(|d| MemRegion {
            ty: alloc::format!("{:?}", d.ty),
            phys_start: d.phys_start,
            page_count: d.page_count,
        })
        .collect()
}

/// Groups a snapshot by memory type, summing page counts -- much more
/// useful to look at than several hundred raw descriptors.
pub fn summarize(regions: &[MemRegion]) -> Vec<(String, u64, u64)> {
    let mut totals: Vec<(String, u64, u64)> = Vec::new();
    for region in regions {
        if let Some(entry) = totals.iter_mut().find(|(ty, ..)| *ty == region.ty) {
            entry.1 += region.page_count;
            entry.2 += 1;
        } else {
            totals.push((region.ty.clone(), region.page_count, 1));
        }
    }
    totals.sort_by(|a, b| b.1.cmp(&a.1));
    totals
}
