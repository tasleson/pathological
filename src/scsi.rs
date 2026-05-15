// SPDX-License-Identifier: MIT
use std::collections::HashMap;
use std::path::Path;

use crate::sysfs;
use crate::types::{
    BlockDevice, FcHostInfo, IscsiHostInfo, IscsiSessionInfo, SasHostInfo, SasPhyInfo, ScsiDevice,
    ScsiHost, ScsiTarget, TransportInfo,
};

struct RawScsiHost {
    host_num: u32,
    proc_name: Option<String>,
    ata_port: Option<String>,
    transport: Option<TransportInfo>,
    pci_address: Option<String>,
}

pub fn discover(
    block_devices: &HashMap<String, BlockDevice>,
) -> (HashMap<String, Vec<ScsiHost>>, Vec<String>) {
    let raw_hosts = discover_hosts();
    let devices_by_host = discover_devices(block_devices);

    let mut pci_addrs = Vec::new();
    let mut result: HashMap<String, Vec<ScsiHost>> = HashMap::new();

    for raw in raw_hosts {
        let host_num = raw.host_num;
        let targets = build_targets(host_num, &devices_by_host);

        let host = ScsiHost {
            host_num: raw.host_num,
            proc_name: raw.proc_name,
            ata_port: raw.ata_port,
            transport: raw.transport,
            targets,
        };

        if let Some(pci_addr) = &raw.pci_address {
            if !pci_addrs.contains(pci_addr) {
                pci_addrs.push(pci_addr.clone());
            }
            result.entry(pci_addr.clone()).or_default().push(host);
        }
    }

    for hosts in result.values_mut() {
        hosts.sort_by_key(|h| h.host_num);
    }

    (result, pci_addrs)
}

fn discover_hosts() -> Vec<RawScsiHost> {
    let scsi_host_class = Path::new("/sys/class/scsi_host");
    if !scsi_host_class.exists() {
        return Vec::new();
    }

    let mut hosts = Vec::new();
    let entries = sysfs::list_dir_sorted(scsi_host_class);

    for entry in entries {
        if !entry.starts_with("host") {
            continue;
        }

        let host_num: u32 = match entry["host".len()..].parse() {
            Ok(n) => n,
            Err(_) => continue,
        };

        let host_path = scsi_host_class.join(&entry);
        let canonical = match sysfs::resolve_link(&host_path) {
            Some(p) => p,
            None => continue,
        };

        let proc_name = sysfs::read_str(canonical.join("proc_name"));
        let ata_port = find_ata_port(&canonical);
        let pci_address = find_pci_parent(&canonical);
        let transport = discover_transport(host_num);

        hosts.push(RawScsiHost {
            host_num,
            proc_name,
            ata_port,
            transport,
            pci_address,
        });
    }

    hosts.sort_by_key(|h| h.host_num);
    hosts
}

fn find_ata_port(path: &Path) -> Option<String> {
    for component in path.components() {
        let name = component.as_os_str().to_string_lossy();
        if let Some(suffix) = name.strip_prefix("ata") {
            if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
                return Some(name.into_owned());
            }
        }
    }
    None
}

fn find_pci_parent(path: &Path) -> Option<String> {
    let ancestors = sysfs::collect_pci_ancestors(path);
    ancestors.last().cloned()
}

fn discover_transport(host_num: u32) -> Option<TransportInfo> {
    let host_name = format!("host{}", host_num);

    if let Some(fc) = discover_fc_transport(&host_name) {
        return Some(TransportInfo::Fc(fc));
    }
    if let Some(sas) = discover_sas_transport(&host_name, host_num) {
        return Some(TransportInfo::Sas(sas));
    }
    if let Some(iscsi) = discover_iscsi_transport(&host_name, host_num) {
        return Some(TransportInfo::Iscsi(iscsi));
    }
    None
}

fn discover_fc_transport(host_name: &str) -> Option<FcHostInfo> {
    let fc_path = Path::new("/sys/class/fc_host").join(host_name);
    if !fc_path.exists() {
        return None;
    }

    let canonical = sysfs::resolve_link(&fc_path).unwrap_or_else(|| fc_path.clone());

    let port_name = sysfs::read_str(canonical.join("port_name"))?;
    let node_name = sysfs::read_str(canonical.join("node_name")).unwrap_or_default();
    let port_state = sysfs::read_str(canonical.join("port_state")).unwrap_or_default();
    let port_type = sysfs::read_str(canonical.join("port_type")).unwrap_or_default();
    let speed = sysfs::read_str(canonical.join("speed")).unwrap_or_default();
    let fabric_name = sysfs::read_str(canonical.join("fabric_name"));
    let supported_speeds = sysfs::read_str(canonical.join("supported_speeds"));

    Some(FcHostInfo {
        port_name,
        node_name,
        port_state,
        port_type,
        speed,
        fabric_name,
        supported_speeds,
    })
}

