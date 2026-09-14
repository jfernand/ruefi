//! UEFI (NVRAM) variable enumeration, via the runtime `GetVariable` /
//! `GetNextVariableName` services -- the same store `efibootmgr` or
//! `BootOrder` reads from on a real system.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use uefi::runtime::{self, VariableAttributes, VariableKey, VariableVendor};

#[derive(Debug, Clone)]
pub struct UefiVariable {
    key: VariableKey,
    pub name: String,
    pub vendor: String,
    pub size: usize,
    pub attributes: String,
}

impl UefiVariable {
    /// Re-reads this variable's current raw value from NVRAM.
    pub fn read(&self) -> Option<Vec<u8>> {
        runtime::get_variable_boxed(&self.key.name, &self.key.vendor)
            .ok()
            .map(|(data, _attr)| data.into())
    }
}

fn format_attributes(attrs: VariableAttributes) -> String {
    let mut parts = Vec::new();
    if attrs.contains(VariableAttributes::NON_VOLATILE) {
        parts.push("NV");
    }
    if attrs.contains(VariableAttributes::BOOTSERVICE_ACCESS) {
        parts.push("BS");
    }
    if attrs.contains(VariableAttributes::RUNTIME_ACCESS) {
        parts.push("RT");
    }
    if attrs.contains(VariableAttributes::HARDWARE_ERROR_RECORD) {
        parts.push("HR");
    }
    if parts.is_empty() {
        "-".to_string()
    } else {
        parts.join(" ")
    }
}

/// Lists every UEFI variable currently in NVRAM, across every vendor
/// namespace, with its size and access attributes.
pub fn list() -> Vec<UefiVariable> {
    runtime::variable_keys()
        .filter_map(|k| k.ok())
        .map(|key| {
            let vendor = if key.vendor == VariableVendor::GLOBAL_VARIABLE {
                "Global".to_string()
            } else {
                format!("{}", key.vendor.0)
            };

            let (size, attributes) = runtime::get_variable_boxed(&key.name, &key.vendor)
                .map(|(data, attr)| (data.len(), format_attributes(attr)))
                .unwrap_or((0, "?".to_string()));

            UefiVariable {
                name: key.name.to_string(),
                vendor,
                size,
                attributes,
                key,
            }
        })
        .collect()
}
