// SPDX-License-Identifier: MIT
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::sysfs;
use crate::types::{PciBridge, PciNode, PciRoot, StorageController};

/// Information about a single PCI device read from sysfs.
struct PciDeviceInfo {
    vendor_id: String,
    device_id: String,
    class_code: u32,
    driver: Option<String>,
    numa_node: Option<i32>,
}

/// A chain of PCI devices from root to endpoint.
struct PciChain {
    root_domain: String,
    /// PCI addresses from root-most bridge to endpoint, in order.
    devices: Vec<String>,
}

/// Discover PCI hierarchy from known NVMe and SCSI PCI addresses.
/// Returns a list of PCI root complexes with their device trees.
pub fn discover_pci_hierarchy(
    nvme_pci_addrs: &[String],
    scsi_pci_addrs: &[String],
) -> Vec<PciRoot> {
    let mut all_addrs: HashSet<String> = HashSet::new();
    for addr in nvme_pci_addrs {
        all_addrs.insert(addr.clone());
    }
    for addr in scsi_pci_addrs {
        all_addrs.insert(addr.clone());
    }

    // Also scan for storage-class PCI devices not already known
    // (catches vfio-pci passthrough controllers)
    let extra = scan_storage_class_devices();
    for addr in &extra {
        all_addrs.insert(addr.clone());
    }

    if all_addrs.is_empty() {
        return Vec::new();
    }

    // For each endpoint address, trace the chain back to root
    let mut chains: Vec<PciChain> = Vec::new();
    let mut device_info_cache: HashMap<String, PciDeviceInfo> = HashMap::new();

    for addr in &all_addrs {
        if let Some(chain) = trace_pci_chain(addr, &mut device_info_cache) {
            chains.push(chain);
        }
    }

    // Group chains by root domain
    let mut root_map: HashMap<String, Vec<PciChain>> = HashMap::new();
    for chain in chains {
        root_map
            .entry(chain.root_domain.clone())
            .or_default()
            .push(chain);
    }

    // Build tree for each root
    let mut roots: Vec<PciRoot> = Vec::new();
    let mut root_domains: Vec<String> = root_map.keys().cloned().collect();
    root_domains.sort();

    for domain in root_domains {
        let domain_chains = root_map.remove(&domain).unwrap();
        let children = build_pci_tree(&domain_chains, &device_info_cache);
        roots.push(PciRoot { domain, children });
    }

    roots
}

/// Scan /sys/bus/pci/devices/ for storage-class devices (class 0x010000-0x0108ff)
/// that might be under vfio-pci or otherwise not discovered via NVMe/SCSI.
fn scan_storage_class_devices() -> Vec<String> {
    let pci_devices = Path::new("/sys/bus/pci/devices");
    if !pci_devices.exists() {
        return Vec::new();
    }

    let mut addrs = Vec::new();
    for entry in sysfs::list_dir_sorted(pci_devices) {
        if !sysfs::is_pci_address(&entry) {
            continue;
        }
        let dev_path = pci_devices.join(&entry);
        if let Some(class_code) = sysfs::read_hex_u32(dev_path.join("class")) {
            let class_base = class_code >> 8;
            // Storage controllers: class 0x0100 through 0x0108
            if (0x0100..=0x0108).contains(&class_base) {
                addrs.push(entry);
            }
        }
    }

    addrs
}

/// Trace a PCI device back to its root complex through sysfs canonical path.
fn trace_pci_chain(addr: &str, cache: &mut HashMap<String, PciDeviceInfo>) -> Option<PciChain> {
    let dev_path = Path::new("/sys/bus/pci/devices").join(addr);
    let canonical = sysfs::resolve_link(&dev_path)?;

    let pci_addrs = sysfs::collect_pci_ancestors(&canonical);
    if pci_addrs.is_empty() {
        return None;
    }

    let root_domain = sysfs::find_pci_root(&canonical)?;

    // Ensure all devices in the chain are cached
    for pci_addr in &pci_addrs {
        if !cache.contains_key(pci_addr) {
            if let Some(info) = read_pci_device_info(pci_addr) {
                cache.insert(pci_addr.clone(), info);
            }
        }
    }

    Some(PciChain {
        root_domain,
        devices: pci_addrs,
    })
}

/// Read PCI device info from sysfs.
fn read_pci_device_info(addr: &str) -> Option<PciDeviceInfo> {
    let dev_path = Path::new("/sys/bus/pci/devices").join(addr);

    let vendor_id =
        sysfs::read_str(dev_path.join("vendor")).unwrap_or_else(|| "0x0000".to_string());
    let device_id =
        sysfs::read_str(dev_path.join("device")).unwrap_or_else(|| "0x0000".to_string());
    let class_code = sysfs::read_hex_u32(dev_path.join("class")).unwrap_or(0);
    let driver = sysfs::read_driver(&dev_path);
    let numa_node = sysfs::read_i32(dev_path.join("numa_node"));

    Some(PciDeviceInfo {
        vendor_id,
        device_id,
        class_code,
        driver,
        numa_node,
    })
}

