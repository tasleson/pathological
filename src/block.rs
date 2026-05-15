// SPDX-License-Identifier: MIT
use std::collections::HashMap;
use std::path::Path;

use crate::sysfs;
use crate::types::{BlockDevice, DmDevice, DmSlave, Partition};

/// Discover all block devices and DM devices from sysfs.
/// Returns (block_device_map, dm_devices).
pub fn discover() -> (HashMap<String, BlockDevice>, Vec<DmDevice>) {
    let mut block_map: HashMap<String, BlockDevice> = HashMap::new();
    let mut dm_devices: Vec<DmDevice> = Vec::new();

    let class_block = Path::new("/sys/class/block");
    let entries = sysfs::list_dir_sorted(class_block);

    // First pass: discover all non-partition, non-dm block devices
    for name in &entries {
        let dev_path = class_block.join(name);
        let canonical = match sysfs::resolve_link(&dev_path) {
            Some(p) => p,
            None => continue,
        };

        // Skip partitions in first pass
        if canonical.join("partition").exists() {
            continue;
        }

        // Skip dm devices in first pass (handle separately)
        if name.starts_with("dm-") {
            continue;
        }

        let size_sectors = sysfs::read_u64(canonical.join("size")).unwrap_or(0);
        let size_bytes = size_sectors * 512;
        let removable = sysfs::read_u64(canonical.join("removable")).unwrap_or(0) != 0;
        let read_only = sysfs::read_u64(canonical.join("ro")).unwrap_or(0) != 0;
        let wwn = sysfs::read_str(canonical.join("device/wwid"))
            .or_else(|| sysfs::read_str(canonical.join("wwid")));

        // Find partitions for this device
        let partitions = discover_partitions(&canonical, name);

        // Find holders
        let holders = sysfs::list_dir_sorted(canonical.join("holders"));

        let blk = BlockDevice {
            name: name.clone(),
            dev_path: format!("/dev/{}", name),
            size_bytes,
            removable,
            read_only,
            wwn,
            partitions,
            holders,
        };

        block_map.insert(name.clone(), blk);
    }

    // Second pass: discover partitions and attach to parent devices
    // (Already done inline above via discover_partitions)

    // Third pass: discover DM devices
    for name in &entries {
        if !name.starts_with("dm-") {
            continue;
        }

        let dev_path = class_block.join(name);
        let canonical = match sysfs::resolve_link(&dev_path) {
            Some(p) => p,
            None => continue,
        };

        let dm_name = sysfs::read_str(canonical.join("dm/name")).unwrap_or_default();
        let dm_uuid = sysfs::read_str(canonical.join("dm/uuid")).unwrap_or_default();
        let dm_type = classify_dm_type(&dm_uuid);
        let size_sectors = sysfs::read_u64(canonical.join("size")).unwrap_or(0);
        let size_bytes = size_sectors * 512;

        let slave_names = sysfs::list_dir_sorted(canonical.join("slaves"));
        let slaves: Vec<DmSlave> = slave_names
            .into_iter()
            .map(|s| discover_slave(&s))
            .collect();
        let partitions = discover_partitions(&canonical, name);

        let dm = DmDevice {
            name: name.clone(),
            dm_name,
            dm_uuid,
            dm_type,
            size_bytes,
            slaves,
            partitions,
        };

        dm_devices.push(dm);
    }

    dm_devices.sort_by(|a, b| a.name.cmp(&b.name));

    (block_map, dm_devices)
}

/// Discover partitions for a given block device by scanning its sysfs directory.
fn discover_partitions(device_sysfs_path: &Path, parent_name: &str) -> Vec<Partition> {
    let mut partitions = Vec::new();

    let entries = sysfs::list_dir_sorted(device_sysfs_path);
    for entry in entries {
        // Partition entries are named like "sda1", "nvme0n1p1", etc.
        // They must start with the parent device name.
        if !entry.starts_with(parent_name) {
            continue;
        }

        let part_path = device_sysfs_path.join(&entry);
        // Must have a "partition" file to be a partition
        let part_num = match sysfs::read_u64(part_path.join("partition")) {
            Some(n) => n as u32,
            None => continue,
        };

        let size_sectors = sysfs::read_u64(part_path.join("size")).unwrap_or(0);
        let size_bytes = size_sectors * 512;
        let holders = sysfs::list_dir_sorted(part_path.join("holders"));

        partitions.push(Partition {
            name: entry,
            number: part_num,
            size_bytes,
            holders,
        });
    }

    partitions.sort_by_key(|p| p.number);
    partitions
}

fn discover_slave(name: &str) -> DmSlave {
    let block_path = Path::new("/sys/class/block").join(name);
    let hctl = sysfs::read_link(block_path.join("device")).and_then(|link| {
        link.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .filter(|n| n.contains(':'))
    });

    let (host_num, host_driver, pci_address) = match &hctl {
        Some(h) => {
            let num: Option<u32> = h.split(':').next().and_then(|s| s.parse().ok());
            match num {
                Some(n) => {
                    let host_path = Path::new("/sys/class/scsi_host").join(format!("host{}", n));
                    let driver = sysfs::read_str(
                        sysfs::resolve_link(&host_path)
                            .unwrap_or_else(|| host_path.clone())
                            .join("proc_name"),
                    );
                    let pci = sysfs::resolve_link(&host_path).and_then(|p| {
                        let ancestors = sysfs::collect_pci_ancestors(&p);
                        ancestors.last().cloned()
                    });
                    (Some(n), driver, pci)
                }
                None => (None, None, None),
            }
        }
        None => (None, None, None),
    };

    DmSlave {
        device_name: name.to_string(),
        hctl,
        host_num,
        host_driver,
        pci_address,
    }
}

/// Classify DM device type from its UUID prefix.
fn classify_dm_type(uuid: &str) -> String {
    if uuid.starts_with("LVM-") {
        "LVM".to_string()
    } else if uuid.starts_with("mpath-") {
        "Multipath".to_string()
    } else if uuid.starts_with("CRYPT-") {
        "Crypt".to_string()
    } else if uuid.is_empty() {
        "Unknown".to_string()
    } else {
        "Other".to_string()
    }
}
