// SPDX-License-Identifier: MIT
use crate::sysfs::format_size;
use crate::types::{
    BlockDevice, ControllerChild, DmDevice, DmSlave, NvmeCtrl, NvmeNamespace, NvmeSubsystem,
    Partition, PciNode, ScsiDevice, ScsiHost, ScsiTarget, Topology, TransportInfo,
};

/// Render the topology as an ASCII tree.
pub fn render(topology: &Topology) -> String {
    let mut out = String::new();

    // PCI roots
    for (i, root) in topology.pci_roots.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&format!("PCI {}\n", root.domain));
        render_pci_children(&mut out, &root.children, "");
    }

    // NVMe Fabrics section
    let show_fabrics = !topology.fabrics_controllers.is_empty();
    if show_fabrics {
        if !topology.pci_roots.is_empty() {
            out.push('\n');
        }
        out.push_str("NVMe Fabrics\n");
        let count = topology.fabrics_controllers.len();
        for (i, ctrl) in topology.fabrics_controllers.iter().enumerate() {
            let is_last = i + 1 == count;
            render_fabrics_controller(&mut out, ctrl, "", is_last);
        }
    }

    // NVMe Multipath section
    let show_multipath = has_interesting_subsystems(&topology.nvme_subsystems);
    if show_multipath {
        if !topology.pci_roots.is_empty() || show_fabrics {
            out.push('\n');
        }
        out.push_str("NVMe Multipath\n");
        let count = topology.nvme_subsystems.len();
        for (i, subsys) in topology.nvme_subsystems.iter().enumerate() {
            let is_last = i + 1 == count;
            render_subsystem(&mut out, subsys, "", is_last);
        }
    }

    // Device Mapper section
    if !topology.dm_devices.is_empty() {
        if !topology.pci_roots.is_empty() || show_fabrics || show_multipath {
            out.push('\n');
        }
        out.push_str("Device Mapper\n");
        let count = topology.dm_devices.len();
        for (i, dm) in topology.dm_devices.iter().enumerate() {
            let is_last = i + 1 == count;
            render_dm_device(&mut out, dm, "", is_last);
        }
    }

    out
}

fn connector(is_last: bool) -> &'static str {
    if is_last {
        "\u{2514}\u{2500}\u{2500} "
    } else {
        "\u{251c}\u{2500}\u{2500} "
    }
}

fn continuation(is_last: bool) -> &'static str {
    if is_last {
        "    "
    } else {
        "\u{2502}   "
    }
}

fn render_pci_children(out: &mut String, children: &[PciNode], prefix: &str) {
    let count = children.len();
    for (i, node) in children.iter().enumerate() {
        let is_last = i + 1 == count;
        match node {
            PciNode::Bridge(bridge) => {
                let driver_str = bridge
                    .driver
                    .as_deref()
                    .map(|d| format!(" (driver: {})", d))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "{}{}[{}] bridge{}\n",
                    prefix,
                    connector(is_last),
                    bridge.address,
                    driver_str
                ));
                let child_prefix = format!("{}{}", prefix, continuation(is_last));
                render_pci_children(out, &bridge.children, &child_prefix);
            }
            PciNode::StorageController(ctrl) => {
                render_storage_controller(out, ctrl, prefix, is_last);
            }
        }
    }
}

fn render_storage_controller(
    out: &mut String,
    ctrl: &crate::types::StorageController,
    prefix: &str,
    is_last: bool,
) {
    let driver_str = ctrl
        .driver
        .as_deref()
        .map(|d| format!(" (driver: {})", d))
        .unwrap_or_default();

    // Try to make a descriptive label
    let label = describe_pci_device(&ctrl.vendor_id, &ctrl.device_id, &ctrl.class_code);

    out.push_str(&format!(
        "{}{}[{}] {}{}\n",
        prefix,
        connector(is_last),
        ctrl.address,
        label,
        driver_str
    ));

    let child_prefix = format!("{}{}", prefix, continuation(is_last));
    let child_count = ctrl.children.len();

    for (i, child) in ctrl.children.iter().enumerate() {
        let child_is_last = i + 1 == child_count;
        match child {
            ControllerChild::NvmeController(nvme) => {
                render_nvme_controller(out, nvme, &child_prefix, child_is_last);
            }
            ControllerChild::ScsiHost(host) => {
                render_scsi_host(out, host, &child_prefix, child_is_last);
            }
        }
    }
}

