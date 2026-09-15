//! Scans this firmware's protocol database against the static catalog in
//! [`super::protocol_catalog`], reporting which UEFI/PI protocols it
//! actually implements.
//!
//! Presence is checked the same way any protocol consumer does it --
//! `boot::locate_handle_buffer(SearchType::ByProtocol(&guid))` -- so this
//! needs only each protocol's GUID, not a full Rust binding for it. That's
//! what makes scanning ~200 protocols this codebase never otherwise touches
//! possible at all.

use alloc::vec::Vec;

use uefi::Guid;
use uefi::boot::{self, SearchType};

use super::protocol_catalog::{self, ProtocolEntry};

pub struct ProtocolStatus {
    pub category: &'static str,
    pub name: &'static str,
    pub guid: Guid,
    pub description: &'static str,
    pub handle_count: usize,
}

impl ProtocolStatus {
    pub fn present(&self) -> bool {
        self.handle_count > 0
    }
}

pub fn scan() -> Vec<ProtocolStatus> {
    protocol_catalog::CATALOG
        .iter()
        .map(|entry: &ProtocolEntry| {
            let handle_count = boot::locate_handle_buffer(SearchType::ByProtocol(&entry.guid))
                .map(|handles| handles.len())
                .unwrap_or(0);
            ProtocolStatus {
                category: entry.category,
                name: entry.name,
                guid: entry.guid,
                description: entry.description,
                handle_count,
            }
        })
        .collect()
}