/// Check if a PCI class code indicates a PCI-to-PCI bridge (class 0x0604xx).
fn is_bridge(class_code: u32) -> bool {
    (class_code >> 8) == 0x0604
}

/// Check if a PCI class code indicates a storage controller.
fn is_storage_controller(class_code: u32) -> bool {
    let class_base = class_code >> 8;
    (0x0100..=0x0108).contains(&class_base)
}

/// Build a PCI tree from a set of chains sharing the same root domain.
fn build_pci_tree(
    chains: &[PciChain],
    device_info: &HashMap<String, PciDeviceInfo>,
) -> Vec<PciNode> {
    // Collect all unique device addresses at each depth level,
    // then build the tree recursively.

    // A trie-like structure: for each address, what are the possible next addresses?
    // chains[i].devices = [bridge1, bridge2, ..., endpoint]
    // We build a tree where each level corresponds to a depth in the chain.

    build_children_at_depth(chains, 0, device_info)
}

/// Recursively build PCI children at a given depth in the chains.
fn build_children_at_depth(
    chains: &[PciChain],
    depth: usize,
    device_info: &HashMap<String, PciDeviceInfo>,
) -> Vec<PciNode> {
    // Group chains by the address at this depth (store indices)
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    let mut order: Vec<String> = Vec::new();

    for (i, chain) in chains.iter().enumerate() {
        if depth < chain.devices.len() {
            let addr = &chain.devices[depth];
            if !order.contains(addr) {
                order.push(addr.clone());
            }
            groups.entry(addr.clone()).or_default().push(i);
        }
    }

    order.sort();

    let mut nodes = Vec::new();
    for addr in order {
        let matching_indices = groups.remove(&addr).unwrap_or_default();
        let info = device_info.get(&addr);

        let is_leaf = matching_indices
            .iter()
            .all(|&i| depth + 1 >= chains[i].devices.len());

        if is_leaf {
            // This is an endpoint (storage controller or other device)
            if let Some(info) = info {
                if is_bridge(info.class_code) {
                    // A bridge with no further children we discovered
                    nodes.push(PciNode::Bridge(PciBridge {
                        address: addr,
                        vendor_id: info.vendor_id.clone(),
                        device_id: info.device_id.clone(),
                        driver: info.driver.clone(),
                        children: Vec::new(),
                    }));
                } else {
                    nodes.push(PciNode::StorageController(StorageController {
                        address: addr,
                        vendor_id: info.vendor_id.clone(),
                        device_id: info.device_id.clone(),
                        class_code: format!("0x{:06x}", info.class_code),
                        driver: info.driver.clone(),
                        numa_node: info.numa_node,
                        children: Vec::new(),
                    }));
                }
            }
        } else {
            // This is an intermediate device — collect matching chains
            let sub_chains: Vec<PciChain> = matching_indices
                .iter()
                .map(|&i| PciChain {
                    root_domain: chains[i].root_domain.clone(),
                    devices: chains[i].devices.clone(),
                })
                .collect();
            let children = build_children_at_depth(&sub_chains, depth + 1, device_info);
            if let Some(info) = info {
                if is_bridge(info.class_code) {
                    nodes.push(PciNode::Bridge(PciBridge {
                        address: addr,
                        vendor_id: info.vendor_id.clone(),
                        device_id: info.device_id.clone(),
                        driver: info.driver.clone(),
                        children,
                    }));
                } else if is_storage_controller(info.class_code) {
                    // A storage controller that somehow has children in the PCI
                    // chain — treat as storage controller with no PCI children,
                    // but we should not lose the children. This is unusual but
                    // handle it by making it a bridge-like node.
                    nodes.push(PciNode::StorageController(StorageController {
                        address: addr,
                        vendor_id: info.vendor_id.clone(),
                        device_id: info.device_id.clone(),
                        class_code: format!("0x{:06x}", info.class_code),
                        driver: info.driver.clone(),
                        numa_node: info.numa_node,
                        children: Vec::new(),
                    }));
                    // Append children as siblings (they'll be at the same level)
                    nodes.extend(children);
                } else {
                    // Some other device type acting as intermediate — treat as bridge
                    nodes.push(PciNode::Bridge(PciBridge {
                        address: addr,
                        vendor_id: info.vendor_id.clone(),
                        device_id: info.device_id.clone(),
                        driver: info.driver.clone(),
                        children,
                    }));
                }
            } else {
                // No info available but has children — show as bridge
                nodes.push(PciNode::Bridge(PciBridge {
                    address: addr.clone(),
                    vendor_id: "0x0000".to_string(),
                    device_id: "0x0000".to_string(),
                    driver: None,
                    children,
                }));
            }
        }
    }

    nodes
}