fn render_nvme_controller(out: &mut String, ctrl: &NvmeCtrl, prefix: &str, is_last: bool) {
    let serial_str = if ctrl.serial.is_empty() {
        String::new()
    } else {
        format!(" (S/N: {}, FW: {})", ctrl.serial, ctrl.firmware_rev)
    };

    out.push_str(&format!(
        "{}{}{}: {}{}\n",
        prefix,
        connector(is_last),
        ctrl.name,
        ctrl.model,
        serial_str
    ));

    let child_prefix = format!("{}{}", prefix, continuation(is_last));
    let ns_count = ctrl.namespaces.len();

    for (i, ns) in ctrl.namespaces.iter().enumerate() {
        let ns_is_last = i + 1 == ns_count;
        render_namespace(out, ns, &child_prefix, ns_is_last);
    }
}

fn render_namespace(out: &mut String, ns: &NvmeNamespace, prefix: &str, is_last: bool) {
    let size_str = format_size(ns.size_bytes);

    // If the namespace has a block device with partitions, show them.
    // If it IS the block device (non-multipath case), show partitions.
    if let Some(bd) = &ns.block_device {
        if bd.name == ns.name {
            // Non-multipath: namespace IS the block device
            out.push_str(&format!(
                "{}{}{} ({})\n",
                prefix,
                connector(is_last),
                ns.name,
                size_str
            ));
            let child_prefix = format!("{}{}", prefix, continuation(is_last));
            render_partitions(out, &bd.partitions, &child_prefix);
        } else {
            // The namespace name differs from block device name
            out.push_str(&format!(
                "{}{}{} ({})\n",
                prefix,
                connector(is_last),
                ns.name,
                size_str
            ));
        }
    } else {
        out.push_str(&format!(
            "{}{}{} ({})\n",
            prefix,
            connector(is_last),
            ns.name,
            size_str
        ));
    }
}

fn render_scsi_host(out: &mut String, host: &ScsiHost, prefix: &str, is_last: bool) {
    let mut details = Vec::new();
    if let Some(proc_name) = &host.proc_name {
        details.push(proc_name.clone());
    }
    if let Some(ata_port) = &host.ata_port {
        details.push(ata_port.clone());
    }
    match &host.transport {
        Some(TransportInfo::Fc(_)) => details.push("FC".to_string()),
        Some(TransportInfo::Sas(_)) => details.push("SAS".to_string()),
        Some(TransportInfo::Iscsi(_)) => details.push("iSCSI".to_string()),
        None => {}
    }
    let details_str = if details.is_empty() {
        String::new()
    } else {
        format!(" ({})", details.join(", "))
    };

    out.push_str(&format!(
        "{}{}host{}{}\n",
        prefix,
        connector(is_last),
        host.host_num,
        details_str
    ));

    let child_prefix = format!("{}{}", prefix, continuation(is_last));

    if let Some(transport) = &host.transport {
        render_transport_info(out, transport, &child_prefix);
    }

    let target_count = host.targets.len();
    for (i, target) in host.targets.iter().enumerate() {
        let target_is_last = i + 1 == target_count;
        render_scsi_target(out, target, &child_prefix, target_is_last);
    }
}

fn render_scsi_target(out: &mut String, target: &ScsiTarget, prefix: &str, is_last: bool) {
    out.push_str(&format!(
        "{}{}target {}\n",
        prefix,
        connector(is_last),
        target.address
    ));

    let child_prefix = format!("{}{}", prefix, continuation(is_last));
    let dev_count = target.devices.len();

    for (i, device) in target.devices.iter().enumerate() {
        let dev_is_last = i + 1 == dev_count;
        render_scsi_device(out, device, &child_prefix, dev_is_last);
    }
}

