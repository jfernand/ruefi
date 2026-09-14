//! Short descriptions for the "globally defined variables" from the UEFI
//! specification (the `8be4df61-93ca-11d2-aa0d-00e098032b8c` namespace --
//! `VariableVendor::GLOBAL_VARIABLE`). These are the ones every UEFI
//! implementation shares meaning for; anything outside this namespace is
//! vendor-specific and we don't try to guess what it means.

/// Looks up a short, human description for a well-known global variable
/// name. Returns `None` for anything not in the global namespace, or not
/// a name we recognize (including vendor-specific variables, which have
/// no standard meaning to describe).
pub fn describe(name: &str, is_global: bool) -> Option<&'static str> {
    if !is_global {
        return None;
    }

    if let Some(desc) = named(name) {
        return Some(desc);
    }

    if is_hex_suffixed(name, "Boot") {
        return Some("A boot load option: one entry in the firmware's boot menu.");
    }
    if is_hex_suffixed(name, "Driver") {
        return Some("A driver load option: a UEFI driver the firmware can load at boot.");
    }
    if is_hex_suffixed(name, "SysPrep") {
        return Some("A system preparation application to run before the boot manager.");
    }
    if name.starts_with("Capsule") {
        return Some("State for the UEFI capsule update mechanism (firmware updates).");
    }

    None
}

fn named(name: &str) -> Option<&'static str> {
    Some(match name {
        "BootOrder" => {
            "Ordered list of Boot#### option numbers: the firmware boot menu's order."
        }
        "BootNext" => "One-time override: the Boot#### option to try on the very next boot only.",
        "BootCurrent" => "The Boot#### option number that was used for the current boot.",
        "BootOptionSupport" => {
            "Bitmask of boot option features the firmware supports (app boot, network boot, ...)."
        }
        "DriverOrder" => "Ordered list of Driver#### option numbers: UEFI driver load order.",
        "Timeout" => "Boot manager timeout, in seconds, before the default option is launched.",
        "ConIn" => "Device path of the active console input device.",
        "ConOut" => "Device path of the active console output device.",
        "ErrOut" => "Device path of the active console error-output device.",
        "ConInDev" => "Device paths of every console input device the firmware found.",
        "ConOutDev" => "Device paths of every console output device the firmware found.",
        "ErrOutDev" => "Device paths of every console error-output device the firmware found.",
        "Lang" => {
            "Deprecated: current platform language as an ISO 639-2 code (see PlatformLang)."
        }
        "LangCodes" => {
            "Deprecated: supported ISO 639-2 language codes (see PlatformLangCodes)."
        }
        "PlatformLang" => "Current platform language, as an RFC 4646 language tag.",
        "PlatformLangCodes" => "Language tags the platform firmware's UI supports (RFC 4646).",
        "SecureBoot" => {
            "Read-only: 1 if Secure Boot signature verification is currently enforced."
        }
        "SetupMode" => {
            "Read-only: 1 if no Platform Key is enrolled, so image verification isn't enforced."
        }
        "AuditMode" => "1 if in audit mode: signature checks are logged, but not enforced.",
        "DeployedMode" => "1 if in deployed mode: a locked-down Secure Boot state per spec.",
        "PK" => "The Platform Key: root of trust for Secure Boot key management.",
        "KEK" => "Key Exchange Key database: authorizes updates to db/dbx.",
        "db" => "Authorized signature database for Secure Boot image verification.",
        "dbx" => "Forbidden signature database: revoked keys/hashes Secure Boot must reject.",
        "dbt" => "Timestamp signature database, used to validate time-stamping certificates.",
        "dbr" => "Recovery signature database, used during OS recovery boot paths.",
        "VendorKeys" => "Read-only: 1 if only firmware-default Secure Boot keys are enrolled.",
        "OsIndications" => {
            "OS-set requests to firmware for the next boot (e.g. \"boot to firmware UI\")."
        }
        "OsIndicationsSupported" => "Bitmask of OsIndications features this firmware supports.",
        "HwErrRecSupport" => "Level of hardware error record persistence the firmware supports.",
        "MemoryTypeInformation" => {
            "Hints on how much memory of each UEFI memory type to reserve next boot."
        }
        "SignatureSupport" => "Signature types the firmware supports in db/dbx entries.",
        _ => return None,
    })
}

/// True if `name` is `prefix` followed by exactly 4 hex digits (the
/// `Boot####` / `Driver####` / `SysPrep####` naming pattern).
fn is_hex_suffixed(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix)
        .is_some_and(|rest| rest.len() == 4 && rest.chars().all(|c| c.is_ascii_hexdigit()))
}
