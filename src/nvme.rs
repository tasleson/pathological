// SPDX-License-Identifier: MIT
use std::collections::HashMap;
use std::path::Path;

use crate::sysfs;
use crate::types::{BlockDevice, NvmeCtrl, NvmeNamespace, NvmeSubsystem};

/// Discover NVMe controllers and subsystems.
/// Returns (controllers, subsystems).
pub fn discover(
    block_devices: &HashMap<String, BlockDevice>,
) -> (Vec<NvmeCtrl>, Vec<NvmeSubsystem>) {
    let controllers = discover_controllers(block_devices);
    let subsystems = discover_subsystems(block_devices);
    (controllers, subsystems)
}

/// Collect PCI addresses used by NVMe controllers.
pub fn pci_addresses(controllers: &[NvmeCtrl]) -> Vec<String> {
    controllers
        .iter()
        .filter_map(|c| c.pci_address.clone())
        .collect()
}

fn discover_controllers(block_devices: &HashMap<String, BlockDevice>) -> Vec<NvmeCtrl> {
    let nvme_class = Path::new("/sys/class/nvme");
    if !nvme_class.exists() {
        return Vec::new();
    }

    let mut controllers = Vec::new();
    let mut entries = sysfs::list_dir_sorted(nvme_class);
    entries.retain(|e| e.starts_with("nvme"));

    for ctrl_name in entries {
        let ctrl_path = nvme_class.join(&ctrl_name);
        let canonical = match sysfs::resolve_link(&ctrl_path) {
            Some(p) => p,
            None => continue,
        };

        let model = sysfs::read_str(canonical.join("model")).unwrap_or_default();
        let serial = sysfs::read_str(canonical.join("serial")).unwrap_or_default();
        let firmware_rev = sysfs::read_str(canonical.join("firmware_rev")).unwrap_or_default();
        let transport =
            sysfs::read_str(canonical.join("transport")).unwrap_or_else(|| "pcie".to_string());
        let state = sysfs::read_str(canonical.join("state")).unwrap_or_default();

        let raw_address = sysfs::read_str(canonical.join("address"));

        let (pci_address, transport_address) = if transport == "pcie" {
            let pci = sysfs::read_link(canonical.join("device")).and_then(|link| {
                link.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .filter(|n| sysfs::is_pci_address(n))
            });
            (pci, None)
        } else {
            (None, raw_address)
        };

        let subsys_name = find_subsystem_for_controller(&canonical, &ctrl_name);
        let namespaces = discover_controller_namespaces(&canonical, &ctrl_name, block_devices);

        controllers.push(NvmeCtrl {
            name: ctrl_name,
            model,
            serial,
            firmware_rev,
            transport,
            state,
            pci_address,
            transport_address,
            subsys_name,
            namespaces,
        });
    }

    controllers.sort_by(|a, b| a.name.cmp(&b.name));
    controllers
}

/// Find the subsystem name for a controller.
/// Check if the canonical path contains "nvme-subsys" or look for a subsystem symlink.
fn find_subsystem_for_controller(canonical_path: &Path, ctrl_name: &str) -> Option<String> {
    // Method 1: Look for nvme-subsysN in the subsystem class that lists this controller
    let subsys_class = Path::new("/sys/class/nvme-subsystem");
    if subsys_class.exists() {
        for entry in sysfs::list_dir_sorted(subsys_class) {
            if !entry.starts_with("nvme-subsys") {
                continue;
            }
            let subsys_path = subsys_class.join(&entry);
            let subsys_canonical = match sysfs::resolve_link(&subsys_path) {
                Some(p) => p,
                None => continue,
            };
            // Check if this controller is listed in the subsystem
            let ctrls_in_subsys = sysfs::list_dir(&subsys_canonical);
            if ctrls_in_subsys.contains(&ctrl_name.to_string()) {
                return Some(entry);
            }
        }
    }

    // Method 2: Check canonical path for subsystem component
    for component in canonical_path.components() {
        let name = component.as_os_str().to_string_lossy();
        if name.starts_with("nvme-subsys") {
            return Some(name.into_owned());
        }
    }

    None
}

/// Discover namespaces directly under a controller directory.
/// These are the controller-specific namespace devices (e.g., nvme0n1 or nvme0c0n1).
fn discover_controller_namespaces(
    ctrl_canonical: &Path,
    ctrl_name: &str,
    block_devices: &HashMap<String, BlockDevice>,
) -> Vec<NvmeNamespace> {
    let mut namespaces = Vec::new();
    let entries = sysfs::list_dir_sorted(ctrl_canonical);

    for entry in &entries {
        // Match patterns like "nvme0n1", "nvme0c0n1"
        if !is_nvme_namespace_name(entry, ctrl_name) {
            continue;
        }

        let ns_path = ctrl_canonical.join(entry);
        let nsid = extract_nsid(entry);
        let nguid = sysfs::read_str(ns_path.join("nguid"))
            .or_else(|| {
                // Try reading from /sys/class/block/<name>/nguid
                sysfs::read_str(Path::new("/sys/class/block").join(entry).join("nguid"))
            })
            .filter(|g| {
                g != "00000000-0000-0000-0000-000000000000"
                    && g != "0"
                    && !g.chars().all(|c| c == '0' || c == '-')
            });

        let size_bytes = block_devices
            .get(entry.as_str())
            .map(|bd| bd.size_bytes)
            .or_else(|| sysfs::read_u64(ns_path.join("size")).map(|s| s * 512))
            .unwrap_or(0);

        let block_device = block_devices.get(entry.as_str()).cloned();

        namespaces.push(NvmeNamespace {
            name: entry.clone(),
            nsid,
            nguid,
            size_bytes,
            block_device,
        });
    }

    namespaces
}