fn render_scsi_device(out: &mut String, device: &ScsiDevice, prefix: &str, is_last: bool) {
    let type_str = match &device.sg_device {
        Some(sg) => format!("{}, {}", device.scsi_type_name, sg),
        None => device.scsi_type_name.clone(),
    };
    out.push_str(&format!(
        "{}{}[{}] {} {} ({})\n",
        prefix,
        connector(is_last),
        device.hctl,
        device.vendor,
        device.model,
        type_str
    ));

    if let Some(bd) = &device.block_device {
        let child_prefix = format!("{}{}", prefix, continuation(is_last));
        render_block_device(out, bd, &child_prefix, true);
    }
}

fn render_block_device(out: &mut String, bd: &BlockDevice, prefix: &str, is_last: bool) {
    let size_str = format_size(bd.size_bytes);

    out.push_str(&format!(
        "{}{}{} ({})\n",
        prefix,
        connector(is_last),
        bd.name,
        size_str
    ));

    let child_prefix = format!("{}{}", prefix, continuation(is_last));

    // Show partitions
    render_partitions(out, &bd.partitions, &child_prefix);

    // Show holders (DM devices that use this block device)
    if !bd.holders.is_empty() && bd.partitions.is_empty() {
        let holder_count = bd.holders.len();
        for (i, holder) in bd.holders.iter().enumerate() {
            let holder_is_last = i + 1 == holder_count;
            out.push_str(&format!(
                "{}{}\u{2192} {}\n",
                child_prefix,
                connector(holder_is_last),
                holder
            ));
        }
    }
}

fn render_partitions(out: &mut String, partitions: &[Partition], prefix: &str) {
    let count = partitions.len();
    for (i, part) in partitions.iter().enumerate() {
        let is_last = i + 1 == count;
        let size_str = format_size(part.size_bytes);
        out.push_str(&format!(
            "{}{}{} ({})\n",
            prefix,
            connector(is_last),
            part.name,
            size_str
        ));

        // Show holders of partitions
        if !part.holders.is_empty() {
            let holder_prefix = format!("{}{}", prefix, continuation(is_last));
            let holder_count = part.holders.len();
            for (j, holder) in part.holders.iter().enumerate() {
                let holder_is_last = j + 1 == holder_count;
                out.push_str(&format!(
                    "{}{}\u{2192} {}\n",
                    holder_prefix,
                    connector(holder_is_last),
                    holder
                ));
            }
        }
    }
}

fn render_subsystem(out: &mut String, subsys: &NvmeSubsystem, prefix: &str, is_last: bool) {
    let nqn_str = subsys
        .nqn
        .as_deref()
        .map(|n| format!(" [{}]", n))
        .unwrap_or_default();

    out.push_str(&format!(
        "{}{}{}: {} (S/N: {}){}\n",
        prefix,
        connector(is_last),
        subsys.name,
        subsys.model,
        subsys.serial,
        nqn_str
    ));

    let child_prefix = format!("{}{}", prefix, continuation(is_last));

    // Controllers
    let total = subsys.controllers.len() + subsys.namespaces.len();
    let mut idx = 0;

    if !subsys.controllers.is_empty() {
        let ctrl_list = subsys.controllers.join(", ");
        idx += 1;
        let item_is_last = idx == total && subsys.namespaces.is_empty();
        out.push_str(&format!(
            "{}{}controllers: {}\n",
            child_prefix,
            connector(item_is_last),
            ctrl_list
        ));
    }

    // Namespaces
    let ns_count = subsys.namespaces.len();
    for (i, ns) in subsys.namespaces.iter().enumerate() {
        let ns_is_last = i + 1 == ns_count;
        let size_str = format_size(ns.size_bytes);
        let nguid_str = ns
            .nguid
            .as_deref()
            .map(|g| format!(" [nguid: {}]", g))
            .unwrap_or_default();

        out.push_str(&format!(
            "{}{}{} ({}){}\n",
            child_prefix,
            connector(ns_is_last),
            ns.name,
            size_str,
            nguid_str
        ));
    }
}

