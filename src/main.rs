// SPDX-License-Identifier: MIT
mod block;
#[cfg(not(feature = "serde"))]
mod json;
mod nvme;
mod pci;
mod scsi;
mod sysfs;
mod topology;
mod tree;
mod types;

use topology::BuildArgs;
use types::Topology;

#[cfg(feature = "serde")]
fn render_json(topo: &Topology) -> String {
    serde_json::to_string_pretty(topo).expect("serialization failed")
}

#[cfg(not(feature = "serde"))]
fn render_json(topo: &Topology) -> String {
    json::to_json(topo)
}

struct Args {
    json: bool,
    no_empty: bool,
}

fn parse_args() -> Args {
    let mut args = Args {
        json: false,
        no_empty: false,
    };

    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--json" => args.json = true,
            "--no-empty" => args.no_empty = true,
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            other => {
                eprintln!("Unknown argument: {}", other);
                print_usage();
                std::process::exit(1);
            }
        }
    }

    args
}

fn print_usage() {
    eprintln!(
        "Usage: pathological [OPTIONS]

Discover and display end-to-end storage path topology from sysfs.

Options:
  --json       Output as JSON instead of ASCII tree
  --no-empty   Hide PCI branches and SCSI hosts with no devices
  -h, --help   Show this help message"
    );
}

fn main() {
    let args = parse_args();

    // Phase 1: Discover block devices and DM devices
    let (block_devices, dm_devices) = block::discover();

    // Phase 2: Discover NVMe controllers and subsystems
    let (nvme_controllers, nvme_subsystems) = nvme::discover(&block_devices);
    let nvme_pci_addrs = nvme::pci_addresses(&nvme_controllers);

    // Phase 3: Discover SCSI hosts and devices
    let (scsi_hosts_by_pci, scsi_pci_addrs) = scsi::discover(&block_devices);

    // Phase 4: Build PCI hierarchy
    let pci_roots = pci::discover_pci_hierarchy(&nvme_pci_addrs, &scsi_pci_addrs);

    // Phase 5: Assemble topology
    let build_args = BuildArgs {
        no_empty: args.no_empty,
    };
    let topo = topology::build(
        pci_roots,
        nvme_controllers,
        scsi_hosts_by_pci,
        nvme_subsystems,
        dm_devices,
        &build_args,
    );

    // Phase 6: Render output
    if args.json {
        println!("{}", render_json(&topo));
    } else {
        let output = tree::render(&topo);
        print!("{}", output);
    }
}
