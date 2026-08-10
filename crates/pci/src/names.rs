use crate::PciCapabilityKind;

pub fn capability_name(kind: &PciCapabilityKind, id: u16) -> &'static str {
    match kind {
        PciCapabilityKind::Standard => standard_name(id),
        PciCapabilityKind::Extended => extended_name(id),
        PciCapabilityKind::Unknown(_) => "unknown",
    }
}

fn standard_name(id: u16) -> &'static str {
    match id {
        0x01 => "pm",
        0x02 => "agp",
        0x03 => "vpd",
        0x04 => "slot-id",
        0x05 => "msi",
        0x06 => "hot-swap",
        0x07 => "pci-x",
        0x08 => "hypertransport",
        0x09 => "vendor-specific",
        0x0a => "debug-port",
        0x0b => "compactpci-crc",
        0x0c => "hot-plug",
        0x0d => "bridge-subsystem-vendor-id",
        0x0e => "agp-8x",
        0x0f => "secure-device",
        0x10 => "pcie",
        0x11 => "msi-x",
        0x12 => "sata",
        0x13 => "af",
        _ => "unknown",
    }
}

fn extended_name(id: u16) -> &'static str {
    match id {
        0x0001 => "aer",
        0x0002 => "virtual-channel",
        0x0003 => "device-serial-number",
        0x0004 => "power-budgeting",
        0x0005 => "root-complex-link",
        0x0006 => "root-complex-internal-link-control",
        0x0007 => "root-complex-event-collector",
        0x0008 => "mfvc",
        0x000a => "acs",
        0x000b => "ari",
        0x000c => "ats",
        0x000d => "sr-iov",
        0x000e => "mr-iov",
        0x000f => "multicast",
        0x0010 => "pri",
        0x0013 => "tph",
        0x0015 => "ltr",
        0x0017 => "flattening-portal-bridge",
        0x0019 => "secondary-pcie",
        0x001e => "l1-pm-substates",
        0x001f => "ptm",
        0x0023 => "doe",
        _ => "unknown",
    }
}
