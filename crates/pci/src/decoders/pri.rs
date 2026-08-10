use super::{read_dword, read_word};
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PriCapability {
    pub enable: bool,
    pub reset: bool,
    pub response_failure: bool,
    pub unexpected_group_index: bool,
    pub stopped: bool,
    pub outstanding_capacity: u32,
    pub outstanding_allocation: u32,
}

pub fn decode_pri(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<PriCapability> {
    let base = u32::from(offset);
    let control = read_word(snapshot, base + 4).ok()?;
    let status = read_word(snapshot, base + 6).ok()?;
    let outstanding_capacity = read_dword(snapshot, base + 8).ok()?;
    let outstanding_allocation = read_dword(snapshot, base + 12).ok()?;

    Some(PriCapability {
        enable: control & 0x0001 != 0,
        reset: control & 0x0002 != 0,
        response_failure: status & 0x0001 != 0,
        unexpected_group_index: status & 0x0002 != 0,
        stopped: status & 0x0004 != 0,
        outstanding_capacity,
        outstanding_allocation,
    })
}
