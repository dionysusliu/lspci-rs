use super::{read_dword, read_word};
use crate::ConfigSpaceSnapshot;

const ROOT_PORT: u8 = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcieCapability {
    pub version: u8,
    /// device/port type code: 0 endpoint, 4 root port, 5/6 switch ports,
    /// 7/8 bridges, 9 root-complex integrated endpoint, 0xa event collector
    pub device_type: u8,
    pub slot_implemented: bool,
    pub interrupt_message_number: u8,
    pub dev_ctl: u16,
    pub dev_sta: u16,
    /// gen code: 1 = 2.5GT/s, 2 = 5, 3 = 8, 4 = 16, 5 = 32, 6 = 64
    pub link_max_speed: u8,
    pub link_max_width: u8,
    pub link_target_speed: u8,
    pub link_current_speed: u8,
    pub link_current_width: u8,
    pub link_training: bool,
    pub slot_ctl: Option<u16>,
    pub slot_sta: Option<u16>,
    pub root_ctl: Option<u16>,
    pub root_sta: Option<u32>,
}

pub fn decode_pcie(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<PcieCapability> {
    let base = u32::from(offset);
    let flags = read_word(snapshot, base + 2).ok()?;
    let device_type = ((flags >> 4) & 0x000f) as u8;
    let slot_implemented = flags & 0x0100 != 0;

    let dev_ctl = read_word(snapshot, base + 8).ok()?;
    let dev_sta = read_word(snapshot, base + 0x0a).ok()?;

    let link_cap = read_dword(snapshot, base + 0x0c).ok()?;
    let link_ctl = read_word(snapshot, base + 0x10).ok()?;
    let link_sta = read_word(snapshot, base + 0x12).ok()?;

    let (slot_ctl, slot_sta) = if slot_implemented {
        (
            Some(read_word(snapshot, base + 0x18).ok()?),
            Some(read_word(snapshot, base + 0x1a).ok()?),
        )
    } else {
        (None, None)
    };

    let (root_ctl, root_sta) = if device_type == ROOT_PORT {
        (
            Some(read_word(snapshot, base + 0x1c).ok()?),
            Some(read_dword(snapshot, base + 0x20).ok()?),
        )
    } else {
        (None, None)
    };

    Some(PcieCapability {
        version: (flags & 0x000f) as u8,
        device_type,
        slot_implemented,
        interrupt_message_number: ((flags >> 9) & 0x001f) as u8,
        dev_ctl,
        dev_sta,
        link_max_speed: (link_cap & 0x0000_000f) as u8,
        link_max_width: ((link_cap >> 4) & 0x003f) as u8,
        link_target_speed: (link_ctl & 0x000f) as u8,
        link_current_speed: (link_sta & 0x000f) as u8,
        link_current_width: ((link_sta >> 4) & 0x003f) as u8,
        link_training: link_sta & 0x0800 != 0,
        slot_ctl,
        slot_sta,
        root_ctl,
        root_sta,
    })
}
