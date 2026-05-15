// SPDX-License-Identifier: MIT
use std::fs;
use std::path::{Path, PathBuf};

pub fn read_str(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path.as_ref())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn read_u64(path: impl AsRef<Path>) -> Option<u64> {
    read_str(path).and_then(|s| s.parse().ok())
}

pub fn read_i32(path: impl AsRef<Path>) -> Option<i32> {
    read_str(path).and_then(|s| s.parse().ok())
}

pub fn read_hex_u32(path: impl AsRef<Path>) -> Option<u32> {
    read_str(path).and_then(|s| u32::from_str_radix(s.trim_start_matches("0x"), 16).ok())
}

pub fn read_link(path: impl AsRef<Path>) -> Option<PathBuf> {
    fs::read_link(path.as_ref()).ok()
}

pub fn resolve_link(path: impl AsRef<Path>) -> Option<PathBuf> {
    fs::canonicalize(path.as_ref()).ok()
}

pub fn list_dir(path: impl AsRef<Path>) -> Vec<String> {
    fs::read_dir(path.as_ref())
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect()
}

pub fn list_dir_sorted(path: impl AsRef<Path>) -> Vec<String> {
    let mut v = list_dir(path);
    v.sort();
    v
}

pub fn format_size(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    const GIB: u64 = 1024 * MIB;
    const TIB: u64 = 1024 * GIB;
    if bytes >= TIB {
        format!("{:.1} TiB", bytes as f64 / TIB as f64)
    } else if bytes >= GIB {
        format!("{:.1} GiB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{} B", bytes)
    }
}

pub fn scsi_type_name(t: u32) -> &'static str {
    match t {
        0 => "disk",
        1 => "tape",
        2 => "printer",
        3 => "processor",
        4 => "worm",
        5 => "cd/dvd",
        6 => "scanner",
        7 => "optical",
        8 => "media changer",
        9 => "communications",
        12 => "storage array",
        13 => "enclosure",
        14 => "simplified disk",
        _ => "unknown",
    }
}

/// Validate that a string looks like a PCI address: XXXX:XX:XX.X
pub fn is_pci_address(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() < 10 {
        return false;
    }
    // Check pattern: XXXX:XX:XX.X (minimum), could be longer domain
    // Format: domain:bus:device.function
    // domain is 4+ hex digits, bus is 2 hex, dev is 2 hex, func is 1 hex
    let parts: Vec<&str> = s.splitn(3, ':').collect();
    if parts.len() != 3 {
        return false;
    }
    // Domain: 4+ hex chars
    if parts[0].len() < 4 || !parts[0].chars().all(|c| c.is_ascii_hexdigit()) {
        return false;
    }
    // Bus: 2 hex chars
    if parts[1].len() != 2 || !parts[1].chars().all(|c| c.is_ascii_hexdigit()) {
        return false;
    }
    // device.function: XX.X
    let dev_fn: Vec<&str> = parts[2].splitn(2, '.').collect();
    if dev_fn.len() != 2 {
        return false;
    }
    if dev_fn[0].len() != 2 || !dev_fn[0].chars().all(|c| c.is_ascii_hexdigit()) {
        return false;
    }
    if dev_fn[1].is_empty() || !dev_fn[1].chars().all(|c| c.is_ascii_hexdigit()) {
        return false;
    }
    true
}

/// Get the driver name from a sysfs device path by reading the `driver` symlink.
pub fn read_driver(device_path: impl AsRef<Path>) -> Option<String> {
    let driver_link = device_path.as_ref().join("driver");
    read_link(&driver_link).and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
}

/// Walk up a canonical sysfs path and collect PCI addresses in order from root to leaf.
pub fn collect_pci_ancestors(canonical_path: &Path) -> Vec<String> {
    let mut addrs = Vec::new();
    for component in canonical_path.components() {
        let name = component.as_os_str().to_string_lossy();
        if is_pci_address(&name) {
            addrs.push(name.into_owned());
        }
    }
    addrs
}

/// Extract the PCI root domain from a canonical sysfs path.
/// Looks for path components like "pci0000:4a" and returns "0000:4a".
pub fn find_pci_root(canonical_path: &Path) -> Option<String> {
    for component in canonical_path.components() {
        let name = component.as_os_str().to_string_lossy();
        if let Some(domain) = name.strip_prefix("pci") {
            // domain looks like "0000:4a"
            if domain.contains(':') {
                return Some(domain.to_string());
            }
        }
    }
    None
}