fn render_dm_device(out: &mut String, dm: &DmDevice, prefix: &str, is_last: bool) {
    let size_str = format_size(dm.size_bytes);
    let path_str = multipath_summary(&dm.slaves);

    out.push_str(&format!(
        "{}{}{} ({}, {}, {}{})\n",
        prefix,
        connector(is_last),
        dm.dm_name,
        dm.dm_type,
        dm.name,
        size_str,
        path_str
    ));

    let child_prefix = format!("{}{}", prefix, continuation(is_last));

    let host_groups = group_slaves_by_host(&dm.slaves);
    let total_children = host_groups.len() + dm.partitions.len();
    let mut child_idx = 0;

    for (key, slaves) in &host_groups {
        child_idx += 1;
        let group_is_last = child_idx == total_children;

        match key {
            Some((host_num, driver, pci_addr)) => {
                let mut label = format!("host{}", host_num);
                if let Some(drv) = driver {
                    label.push_str(&format!(" ({})", drv));
                }
                if let Some(pci) = pci_addr {
                    label.push_str(&format!(" [{}]", pci));
                }
                out.push_str(&format!(
                    "{}{}{}\n",
                    child_prefix,
                    connector(group_is_last),
                    label
                ));
                let group_prefix = format!("{}{}", child_prefix, continuation(group_is_last));
                for (i, slave) in slaves.iter().enumerate() {
                    let s_is_last = i + 1 == slaves.len();
                    render_dm_slave(out, slave, &group_prefix, s_is_last);
                }
            }
            None => {
                for (i, slave) in slaves.iter().enumerate() {
                    let s_is_last = i + 1 == slaves.len() && child_idx == total_children;
                    out.push_str(&format!(
                        "{}{}\u{2190} {}\n",
                        child_prefix,
                        connector(s_is_last),
                        slave.device_name
                    ));
                }
            }
        }
    }

    for (i, part) in dm.partitions.iter().enumerate() {
        let p_is_last = child_idx + i + 1 == total_children;
        let p_size_str = format_size(part.size_bytes);
        out.push_str(&format!(
            "{}{}{} ({})\n",
            child_prefix,
            connector(p_is_last),
            part.name,
            p_size_str
        ));
    }
}

fn render_dm_slave(out: &mut String, slave: &DmSlave, prefix: &str, is_last: bool) {
    let hctl_str = slave
        .hctl
        .as_deref()
        .map(|h| format!(" [{}]", h))
        .unwrap_or_default();
    out.push_str(&format!(
        "{}{}{}{}\n",
        prefix,
        connector(is_last),
        slave.device_name,
        hctl_str
    ));
}

fn multipath_summary(slaves: &[DmSlave]) -> String {
    let num_paths = slaves.len();
    if num_paths < 2 {
        return String::new();
    }
    let mut unique_hosts = Vec::new();
    for slave in slaves {
        if let Some(h) = slave.host_num {
            if !unique_hosts.contains(&h) {
                unique_hosts.push(h);
            }
        }
    }
    let num_hosts = unique_hosts.len();
    if num_hosts == 0 {
        return String::new();
    }
    if num_hosts == 1 {
        format!(", {} paths via 1 HBA, multi-port", num_paths)
    } else {
        format!(", {} paths via {} HBAs", num_paths, num_hosts)
    }
}

type HostKey = Option<(u32, Option<String>, Option<String>)>;

fn group_slaves_by_host(slaves: &[DmSlave]) -> Vec<(HostKey, Vec<&DmSlave>)> {
    let mut groups: Vec<(HostKey, Vec<&DmSlave>)> = Vec::new();

    for slave in slaves {
        let key: HostKey = slave
            .host_num
            .map(|n| (n, slave.host_driver.clone(), slave.pci_address.clone()));

        if let Some(group) = groups.iter_mut().find(|(k, _)| *k == key) {
            group.1.push(slave);
        } else {
            groups.push((key, vec![slave]));
        }
    }

    groups
}

