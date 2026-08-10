pub mod acs;
pub mod aer;
pub mod ari;
pub mod ats;
pub mod dpc;
pub mod dsn;
pub mod hot_plug;
pub mod ltr;
pub mod msi;
pub mod msix;
pub mod pasid;
pub mod pci_x;
pub mod pcie;
pub mod pm;
pub mod pri;
pub mod ptm;
pub mod slot_id;
pub mod sriov;
pub mod tph;
pub mod vendor;
pub mod vpd;

pub use acs::AcsCapability;
pub use aer::AerCapability;
pub use ari::AriCapability;
pub use ats::AtsCapability;
pub use dpc::DpcCapability;
pub use dsn::DsnCapability;
pub use hot_plug::HotPlugCapability;
pub use ltr::LtrCapability;
pub use msi::MsiCapability;
pub use msix::MsiXCapability;
pub use pasid::PasidCapability;
pub use pci_x::PciXCapability;
pub use pcie::PcieCapability;
pub use pm::PmCapability;
pub use pri::PriCapability;
pub use ptm::PtmCapability;
pub use slot_id::SlotIdCapability;
pub use sriov::SriovCapability;
pub use tph::TphCapability;
pub use vendor::VendorSpecificCapability;
pub use vpd::VpdCapability;

use crate::{
    ConfigReadFailure, ConfigSpaceSnapshot, PciCapability, PciCapabilityKind, PciCapabilityState,
};

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
    Dsn(DsnCapability),
    Ari(AriCapability),
    Acs(AcsCapability),
    Sriov(SriovCapability),
    Aer(AerCapability),
    Ltr(LtrCapability),
    Ats(AtsCapability),
    Pri(PriCapability),
    Pasid(PasidCapability),
    Ptm(PtmCapability),
    Dpc(DpcCapability),
    Tph(TphCapability),
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
    capability.content = match (&capability.kind, capability.id) {
        (PciCapabilityKind::Standard, 0x01) => {
            pm::decode_pm(snapshot, offset).map(PciCapabilityContent::Pm)
        }
        (PciCapabilityKind::Standard, 0x03) => {
            vpd::decode_vpd(snapshot, offset).map(PciCapabilityContent::Vpd)
        }
        (PciCapabilityKind::Standard, 0x04) => {
            slot_id::decode_slot_id(snapshot, offset).map(PciCapabilityContent::SlotId)
        }
        (PciCapabilityKind::Standard, 0x05) => {
            msi::decode_msi(snapshot, offset).map(PciCapabilityContent::Msi)
        }
        (PciCapabilityKind::Standard, 0x07) => {
            pci_x::decode_pci_x(snapshot, offset).map(PciCapabilityContent::PciX)
        }
        (PciCapabilityKind::Standard, 0x09) => vendor::decode_vendor_specific(snapshot, offset)
            .map(PciCapabilityContent::VendorSpecific),
        (PciCapabilityKind::Standard, 0x0c) => {
            hot_plug::decode_hot_plug(snapshot, offset).map(PciCapabilityContent::HotPlug)
        }
        (PciCapabilityKind::Standard, 0x10) => {
            pcie::decode_pcie(snapshot, offset).map(PciCapabilityContent::Pcie)
        }
        (PciCapabilityKind::Standard, 0x11) => {
            msix::decode_msix(snapshot, offset).map(PciCapabilityContent::MsiX)
        }
        (PciCapabilityKind::Extended, 0x03) => {
            dsn::decode_dsn(snapshot, offset).map(PciCapabilityContent::Dsn)
        }
        (PciCapabilityKind::Extended, 0x0d) => {
            acs::decode_acs(snapshot, offset).map(PciCapabilityContent::Acs)
        }
        (PciCapabilityKind::Extended, 0x0e) => {
            ari::decode_ari(snapshot, offset).map(PciCapabilityContent::Ari)
        }
        (PciCapabilityKind::Extended, 0x01) => {
            aer::decode_aer(snapshot, offset).map(PciCapabilityContent::Aer)
        }
        (PciCapabilityKind::Extended, 0x10) => {
            sriov::decode_sriov(snapshot, offset).map(PciCapabilityContent::Sriov)
        }
        (PciCapabilityKind::Extended, 0x17) => {
            tph::decode_tph(snapshot, offset).map(PciCapabilityContent::Tph)
        }
        (PciCapabilityKind::Extended, 0x1d) => {
            dpc::decode_dpc(snapshot, offset).map(PciCapabilityContent::Dpc)
        }
        (PciCapabilityKind::Extended, 0x0f) => {
            ats::decode_ats(snapshot, offset).map(PciCapabilityContent::Ats)
        }
        (PciCapabilityKind::Extended, 0x13) => {
            pri::decode_pri(snapshot, offset).map(PciCapabilityContent::Pri)
        }
        (PciCapabilityKind::Extended, 0x18) => {
            ltr::decode_ltr(snapshot, offset).map(PciCapabilityContent::Ltr)
        }
        (PciCapabilityKind::Extended, 0x1b) => {
            pasid::decode_pasid(snapshot, offset).map(PciCapabilityContent::Pasid)
        }
        (PciCapabilityKind::Extended, 0x1f) => {
            ptm::decode_ptm(snapshot, offset).map(PciCapabilityContent::Ptm)
        }
        _ => None,
    };
}
