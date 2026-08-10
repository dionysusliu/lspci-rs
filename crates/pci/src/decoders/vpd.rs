use super::read_word;
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VpdCapability {
    pub address_flag: bool,
    pub address: u16,
    pub data: u16,
}

pub fn decode_vpd(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<VpdCapability> {
    let base = u32::from(offset);
    let address_register = read_word(snapshot, base + 2).ok()?;
    let data = read_word(snapshot, base + 4).ok()?;

    Some(VpdCapability {
        address_flag: address_register & 0x8000 != 0,
        address: address_register & 0x7fff,
        data,
    })
}
