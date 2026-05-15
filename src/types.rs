// SPDX-License-Identifier: MIT
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Topology {
    pub pci_roots: Vec<PciRoot>,
    pub fabrics_controllers: Vec<NvmeCtrl>,
    pub nvme_subsystems: Vec<NvmeSubsystem>,
    pub dm_devices: Vec<DmDevice>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct PciRoot {
    pub domain: String,
    pub children: Vec<PciNode>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(tag = "type"))]
pub enum PciNode {
    Bridge(PciBridge),
    StorageController(StorageController),
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct PciBridge {
    pub address: String,
    pub vendor_id: String,
    pub device_id: String,
    pub driver: Option<String>,
    pub children: Vec<PciNode>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct StorageController {
    pub address: String,
    pub vendor_id: String,
    pub device_id: String,
    pub class_code: String,
    pub driver: Option<String>,
    pub numa_node: Option<i32>,
    pub children: Vec<ControllerChild>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(tag = "type"))]
pub enum ControllerChild {
    ScsiHost(ScsiHost),
    NvmeController(NvmeCtrl),
}

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ScsiHost {
    pub host_num: u32,
    pub proc_name: Option<String>,
    pub ata_port: Option<String>,
    pub transport: Option<TransportInfo>,
    pub targets: Vec<ScsiTarget>,
}

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(tag = "transport_type"))]
pub enum TransportInfo {
    Fc(FcHostInfo),
    Sas(SasHostInfo),
    Iscsi(IscsiHostInfo),
}

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct FcHostInfo {
    pub port_name: String,
    pub node_name: String,
    pub port_state: String,
    pub port_type: String,
    pub speed: String,
    pub fabric_name: Option<String>,
    pub supported_speeds: Option<String>,
}

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct SasHostInfo {
    pub phys: Vec<SasPhyInfo>,
}

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct SasPhyInfo {
    pub name: String,
    pub sas_address: String,
    pub negotiated_linkrate: String,
    pub initiator_protocols: Option<String>,
    pub target_protocols: Option<String>,
}

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct IscsiHostInfo {
    pub sessions: Vec<IscsiSessionInfo>,
}

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct IscsiSessionInfo {
    pub target_name: String,
    pub state: String,
    pub target_address: Option<String>,
    pub target_port: Option<u16>,
}

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ScsiTarget {
    pub address: String,
    pub devices: Vec<ScsiDevice>,
}

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ScsiDevice {
    pub hctl: String,
    pub vendor: String,
    pub model: String,
    pub rev: String,
    pub scsi_type: u32,
    pub scsi_type_name: String,
    pub block_device: Option<BlockDevice>,
    pub sg_device: Option<String>,
}

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct NvmeCtrl {
    pub name: String,
    pub model: String,
    pub serial: String,
    pub firmware_rev: String,
    pub transport: String,
    pub state: String,
    pub pci_address: Option<String>,
    pub transport_address: Option<String>,
    pub subsys_name: Option<String>,
    pub namespaces: Vec<NvmeNamespace>,
}

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct NvmeNamespace {
    pub name: String,
    pub nsid: Option<u32>,
    pub nguid: Option<String>,
    pub size_bytes: u64,
    pub block_device: Option<BlockDevice>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct NvmeSubsystem {
    pub name: String,
    pub nqn: Option<String>,
    pub model: String,
    pub serial: String,
    pub controllers: Vec<String>,
    pub namespaces: Vec<NvmeNamespace>,
}

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct BlockDevice {
    pub name: String,
    pub dev_path: String,
    pub size_bytes: u64,
    pub removable: bool,
    pub read_only: bool,
    pub wwn: Option<String>,
    pub partitions: Vec<Partition>,
    pub holders: Vec<String>,
}

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Partition {
    pub name: String,
    pub number: u32,
    pub size_bytes: u64,
    pub holders: Vec<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct DmSlave {
    pub device_name: String,
    pub hctl: Option<String>,
    pub host_num: Option<u32>,
    pub host_driver: Option<String>,
    pub pci_address: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct DmDevice {
    pub name: String,
    pub dm_name: String,
    pub dm_uuid: String,
    pub dm_type: String,
    pub size_bytes: u64,
    pub slaves: Vec<DmSlave>,
    pub partitions: Vec<Partition>,
}
