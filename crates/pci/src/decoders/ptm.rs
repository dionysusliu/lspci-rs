use super::read_dword;
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PtmCapability {
    pub root_capable: bool,
    pub clock_capable: bool,
    pub enable: bool,
    pub root_select: bool,
    pub granularity: u8,
}

pub fn decode_ptm(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<PtmCapability> {
    let base = u32::from(offset);
    let capability = read_dword(snapshot, base + 4).ok()?;
    let control = read_dword(snapshot, base + 8).ok()?;

    Some(PtmCapability {
        root_capable: capability & 0x0000_0001 != 0,
        clock_capable: capability & 0x0000_0002 != 0,
        enable: control & 0x0000_0001 != 0,
        root_select: control & 0x0000_0002 != 0,
        granularity: ((control >> 24) & 0x0000_00ff) as u8,
    })
}