fn discover_sas_transport(host_name: &str, host_num: u32) -> Option<SasHostInfo> {
    let sas_path = Path::new("/sys/class/sas_host").join(host_name);
    if !sas_path.exists() {
        return None;
    }

    let phy_prefix = format!("phy-{}:", host_num);
    let phy_class = Path::new("/sys/class/sas_phy");
    let mut phys = Vec::new();

    if phy_class.exists() {
        for entry in sysfs::list_dir_sorted(phy_class) {
            let suffix = match entry.strip_prefix(&phy_prefix) {
                Some(s) => s,
                None => continue,
            };
            // Only host-level phys (phy-N:M), not end-device phys (phy-N:M:P)
            if suffix.contains(':') {
                continue;
            }

            let phy_path = phy_class.join(&entry);
            let canonical = sysfs::resolve_link(&phy_path).unwrap_or_else(|| phy_path.clone());

            let sas_address = match sysfs::read_str(canonical.join("sas_address")) {
                Some(a) => a,
                None => continue,
            };
            let negotiated_linkrate =
                sysfs::read_str(canonical.join("negotiated_linkrate")).unwrap_or_default();
            let initiator_protocols = sysfs::read_str(canonical.join("initiator_port_protocols"));
            let target_protocols = sysfs::read_str(canonical.join("target_port_protocols"));

            phys.push(SasPhyInfo {
                name: entry,
                sas_address,
                negotiated_linkrate,
                initiator_protocols,
                target_protocols,
            });
        }
    }

    Some(SasHostInfo { phys })
}

fn discover_iscsi_transport(host_name: &str, _host_num: u32) -> Option<IscsiHostInfo> {
    let iscsi_host_path = Path::new("/sys/class/iscsi_host").join(host_name);
    if !iscsi_host_path.exists() {
        return None;
    }

    let session_class = Path::new("/sys/class/iscsi_session");
    let conn_class = Path::new("/sys/class/iscsi_connection");
    let mut sessions = Vec::new();

    if session_class.exists() {
        for entry in sysfs::list_dir_sorted(session_class) {
            if !entry.starts_with("session") {
                continue;
            }

            let session_path = session_class.join(&entry);
            let canonical =
                sysfs::resolve_link(&session_path).unwrap_or_else(|| session_path.clone());

            // Check if this session belongs to our host by looking at the sysfs path
            let path_str = canonical.to_string_lossy();
            if !path_str.contains(&format!("/{}/", host_name)) {
                continue;
            }

            let target_name = match sysfs::read_str(canonical.join("targetname")) {
                Some(n) => n,
                None => continue,
            };
            let state = sysfs::read_str(canonical.join("state")).unwrap_or_default();

            // Find connection for this session
            let (target_address, target_port) = find_iscsi_connection(conn_class, &entry);

            sessions.push(IscsiSessionInfo {
                target_name,
                state,
                target_address,
                target_port,
            });
        }
    }

    Some(IscsiHostInfo { sessions })
}

fn find_iscsi_connection(conn_class: &Path, session_name: &str) -> (Option<String>, Option<u16>) {
    if !conn_class.exists() {
        return (None, None);
    }

    // Session name is "sessionN", connection name is "connectionN:C"
    let session_num = session_name.strip_prefix("session").unwrap_or("");
    let conn_prefix = format!("connection{}:", session_num);

    for entry in sysfs::list_dir_sorted(conn_class) {
        if !entry.starts_with(&conn_prefix) {
            continue;
        }

        let conn_path = conn_class.join(&entry);
        let canonical = sysfs::resolve_link(&conn_path).unwrap_or_else(|| conn_path.clone());

        let address = sysfs::read_str(canonical.join("persistent_address"))
            .or_else(|| sysfs::read_str(canonical.join("address")));
        let port = sysfs::read_str(canonical.join("persistent_port"))
            .or_else(|| sysfs::read_str(canonical.join("port")))
            .and_then(|p| p.parse().ok());

        return (address, port);
    }

    (None, None)
}

