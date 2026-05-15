// SPDX-License-Identifier: MIT
use crate::types::*;

struct JsonWriter {
    buf: String,
    indent: usize,
}

impl JsonWriter {
    fn new() -> Self {
        Self {
            buf: String::with_capacity(8192),
            indent: 0,
        }
    }

    fn indent(&mut self) {
        for _ in 0..self.indent {
            self.buf.push_str("  ");
        }
    }

    fn key(&mut self, name: &str) {
        self.indent();
        self.buf.push('"');
        self.buf.push_str(name);
        self.buf.push_str("\": ");
    }

    fn str_val(&mut self, s: &str) {
        self.buf.push('"');
        for c in s.chars() {
            match c {
                '"' => self.buf.push_str("\\\""),
                '\\' => self.buf.push_str("\\\\"),
                '\n' => self.buf.push_str("\\n"),
                '\r' => self.buf.push_str("\\r"),
                '\t' => self.buf.push_str("\\t"),
                c if c < '\x20' => {
                    self.buf.push_str(&format!("\\u{:04x}", c as u32));
                }
                c => self.buf.push(c),
            }
        }
        self.buf.push('"');
    }

    fn opt_str(&mut self, key: &str, val: &Option<String>, trailing_comma: bool) {
        self.key(key);
        match val {
            Some(s) => self.str_val(s),
            None => self.buf.push_str("null"),
        }
        if trailing_comma {
            self.buf.push(',');
        }
        self.buf.push('\n');
    }

    fn str_field(&mut self, key: &str, val: &str, trailing_comma: bool) {
        self.key(key);
        self.str_val(val);
        if trailing_comma {
            self.buf.push(',');
        }
        self.buf.push('\n');
    }

    fn u32_field(&mut self, key: &str, val: u32, trailing_comma: bool) {
        self.key(key);
        self.buf.push_str(&val.to_string());
        if trailing_comma {
            self.buf.push(',');
        }
        self.buf.push('\n');
    }

    fn u64_field(&mut self, key: &str, val: u64, trailing_comma: bool) {
        self.key(key);
        self.buf.push_str(&val.to_string());
        if trailing_comma {
            self.buf.push(',');
        }
        self.buf.push('\n');
    }

    fn bool_field(&mut self, key: &str, val: bool, trailing_comma: bool) {
        self.key(key);
        self.buf.push_str(if val { "true" } else { "false" });
        if trailing_comma {
            self.buf.push(',');
        }
        self.buf.push('\n');
    }

    fn opt_u32(&mut self, key: &str, val: Option<u32>, trailing_comma: bool) {
        self.key(key);
        match val {
            Some(v) => self.buf.push_str(&v.to_string()),
            None => self.buf.push_str("null"),
        }
        if trailing_comma {
            self.buf.push(',');
        }
        self.buf.push('\n');
    }

    fn opt_i32(&mut self, key: &str, val: Option<i32>, trailing_comma: bool) {
        self.key(key);
        match val {
            Some(v) => self.buf.push_str(&v.to_string()),
            None => self.buf.push_str("null"),
        }
        if trailing_comma {
            self.buf.push(',');
        }
        self.buf.push('\n');
    }

    fn string_array(&mut self, key: &str, vals: &[String], trailing_comma: bool) {
        self.key(key);
        if vals.is_empty() {
            self.buf.push_str("[]");
        } else {
            self.buf.push_str("[\n");
            self.indent += 1;
            for (i, v) in vals.iter().enumerate() {
                self.indent();
                self.str_val(v);
                if i + 1 < vals.len() {
                    self.buf.push(',');
                }
                self.buf.push('\n');
            }
            self.indent -= 1;
            self.indent();
            self.buf.push(']');
        }
        if trailing_comma {
            self.buf.push(',');
        }
        self.buf.push('\n');
    }

    fn begin_obj(&mut self) {
        self.buf.push_str("{\n");
        self.indent += 1;
    }

    fn end_obj(&mut self) {
        self.indent -= 1;
        self.indent();
        self.buf.push('}');
    }

    fn begin_arr(&mut self, key: &str) {
        self.key(key);
        self.buf.push_str("[\n");
        self.indent += 1;
    }

    fn end_arr(&mut self, trailing_comma: bool) {
        self.indent -= 1;
        self.indent();
        self.buf.push(']');
        if trailing_comma {
            self.buf.push(',');
        }
        self.buf.push('\n');
    }

