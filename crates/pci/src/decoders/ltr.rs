use super::read_dword;
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LtrCapability {
    pub snoop_value: u16,
    pub snoop_scale: u8,
    pub no_snoop_value: u16,
    pub no_snoop_scale: u8,
}

pub fn decode_ltr(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<LtrCapability> {
    let base = u32::from(offset);
    let latency = read_dword(snapshot, base + 4).ok()?;

    Some(LtrCapability {
        snoop_value: (latency & 0x0000_03ff) as u16,
        snoop_scale: ((latency >> 10) & 0x0000_0007) as u8,
        no_snoop_value: ((latency >> 16) & 0x0000_03ff) as u16,
        no_snoop_scale: ((latency >> 26) & 0x0000_0007) as u8,
    })
}