struct RawScsiDevice {
    _hctl: String,
    channel: u32,
    target_id: u32,
    _lun: u32,
    device: ScsiDevice,
}

fn discover_devices(
    block_devices: &HashMap<String, BlockDevice>,
) -> HashMap<u32, Vec<RawScsiDevice>> {
    let scsi_bus = Path::new("/sys/bus/scsi/devices");
    if !scsi_bus.exists() {
        return HashMap::new();
    }

    let mut devices: HashMap<u32, Vec<RawScsiDevice>> = HashMap::new();
    let entries = sysfs::list_dir_sorted(scsi_bus);

    for entry in entries {
        if entry.starts_with("host") || entry.starts_with("target") {
            continue;
        }

        let parts: Vec<&str> = entry.split(':').collect();
        if parts.len() != 4 {
            continue;
        }
        let host_num: u32 = match parts[0].parse() {
            Ok(n) => n,
            Err(_) => continue,
        };
        let channel: u32 = match parts[1].parse() {
            Ok(n) => n,
            Err(_) => continue,
        };
        let target_id: u32 = match parts[2].parse() {
            Ok(n) => n,
            Err(_) => continue,
        };
        let lun: u32 = match parts[3].parse() {
            Ok(n) => n,
            Err(_) => continue,
        };

        let dev_path = scsi_bus.join(&entry);
        let canonical = match sysfs::resolve_link(&dev_path) {
            Some(p) => p,
            None => continue,
        };

        let vendor = sysfs::read_str(canonical.join("vendor")).unwrap_or_default();
        let model = sysfs::read_str(canonical.join("model")).unwrap_or_default();
        let rev = sysfs::read_str(canonical.join("rev")).unwrap_or_default();
        let scsi_type = sysfs::read_u64(canonical.join("type")).unwrap_or(0) as u32;
        let scsi_type_name = sysfs::scsi_type_name(scsi_type).to_string();

        let block_device = find_block_device(&canonical, block_devices);
        let sg_device = find_sg_device(&canonical);

        let device = ScsiDevice {
            hctl: entry.clone(),
            vendor,
            model,
            rev,
            scsi_type,
            scsi_type_name,
            block_device,
            sg_device,
        };

        devices.entry(host_num).or_default().push(RawScsiDevice {
            _hctl: entry,
            channel,
            target_id,
            _lun: lun,
            device,
        });
    }

    devices
}

fn find_block_device(
    scsi_dev_path: &Path,
    block_devices: &HashMap<String, BlockDevice>,
) -> Option<BlockDevice> {
    let block_dir = scsi_dev_path.join("block");
    if !block_dir.exists() {
        return None;
    }

    for name in sysfs::list_dir(&block_dir) {
        if let Some(bd) = block_devices.get(&name) {
            return Some(bd.clone());
        }
    }

    None
}

fn find_sg_device(scsi_dev_path: &Path) -> Option<String> {
    let sg_dir = scsi_dev_path.join("scsi_generic");
    if !sg_dir.exists() {
        return None;
    }
    sysfs::list_dir(&sg_dir)
        .into_iter()
        .find(|name| name.starts_with("sg"))
}

fn build_targets(
    host_num: u32,
    devices_by_host: &HashMap<u32, Vec<RawScsiDevice>>,
) -> Vec<ScsiTarget> {
    let devs = match devices_by_host.get(&host_num) {
        Some(d) => d,
        None => return Vec::new(),
    };

    let mut target_map: HashMap<String, Vec<ScsiDevice>> = HashMap::new();
    let mut target_order: Vec<String> = Vec::new();

    for raw in devs {
        let target_addr = format!("{}:{}:{}", host_num, raw.channel, raw.target_id);
        if !target_order.contains(&target_addr) {
            target_order.push(target_addr.clone());
        }
        target_map
            .entry(target_addr)
            .or_default()
            .push(raw.device.clone());
    }

    target_order.sort();

    let mut targets = Vec::new();
    for addr in target_order {
        let mut devices = target_map.remove(&addr).unwrap_or_default();
        devices.sort_by(|a, b| a.hctl.cmp(&b.hctl));
        targets.push(ScsiTarget {
            address: addr,
            devices,
        });
    }

    targets
}
