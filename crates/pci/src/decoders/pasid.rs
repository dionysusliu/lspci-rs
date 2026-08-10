use super::read_word;
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PasidCapability {
    pub execute_supported: bool,
    pub privileged_supported: bool,
    pub max_pasid_width: u8,
    pub enable: bool,
    pub execute_enable: bool,
    pub privileged_enable: bool,
}

pub fn decode_pasid(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<PasidCapability> {
    let base = u32::from(offset);
    let capability = read_word(snapshot, base + 4).ok()?;
    let control = read_word(snapshot, base + 6).ok()?;

    Some(PasidCapability {
        execute_supported: capability & 0x0002 != 0,
        privileged_supported: capability & 0x0004 != 0,
        max_pasid_width: ((capability >> 8) & 0x001f) as u8,
        enable: control & 0x0001 != 0,
        execute_enable: control & 0x0002 != 0,
        privileged_enable: control & 0x0004 != 0,
    })
}