    fn write_topology(&mut self, t: &Topology) {
        self.begin_obj();
        self.begin_arr("pci_roots");
        for (i, root) in t.pci_roots.iter().enumerate() {
            self.indent();
            self.write_pci_root(root);
            if i + 1 < t.pci_roots.len() {
                self.buf.push(',');
            }
            self.buf.push('\n');
        }
        self.end_arr(true);
        self.begin_arr("fabrics_controllers");
        for (i, ctrl) in t.fabrics_controllers.iter().enumerate() {
            self.indent();
            self.write_nvme_ctrl(ctrl);
            if i + 1 < t.fabrics_controllers.len() {
                self.buf.push(',');
            }
            self.buf.push('\n');
        }
        self.end_arr(true);
        self.begin_arr("nvme_subsystems");
        for (i, s) in t.nvme_subsystems.iter().enumerate() {
            self.indent();
            self.write_nvme_subsystem(s);
            if i + 1 < t.nvme_subsystems.len() {
                self.buf.push(',');
            }
            self.buf.push('\n');
        }
        self.end_arr(true);
        self.begin_arr("dm_devices");
        for (i, dm) in t.dm_devices.iter().enumerate() {
            self.indent();
            self.write_dm_device(dm);
            if i + 1 < t.dm_devices.len() {
                self.buf.push(',');
            }
            self.buf.push('\n');
        }
        self.end_arr(false);
        self.end_obj();
    }

    fn write_pci_root(&mut self, r: &PciRoot) {
        self.begin_obj();
        self.str_field("domain", &r.domain, true);
        self.begin_arr("children");
        for (i, node) in r.children.iter().enumerate() {
            self.indent();
            self.write_pci_node(node);
            if i + 1 < r.children.len() {
                self.buf.push(',');
            }
            self.buf.push('\n');
        }
        self.end_arr(false);
        self.end_obj();
    }

    fn write_pci_node(&mut self, node: &PciNode) {
        match node {
            PciNode::Bridge(b) => self.write_pci_bridge(b),
            PciNode::StorageController(c) => self.write_storage_controller(c),
        }
    }

    fn write_pci_bridge(&mut self, b: &PciBridge) {
        self.begin_obj();
        self.str_field("type", "Bridge", true);
        self.str_field("address", &b.address, true);
        self.str_field("vendor_id", &b.vendor_id, true);
        self.str_field("device_id", &b.device_id, true);
        self.opt_str("driver", &b.driver, true);
        self.begin_arr("children");
        for (i, child) in b.children.iter().enumerate() {
            self.indent();
            self.write_pci_node(child);
            if i + 1 < b.children.len() {
                self.buf.push(',');
            }
            self.buf.push('\n');
        }
        self.end_arr(false);
        self.end_obj();
    }

    fn write_storage_controller(&mut self, c: &StorageController) {
        self.begin_obj();
        self.str_field("type", "StorageController", true);
        self.str_field("address", &c.address, true);
        self.str_field("vendor_id", &c.vendor_id, true);
        self.str_field("device_id", &c.device_id, true);
        self.str_field("class_code", &c.class_code, true);
        self.opt_str("driver", &c.driver, true);
        self.opt_i32("numa_node", c.numa_node, true);
        self.begin_arr("children");
        for (i, child) in c.children.iter().enumerate() {
            self.indent();
            self.write_controller_child(child);
            if i + 1 < c.children.len() {
                self.buf.push(',');
            }
            self.buf.push('\n');
        }
        self.end_arr(false);
        self.end_obj();
    }

    fn write_controller_child(&mut self, child: &ControllerChild) {
        match child {
            ControllerChild::ScsiHost(h) => {
                self.begin_obj();
                self.str_field("type", "ScsiHost", true);
                self.write_scsi_host_fields(h);
                self.end_obj();
            }
            ControllerChild::NvmeController(n) => {
                self.begin_obj();
                self.str_field("type", "NvmeController", true);
                self.write_nvme_ctrl_fields(n);
                self.end_obj();
            }
        }
    }

    fn write_scsi_host_fields(&mut self, h: &ScsiHost) {
        self.u32_field("host_num", h.host_num, true);
        self.opt_str("proc_name", &h.proc_name, true);
        self.opt_str("ata_port", &h.ata_port, true);
        self.key("transport");
        match &h.transport {
            Some(t) => {
                self.write_transport_info(t);
                self.buf.push_str(",\n");
            }
            None => self.buf.push_str("null,\n"),
        }
        self.begin_arr("targets");
        for (i, t) in h.targets.iter().enumerate() {
            self.indent();
            self.write_scsi_target(t);
            if i + 1 < h.targets.len() {
                self.buf.push(',');
            }
            self.buf.push('\n');
        }
        self.end_arr(false);
    }

