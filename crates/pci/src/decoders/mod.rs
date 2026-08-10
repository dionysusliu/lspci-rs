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

use crate::{ConfigReadFailure, ConfigSpaceSnapshot};

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
