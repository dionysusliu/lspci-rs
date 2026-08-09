use std::fmt::{self, Display};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PciAddress {
    pub domain: u16,
    pub bus: u8,
    pub slot: u8,
    pub function: u8,
}

impl Display for PciAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:04x}:{:02x}:{:02x}.{}",
            self.domain, self.bus, self.slot, self.function
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PciDevice {
    pub address: PciAddress,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_id: u16,
    pub vendor_name: String,
    pub device_name: String,
    pub class_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PciSnapshot {
    devices: Vec<PciDevice>,
}

impl PciSnapshot {
    pub fn devices(&self) -> &[PciDevice] {
        self.devices.as_ref()
    }

    pub(crate) fn from_devices(devices: Vec<PciDevice>) -> Self {
        Self { devices }
    }
}
