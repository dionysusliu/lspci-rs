pub mod hot_plug;
pub mod msi;
pub mod msix;
pub mod pci_x;
pub mod pcie;
pub mod pm;
pub mod slot_id;
pub mod vendor;
pub mod vpd;

pub use hot_plug::HotPlugCapability;
pub use msi::MsiCapability;
pub use msix::MsiXCapability;
pub use pci_x::PciXCapability;
pub use pcie::PcieCapability;
pub use pm::PmCapability;
pub use slot_id::SlotIdCapability;
pub use vendor::VendorSpecificCapability;
pub use vpd::VpdCapability;

use crate::{ConfigReadFailure, ConfigSpaceSnapshot, PciCapability, PciCapabilityState};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PciCapabilityContent {
    Pm(PmCapability),
    Msi(MsiCapability),
    MsiX(MsiXCapability),
    Pcie(PcieCapability),
    VendorSpecific(VendorSpecificCapability),
    SlotId(SlotIdCapability),
    HotPlug(HotPlugCapability),
    Vpd(VpdCapability),
    PciX(PciXCapability),
}

pub(crate) fn read_word(
    snapshot: &ConfigSpaceSnapshot,
    offset: u32,
) -> Result<u16, ConfigReadFailure> {
    let bytes = snapshot.read(offset, 2)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

pub(crate) fn read_dword(
    snapshot: &ConfigSpaceSnapshot,
    offset: u32,
) -> Result<u32, ConfigReadFailure> {
    let bytes = snapshot.read(offset, 4)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

pub(crate) fn decode_content(snapshot: &ConfigSpaceSnapshot, capability: &mut PciCapability) {
    if !matches!(capability.state, PciCapabilityState::Valid) {
        return;
    }

    let offset = capability.offset;
    capability.content = match capability.id {
        0x01 => pm::decode_pm(snapshot, offset).map(PciCapabilityContent::Pm),
        0x03 => vpd::decode_vpd(snapshot, offset).map(PciCapabilityContent::Vpd),
        0x04 => slot_id::decode_slot_id(snapshot, offset).map(PciCapabilityContent::SlotId),
        0x05 => msi::decode_msi(snapshot, offset).map(PciCapabilityContent::Msi),
        0x09 => vendor::decode_vendor_specific(snapshot, offset)
            .map(PciCapabilityContent::VendorSpecific),
        0x07 => pci_x::decode_pci_x(snapshot, offset).map(PciCapabilityContent::PciX),
        0x0c => hot_plug::decode_hot_plug(snapshot, offset).map(PciCapabilityContent::HotPlug),
        0x10 => pcie::decode_pcie(snapshot, offset).map(PciCapabilityContent::Pcie),
        0x11 => msix::decode_msix(snapshot, offset).map(PciCapabilityContent::MsiX),
        _ => None,
    };
}