fn render_transport_info(out: &mut String, transport: &TransportInfo, prefix: &str) {
    match transport {
        TransportInfo::Fc(fc) => {
            out.push_str(&format!(
                "{}WWPN: {}  WWNN: {}\n",
                prefix, fc.port_name, fc.node_name
            ));
            let mut line = format!("{}state: {}  speed: {}", prefix, fc.port_state, fc.speed);
            if let Some(fabric) = &fc.fabric_name {
                line.push_str(&format!("  fabric: {}", fabric));
            }
            line.push('\n');
            out.push_str(&line);
        }
        TransportInfo::Sas(sas) => {
            for phy in &sas.phys {
                out.push_str(&format!(
                    "{}{}: addr={}  rate={}\n",
                    prefix, phy.name, phy.sas_address, phy.negotiated_linkrate
                ));
            }
        }
        TransportInfo::Iscsi(iscsi) => {
            for session in &iscsi.sessions {
                out.push_str(&format!(
                    "{}session: {} ({})\n",
                    prefix, session.target_name, session.state
                ));
                if let Some(addr) = &session.target_address {
                    let port_str = session
                        .target_port
                        .map(|p| format!(":{}", p))
                        .unwrap_or_default();
                    out.push_str(&format!("{}portal: {}{}\n", prefix, addr, port_str));
                }
            }
        }
    }
}

fn render_fabrics_controller(out: &mut String, ctrl: &NvmeCtrl, prefix: &str, is_last: bool) {
    let serial_str = if ctrl.serial.is_empty() {
        String::new()
    } else {
        format!(" (S/N: {}, FW: {})", ctrl.serial, ctrl.firmware_rev)
    };

    out.push_str(&format!(
        "{}{}{}: {}{}\n",
        prefix,
        connector(is_last),
        ctrl.name,
        ctrl.model,
        serial_str
    ));

    let child_prefix = format!("{}{}", prefix, continuation(is_last));

    let addr_str = ctrl
        .transport_address
        .as_deref()
        .map(|a| format!("  addr: {}", a))
        .unwrap_or_default();
    out.push_str(&format!(
        "{}transport: {}{}\n",
        child_prefix, ctrl.transport, addr_str
    ));

    let ns_count = ctrl.namespaces.len();
    for (i, ns) in ctrl.namespaces.iter().enumerate() {
        let ns_is_last = i + 1 == ns_count;
        render_namespace(out, ns, &child_prefix, ns_is_last);
    }
}

/// Determine if we should show the NVMe Multipath section.
/// Show it if any subsystem has more than one controller, or if subsystems exist at all
/// (indicating the multipath module is loaded).
fn has_interesting_subsystems(subsystems: &[NvmeSubsystem]) -> bool {
    subsystems.iter().any(|s| s.controllers.len() > 1)
}

/// Generate a descriptive label for a PCI device from its vendor/device IDs.
/// Without a PCI ID database, we fall back to showing the raw IDs plus class info.
fn describe_pci_device(vendor_id: &str, device_id: &str, class_code: &str) -> String {
    let class_desc = match class_code {
        c if c.starts_with("0x0106") => "SATA controller",
        c if c.starts_with("0x0108") => "NVMe controller",
        c if c.starts_with("0x0107") => "SAS controller",
        c if c.starts_with("0x0104") => "RAID controller",
        c if c.starts_with("0x0100") => "SCSI controller",
        c if c.starts_with("0x0101") => "IDE controller",
        c if c.starts_with("0x0102") => "Floppy controller",
        c if c.starts_with("0x0105") => "ATA controller",
        c if c.starts_with("0x01") => "storage controller",
        _ => "device",
    };
    format!("{} [{} {}]", class_desc, vendor_id, device_id)
}
