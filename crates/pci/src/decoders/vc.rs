use super::{read_dword, read_word};
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VcResource {
    pub capability: u32,
    pub control: u32,
    pub status: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VcCapability {
    pub extended_vc_count: u8,
    pub port_vc_capability: u8,
    pub reference_clock: u8,
    pub port_arbitration_table_entry_count: u8,
    pub vc_arbitration_table_offset: u8,
    pub vc_arbitration_table_entry_count: u8,
    pub port_control: u16,
    pub port_status: u16,
    pub resources: Vec<VcResource>,
}

pub fn decode_vc(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<VcCapability> {
    let base = u32::from(offset);

    let extended_cap = read_dword(snapshot, base + 4).ok()?;
    let port_cap_1 = read_dword(snapshot, base + 8).ok()?;
    let port_cap_2 = read_dword(snapshot, base + 12).ok()?;
    let port_control = read_word(snapshot, base + 16).ok()?;
    let port_status = read_word(snapshot, base + 18).ok()?;

    let extended_vc_count = ((extended_cap >> 4) & 0x0000_0007) as u8;

    let mut resources = Vec::new();
    for index in 0..=u32::from(extended_vc_count) {
        let entry = base + 20 + index * 12;
        let capability = read_dword(snapshot, entry).ok()?;
        let control = read_dword(snapshot, entry + 4).ok()?;
        let status = read_dword(snapshot, entry + 8).ok()?;
        resources.push(VcResource {
            capability,
            control,
            status,
        });
    }

    Some(VcCapability {
        extended_vc_count,
        port_vc_capability: ((extended_cap >> 8) & 0x0000_0003) as u8,
        reference_clock: (port_cap_1 & 0x0000_00ff) as u8,
        port_arbitration_table_entry_count: ((port_cap_1 >> 8) & 0x0000_00ff) as u8,
        vc_arbitration_table_offset: (port_cap_2 & 0x0000_000f) as u8,
        vc_arbitration_table_entry_count: ((port_cap_2 >> 8) & 0x0000_00ff) as u8,
        port_control,
        port_status,
        resources,
    })
}
