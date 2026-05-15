# pathological

***because storage paths get weird...***

Discover and display end-to-end storage path topology on Linux by walking
sysfs. Shows the full chain from PCI bus through HBAs, SCSI/NVMe controllers,
and block devices — including transport details, multipath relationships, and
device-mapper stacking.

Zero required dependencies. Single static binary.

## What it shows

- PCI hierarchy: root complexes, bridges, PCIe switches, endpoint controllers
- SCSI hosts with transport info (SAS phys/addresses, FC WWPN/WWNN/speed, iSCSI sessions/portals)
- NVMe controllers with serial, firmware, namespaces, and partitions
- NVMe-oF (Fabrics) controllers in a separate section with transport address
- SCSI generic devices (`/dev/sgN`) alongside block devices
- Device Mapper: multipath, LVM, crypt — with slave-to-host grouping
- Multipath path topology: which HBAs, how many paths, multi-port vs multi-HBA

## Example

Trimmed output from a dual-NUMA server with a SAS HBA (mpt3sas behind a
dual-ported SAS expander), a MegaRAID controller, an HP SmartArray, and two
NVMe SSDs:

```
PCI 0000:00
├── [0000:00:01.0] bridge (driver: pcieport)
│   └── [0000:09:00.0] SAS controller [0x1000 0x0072] (driver: mpt3sas)
│       └── host12 (mpt2sas, SAS)
│           phy-12:0: addr=0x5d4ae520995f8200  rate=Unknown
│           phy-12:2: addr=0x5d4ae520995f8200  rate=3.0 Gbit
│           phy-12:7: addr=0x5d4ae520995f8200  rate=3.0 Gbit
│           ├── target 12:0:0
│           │   └── [12:0:0:0] ATA WDC WD10EFRX-68P (disk, sg9)
│           │       └── sdg (931.5 GiB)
│           │           └── → dm-12
│           ├── target 12:0:1
│           │   └── [12:0:1:0] ATA WDC WD10EFRX-68P (disk, sg10)
│           │       └── sdh (931.5 GiB)
│           │           └── → dm-0
│           ├── target 12:0:14
│           │   └── [12:0:14:0] WD WD4001FYYG-01SL3 (disk, sg23)
│           │       └── sdu (3.6 TiB)
│           │           └── → dm-9
│           └── target 12:0:16
│               └── [12:0:16:0] PROMISE 3U-SAS-16-D BP (enclosure, sg25)
├── [0000:00:02.0] bridge (driver: pcieport)
│   └── [0000:0c:00.0] NVMe controller [0x8086 0x0953] (driver: nvme)
│       └── nvme1: INTEL SSDPEDMD400G4 (S/N: CVFT5194002T400BGN, FW: 8DV101H0)
│           └── nvme1n1 (372.6 GiB)
│               ├── nvme1n1p1 (600.0 MiB)
│               ├── nvme1n1p2 (1.0 GiB)
│               └── nvme1n1p3 (180.0 GiB)
└── [0000:00:03.0] bridge (driver: pcieport)
    └── [0000:03:00.0] RAID controller [0x103c 0x3239] (driver: hpsa)
        └── host10 (hpsa, SAS)
            phy-10:0: addr=0x5001438031742180  rate=Unknown
            └── target 10:1:0
                └── [10:1:0:0] HP LOGICAL VOLUME (disk, sg1)
                    └── sda (279.4 GiB)

PCI 0000:80
├── [0000:80:02.0] bridge (driver: pcieport)
│   └── [0000:81:00.0] RAID controller [0x1000 0x005b] (driver: megaraid_sas)
│       └── host11 (megaraid_sas)
│           ├── target 11:0:12
│           │   └── [11:0:12:0] ATA WDC WD2004FBYZ-0 (disk, sg4)
│           │       └── sdb (1.8 TiB)
│           │           └── → dm-2
│           └── target 11:0:8
│               └── [11:0:8:0] PROMISE 3U-SAS-16-D BP (enclosure, sg2)
└── [0000:80:03.0] bridge (driver: pcieport)
    └── [0000:84:00.0] NVMe controller [0x8086 0x0953] (driver: nvme)
        └── nvme0: INTEL SSDPEDMD400G4 (S/N: CVFT520000BQ400BGN, FW: 8DV101H0)
            └── nvme0n1 (372.6 GiB)

Device Mapper
├── mpathm (Multipath, dm-0, 931.5 GiB, 2 paths via 1 HBA, multi-port)
│   └── host12 (mpt2sas) [0000:09:00.0]
│       ├── sdh [12:0:1:0]
│       └── sdx [12:0:18:0]
├── mpathk (Multipath, dm-9, 3.6 TiB, 2 paths via 1 HBA, multi-port)
│   └── host12 (mpt2sas) [0000:09:00.0]
│       ├── sdak [12:0:31:0]
│       └── sdu [12:0:14:0]
└── mpathai (Multipath, dm-5, 1.8 TiB)
    └── host11 (megaraid_sas) [0000:81:00.0]
        └── sdd [11:0:26:0]
```

## Building

```bash
cargo build --release
```

The binary has zero runtime dependencies by default. Optional serde support
for JSON serialization (the default JSON output uses a hand-rolled emitter):

```bash
cargo build --release --features serde
```

Fully static binary via musl:

```bash
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
```

## Usage

```
pathological [OPTIONS]

Options:
  --json       Output as JSON instead of ASCII tree
  --no-empty   Hide PCI branches and SCSI hosts with no devices
  -h, --help   Show this help message
```

JSON output includes the same topology in a structured format suitable for
programmatic consumption. The hand-rolled JSON emitter produces output
identical to `serde_json::to_string_pretty` when built with `--features serde`.

## Requirements

- Linux (reads from sysfs, so it only works on Linux)
- Root access or sufficient permissions to read `/sys/class/block`,
  `/sys/class/scsi_host`, `/sys/class/nvme`, etc.

## License

[MIT](LICENSE)