/// Check if a name looks like an NVMe namespace for the given controller.
/// Matches: nvme0n1, nvme0c0n1, nvme1n2, etc.
fn is_nvme_namespace_name(name: &str, ctrl_name: &str) -> bool {
    if !name.starts_with(ctrl_name) {
        return false;
    }
    let suffix = &name[ctrl_name.len()..];
    // Must start with 'n' (direct namespace) or 'c' (controller-specific namespace path)
    if let Some(after_n) = suffix.strip_prefix('n') {
        // nvme0n1 pattern - remainder after 'n' should be digits
        !after_n.is_empty() && after_n.chars().all(|c| c.is_ascii_digit())
    } else if let Some(after_c) = suffix.strip_prefix('c') {
        // nvme0c0n1 pattern
        // After 'c' we expect digits, then 'n', then digits
        if let Some(n_pos) = after_c.find('n') {
            let ctrl_part = &after_c[..n_pos];
            let ns_part = &after_c[n_pos + 1..];
            !ctrl_part.is_empty()
                && ctrl_part.chars().all(|c| c.is_ascii_digit())
                && !ns_part.is_empty()
                && ns_part.chars().all(|c| c.is_ascii_digit())
        } else {
            false
        }
    } else {
        false
    }
}

/// Extract the namespace ID from a namespace name.
/// E.g., "nvme0n1" → 1, "nvme0c0n1" → 1
fn extract_nsid(name: &str) -> Option<u32> {
    // Find the last 'n' followed by digits
    let mut last_n_pos = None;
    for (i, c) in name.char_indices() {
        if c == 'n' && i + 1 < name.len() {
            let rest = &name[i + 1..];
            if !rest.is_empty() && rest.chars().all(|ch| ch.is_ascii_digit()) {
                last_n_pos = Some(i);
            }
        }
    }
    last_n_pos.and_then(|pos| name[pos + 1..].parse().ok())
}

fn discover_subsystems(block_devices: &HashMap<String, BlockDevice>) -> Vec<NvmeSubsystem> {
    let subsys_class = Path::new("/sys/class/nvme-subsystem");
    if !subsys_class.exists() {
        return Vec::new();
    }

    let mut subsystems = Vec::new();
    let entries = sysfs::list_dir_sorted(subsys_class);

    for entry in entries {
        if !entry.starts_with("nvme-subsys") {
            continue;
        }

        let subsys_path = subsys_class.join(&entry);
        let canonical = match sysfs::resolve_link(&subsys_path) {
            Some(p) => p,
            None => continue,
        };

        let model = sysfs::read_str(canonical.join("model")).unwrap_or_default();
        let serial = sysfs::read_str(canonical.join("serial")).unwrap_or_default();
        let nqn = sysfs::read_str(canonical.join("subsysnqn"));

        // List controllers in this subsystem
        let mut controllers = Vec::new();
        for item in sysfs::list_dir_sorted(&canonical) {
            if item.starts_with("nvme") && !item.contains("subsys") && !item.contains('n') {
                // Just controller names like "nvme0", "nvme1"
                // They should be purely "nvme" + digits
                let suffix = &item["nvme".len()..];
                if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
                    controllers.push(item);
                }
            }
        }

        // List namespaces in this subsystem
        let mut namespaces = Vec::new();
        for item in sysfs::list_dir_sorted(&canonical) {
            // Namespace entries at subsystem level: nvme0n1, nvme1n1, etc.
            if !item.starts_with("nvme") {
                continue;
            }
            // Must contain 'n' after initial "nvme\d+"
            if let Some(first_n) = item[4..].find('n') {
                let ctrl_part = &item[4..4 + first_n];
                let ns_part = &item[4 + first_n + 1..];
                if ctrl_part.chars().all(|c| c.is_ascii_digit())
                    && !ns_part.is_empty()
                    && ns_part.chars().all(|c| c.is_ascii_digit())
                {
                    // This is a namespace name
                    let ns_path = canonical.join(&item);
                    let nsid = ns_part.parse().ok();
                    let nguid = sysfs::read_str(ns_path.join("nguid"))
                        .or_else(|| {
                            sysfs::read_str(Path::new("/sys/class/block").join(&item).join("nguid"))
                        })
                        .filter(|g| {
                            g != "00000000-0000-0000-0000-000000000000"
                                && g != "0"
                                && !g.chars().all(|c| c == '0' || c == '-')
                        });

                    let size_bytes = sysfs::read_u64(ns_path.join("size"))
                        .map(|s| s * 512)
                        .or_else(|| block_devices.get(item.as_str()).map(|bd| bd.size_bytes))
                        .unwrap_or(0);

                    let block_device = block_devices.get(item.as_str()).cloned();

                    namespaces.push(NvmeNamespace {
                        name: item,
                        nsid,
                        nguid,
                        size_bytes,
                        block_device,
                    });
                }
            }
        }

        subsystems.push(NvmeSubsystem {
            name: entry,
            nqn,
            model,
            serial,
            controllers,
            namespaces,
        });
    }

    subsystems
}
