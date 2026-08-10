use crate::{
    PciAddress, PciDevice,
    field::{PciCapabilityReport, PciField},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PciResource {
    pub index: u8,
    pub start: u64,
    pub size: u64,
    pub flags: u64,
    pub bar_type: Option<crate::PciBarType>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PciDeviceDetails {
    pub revision: PciField<u8>,
    pub programming_interface: PciField<u8>,
    pub subsystem_vendor_id: PciField<u16>,
    pub subsystem_device_id: PciField<u16>,
    pub parent: PciField<PciAddress>,
    pub irq: PciField<u32>,
    pub driver: PciField<String>,
    pub resources: PciField<Vec<PciResource>>,
    pub capabilities: PciField<PciCapabilityReport>,
    pub command: PciField<crate::CommandRegister>,
    pub status: PciField<crate::StatusRegister>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PciInspection {
    pub device: PciDevice,
    pub details: PciDeviceDetails,
}
