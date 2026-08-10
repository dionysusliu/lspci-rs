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
    pub cache_line_size: PciField<u8>,
    pub latency_timer: PciField<u8>,
    pub header_type: PciField<crate::PciHeaderType>,
    pub bist: PciField<crate::PciBist>,
    pub expansion_rom: PciField<crate::PciExpansionRom>,
    pub interrupt_line: PciField<u8>,
    pub interrupt_pin: PciField<crate::PciInterruptPin>,
    pub cardbus_cis_pointer: PciField<u32>,
    pub bridge: PciField<crate::PciBridgeHeader>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PciInspection {
    pub device: PciDevice,
    pub details: PciDeviceDetails,
}
