pub mod msi;
pub mod msix;
pub mod pcie;
pub mod pm;
pub mod vendor;

pub use msi::MsiCapability;
pub use msix::MsiXCapability;
pub use pcie::PcieCapability;
pub use pm::PmCapability;
pub use vendor::VendorSpecificCapability;

use crate::{ConfigReadFailure, ConfigSpaceSnapshot, PciCapability, PciCapabilityState};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PciCapabilityContent {
    Pm(PmCapability),
    Msi(MsiCapability),
    MsiX(MsiXCapability),
    Pcie(PcieCapability),
    VendorSpecific(VendorSpecificCapability),
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
        0x05 => msi::decode_msi(snapshot, offset).map(PciCapabilityContent::Msi),
        0x09 => vendor::decode_vendor_specific(snapshot, offset)
            .map(PciCapabilityContent::VendorSpecific),
        0x10 => pcie::decode_pcie(snapshot, offset).map(PciCapabilityContent::Pcie),
        0x11 => msix::decode_msix(snapshot, offset).map(PciCapabilityContent::MsiX),
        _ => None,
    };
}
