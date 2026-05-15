// SPDX-License-Identifier: MIT
use std::collections::HashMap;

use crate::types::{
    ControllerChild, DmDevice, NvmeCtrl, NvmeSubsystem, PciNode, PciRoot, ScsiHost,
    StorageController, Topology,
};

pub struct BuildArgs {
    pub no_empty: bool,
}

pub fn build(
    mut pci_roots: Vec<PciRoot>,
    nvme_controllers: Vec<NvmeCtrl>,
    scsi_hosts_by_pci: HashMap<String, Vec<ScsiHost>>,
    nvme_subsystems: Vec<NvmeSubsystem>,
    dm_devices: Vec<DmDevice>,
    args: &BuildArgs,
) -> Topology {
    let mut nvme_by_pci: HashMap<String, Vec<NvmeCtrl>> = HashMap::new();
    let mut fabrics_controllers: Vec<NvmeCtrl> = Vec::new();

    for ctrl in nvme_controllers {
        if let Some(addr) = &ctrl.pci_address {
            nvme_by_pci.entry(addr.clone()).or_default().push(ctrl);
        } else {
            fabrics_controllers.push(ctrl);
        }
    }

    for root in &mut pci_roots {
        attach_children_to_tree(
            &mut root.children,
            &mut nvme_by_pci,
            &scsi_hosts_by_pci,
            args,
        );
    }

    if args.no_empty {
        for root in &mut pci_roots {
            prune_empty(&mut root.children);
        }
        pci_roots.retain(|r| !r.children.is_empty());
    }

    fabrics_controllers.sort_by(|a, b| a.name.cmp(&b.name));

    Topology {
        pci_roots,
        fabrics_controllers,
        nvme_subsystems,
        dm_devices,
    }
}

fn attach_children_to_tree(
    nodes: &mut [PciNode],
    nvme_by_pci: &mut HashMap<String, Vec<NvmeCtrl>>,
    scsi_hosts_by_pci: &HashMap<String, Vec<ScsiHost>>,
    args: &BuildArgs,
) {
    for node in nodes.iter_mut() {
        match node {
            PciNode::Bridge(bridge) => {
                attach_children_to_tree(&mut bridge.children, nvme_by_pci, scsi_hosts_by_pci, args);
            }
            PciNode::StorageController(ctrl) => {
                attach_to_controller(ctrl, nvme_by_pci, scsi_hosts_by_pci, args);
            }
        }
    }
}

fn attach_to_controller(
    ctrl: &mut StorageController,
    nvme_by_pci: &mut HashMap<String, Vec<NvmeCtrl>>,
    scsi_hosts_by_pci: &HashMap<String, Vec<ScsiHost>>,
    args: &BuildArgs,
) {
    if let Some(nvme_ctrls) = nvme_by_pci.remove(&ctrl.address) {
        for nvme_ctrl in nvme_ctrls {
            ctrl.children
                .push(ControllerChild::NvmeController(nvme_ctrl));
        }
    }

    if let Some(scsi_hosts) = scsi_hosts_by_pci.get(&ctrl.address) {
        for host in scsi_hosts {
            let mut host_clone = host.clone();
            if args.no_empty {
                host_clone.targets.retain(|t| !t.devices.is_empty());
            }
            if !args.no_empty || !host_clone.targets.is_empty() {
                ctrl.children.push(ControllerChild::ScsiHost(host_clone));
            }
        }
    }
}

fn prune_empty(nodes: &mut Vec<PciNode>) {
    for node in nodes.iter_mut() {
        if let PciNode::Bridge(bridge) = node {
            prune_empty(&mut bridge.children);
        }
    }

    nodes.retain(|node| match node {
        PciNode::Bridge(bridge) => !bridge.children.is_empty(),
        PciNode::StorageController(ctrl) => !ctrl.children.is_empty(),
    });
}