    fn write_scsi_target(&mut self, t: &ScsiTarget) {
        self.begin_obj();
        self.str_field("address", &t.address, true);
        self.begin_arr("devices");
        for (i, d) in t.devices.iter().enumerate() {
            self.indent();
            self.write_scsi_device(d);
            if i + 1 < t.devices.len() {
                self.buf.push(',');
            }
            self.buf.push('\n');
        }
        self.end_arr(false);
        self.end_obj();
    }

    fn write_scsi_device(&mut self, d: &ScsiDevice) {
        self.begin_obj();
        self.str_field("hctl", &d.hctl, true);
        self.str_field("vendor", &d.vendor, true);
        self.str_field("model", &d.model, true);
        self.str_field("rev", &d.rev, true);
        self.u32_field("scsi_type", d.scsi_type, true);
        self.str_field("scsi_type_name", &d.scsi_type_name, true);
        self.key("block_device");
        match &d.block_device {
            Some(bd) => {
                self.write_block_device(bd);
                self.buf.push_str(",\n");
            }
            None => {
                self.buf.push_str("null,\n");
            }
        }
        self.opt_str("sg_device", &d.sg_device, false);
        self.end_obj();
    }

    fn write_nvme_ctrl(&mut self, n: &NvmeCtrl) {
        self.begin_obj();
        self.write_nvme_ctrl_fields(n);
        self.end_obj();
    }

    fn write_nvme_ctrl_fields(&mut self, n: &NvmeCtrl) {
        self.str_field("name", &n.name, true);
        self.str_field("model", &n.model, true);
        self.str_field("serial", &n.serial, true);
        self.str_field("firmware_rev", &n.firmware_rev, true);
        self.str_field("transport", &n.transport, true);
        self.str_field("state", &n.state, true);
        self.opt_str("pci_address", &n.pci_address, true);
        self.opt_str("transport_address", &n.transport_address, true);
        self.opt_str("subsys_name", &n.subsys_name, true);
        self.begin_arr("namespaces");
        for (i, ns) in n.namespaces.iter().enumerate() {
            self.indent();
            self.write_nvme_namespace(ns);
            if i + 1 < n.namespaces.len() {
                self.buf.push(',');
            }
            self.buf.push('\n');
        }
        self.end_arr(false);
    }

    fn write_nvme_namespace(&mut self, ns: &NvmeNamespace) {
        self.begin_obj();
        self.str_field("name", &ns.name, true);
        self.opt_u32("nsid", ns.nsid, true);
        self.opt_str("nguid", &ns.nguid, true);
        self.u64_field("size_bytes", ns.size_bytes, true);
        self.key("block_device");
        match &ns.block_device {
            Some(bd) => {
                self.write_block_device(bd);
                self.buf.push('\n');
            }
            None => {
                self.buf.push_str("null\n");
            }
        }
        self.end_obj();
    }

    fn write_nvme_subsystem(&mut self, s: &NvmeSubsystem) {
        self.begin_obj();
        self.str_field("name", &s.name, true);
        self.opt_str("nqn", &s.nqn, true);
        self.str_field("model", &s.model, true);
        self.str_field("serial", &s.serial, true);
        self.string_array("controllers", &s.controllers, true);
        self.begin_arr("namespaces");
        for (i, ns) in s.namespaces.iter().enumerate() {
            self.indent();
            self.write_nvme_namespace(ns);
            if i + 1 < s.namespaces.len() {
                self.buf.push(',');
            }
            self.buf.push('\n');
        }
        self.end_arr(false);
        self.end_obj();
    }

    fn write_block_device(&mut self, bd: &BlockDevice) {
        self.begin_obj();
        self.str_field("name", &bd.name, true);
        self.str_field("dev_path", &bd.dev_path, true);
        self.u64_field("size_bytes", bd.size_bytes, true);
        self.bool_field("removable", bd.removable, true);
        self.bool_field("read_only", bd.read_only, true);
        self.opt_str("wwn", &bd.wwn, true);
        self.begin_arr("partitions");
        for (i, p) in bd.partitions.iter().enumerate() {
            self.indent();
            self.write_partition(p);
            if i + 1 < bd.partitions.len() {
                self.buf.push(',');
            }
            self.buf.push('\n');
        }
        self.end_arr(true);
        self.string_array("holders", &bd.holders, false);
        self.end_obj();
    }

