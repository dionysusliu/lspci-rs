use std::{error::Error, fmt::Display};

use crate::device::PciAddress;

#[derive(Debug)]
pub enum PciError {
    Allocation,
    DeviceInfo {
        address: PciAddress,
        known_fields: u32,
        requested_fields: u32,
    },
    Message(String),
}

impl Error for PciError {}

impl Display for PciError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Allocation => {
                write!(f, "failed to allocate libpci context")
            }
            Self::DeviceInfo {
                address,
                known_fields,
                requested_fields,
            } => {
                write!(
                    f,
                    "failed to fill PCI device {address}: \
                    known fields=0x{known_fields:08x}, \
                    requested fields=0x{requested_fields:08x}"
                )
            }
            Self::Message(message) => f.write_str(message),
        }
    }
}
