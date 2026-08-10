use super::{read_dword, read_word};
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PciXCapability {
    pub parity_error_recovery: bool,
    pub relaxed_ordering: bool,
    pub max_memory_block: u8,
    pub max_split: u8,
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub status_raw: u32,
}

pub fn decode_pci_x(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<PciXCapability> {
    let base = u32::from(offset);
    let command = read_word(snapshot, base + 2).ok()?;
    let status = read_dword(snapshot, base + 4).ok()?;

    Some(PciXCapability {
        parity_error_recovery: command & 0x0001 != 0,
        relaxed_ordering: command & 0x0002 != 0,
        max_memory_block: ((command >> 2) & 0x0003) as u8,
        max_split: ((command >> 4) & 0x0007) as u8,
        bus: ((status >> 8) & 0xff) as u8,
        device: ((status >> 3) & 0x001f) as u8,
        function: (status & 0x0007) as u8,
        status_raw: status,
    })
}