    fn write_partition(&mut self, p: &Partition) {
        self.begin_obj();
        self.str_field("name", &p.name, true);
        self.u32_field("number", p.number, true);
        self.u64_field("size_bytes", p.size_bytes, true);
        self.string_array("holders", &p.holders, false);
        self.end_obj();
    }

    fn write_transport_info(&mut self, t: &TransportInfo) {
        self.begin_obj();
        match t {
            TransportInfo::Fc(fc) => {
                self.str_field("transport_type", "Fc", true);
                self.str_field("port_name", &fc.port_name, true);
                self.str_field("node_name", &fc.node_name, true);
                self.str_field("port_state", &fc.port_state, true);
                self.str_field("port_type", &fc.port_type, true);
                self.str_field("speed", &fc.speed, true);
                self.opt_str("fabric_name", &fc.fabric_name, true);
                self.opt_str("supported_speeds", &fc.supported_speeds, false);
            }
            TransportInfo::Sas(sas) => {
                self.str_field("transport_type", "Sas", true);
                self.begin_arr("phys");
                for (i, phy) in sas.phys.iter().enumerate() {
                    self.indent();
                    self.write_sas_phy(phy);
                    if i + 1 < sas.phys.len() {
                        self.buf.push(',');
                    }
                    self.buf.push('\n');
                }
                self.end_arr(false);
            }
            TransportInfo::Iscsi(iscsi) => {
                self.str_field("transport_type", "Iscsi", true);
                self.begin_arr("sessions");
                for (i, session) in iscsi.sessions.iter().enumerate() {
                    self.indent();
                    self.write_iscsi_session(session);
                    if i + 1 < iscsi.sessions.len() {
                        self.buf.push(',');
                    }
                    self.buf.push('\n');
                }
                self.end_arr(false);
            }
        }
        self.end_obj();
    }

    fn write_sas_phy(&mut self, phy: &SasPhyInfo) {
        self.begin_obj();
        self.str_field("name", &phy.name, true);
        self.str_field("sas_address", &phy.sas_address, true);
        self.str_field("negotiated_linkrate", &phy.negotiated_linkrate, true);
        self.opt_str("initiator_protocols", &phy.initiator_protocols, true);
        self.opt_str("target_protocols", &phy.target_protocols, false);
        self.end_obj();
    }

    fn write_iscsi_session(&mut self, s: &IscsiSessionInfo) {
        self.begin_obj();
        self.str_field("target_name", &s.target_name, true);
        self.str_field("state", &s.state, true);
        self.opt_str("target_address", &s.target_address, true);
        self.key("target_port");
        match s.target_port {
            Some(p) => self.buf.push_str(&p.to_string()),
            None => self.buf.push_str("null"),
        }
        self.buf.push('\n');
        self.end_obj();
    }

    fn write_dm_slave(&mut self, s: &DmSlave) {
        self.begin_obj();
        self.str_field("device_name", &s.device_name, true);
        self.opt_str("hctl", &s.hctl, true);
        self.opt_u32("host_num", s.host_num, true);
        self.opt_str("host_driver", &s.host_driver, true);
        self.opt_str("pci_address", &s.pci_address, false);
        self.end_obj();
    }

    fn write_dm_device(&mut self, dm: &DmDevice) {
        self.begin_obj();
        self.str_field("name", &dm.name, true);
        self.str_field("dm_name", &dm.dm_name, true);
        self.str_field("dm_uuid", &dm.dm_uuid, true);
        self.str_field("dm_type", &dm.dm_type, true);
        self.u64_field("size_bytes", dm.size_bytes, true);
        self.begin_arr("slaves");
        for (i, s) in dm.slaves.iter().enumerate() {
            self.indent();
            self.write_dm_slave(s);
            if i + 1 < dm.slaves.len() {
                self.buf.push(',');
            }
            self.buf.push('\n');
        }
        self.end_arr(true);
        self.begin_arr("partitions");
        for (i, p) in dm.partitions.iter().enumerate() {
            self.indent();
            self.write_partition(p);
            if i + 1 < dm.partitions.len() {
                self.buf.push(',');
            }
            self.buf.push('\n');
        }
        self.end_arr(false);
        self.end_obj();
    }
}

pub fn to_json(topology: &Topology) -> String {
    let mut w = JsonWriter::new();
    w.write_topology(topology);
    w.buf
}
