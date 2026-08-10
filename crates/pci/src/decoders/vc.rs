use super::{read_dword, read_word};
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VcResource {
    pub control: u32,
    pub status: u32,
    pub capability: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VcCapability {
    pub evc_count: u8,
    pub lpevc: u8,
    pub reference_clock: u8,
    pub pat_entry_bits: u8,
    pub arbitration_table_position: u8,
    pub port_control: u16,
    pub port_status: u16,
    pub resources: Vec<VcResource>,
}

pub fn decode_vc(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<VcCapability> {
    let base = u32::from(offset);

    let cr1 = read_dword(snapshot, base + 4).ok()?;
    let cr2 = read_dword(snapshot, base + 8).ok()?;
    let port_control = read_word(snapshot, base + 16).ok()?;
    let port_status = read_word(snapshot, base + 18).ok()?;

    let lpevc = ((cr1 >> 4) & 0x0000_0007) as u8;

    let mut resources = Vec::new();
    for index in 0..=u32::from(lpevc) {
        let entry = base + 20 + index * 12;
        let control = read_dword(snapshot, entry).ok()?;
        let status = read_dword(snapshot, entry + 4).ok()?;
        let capability = read_dword(snapshot, entry + 8).ok()?;
        resources.push(VcResource {
            control,
            status,
            capability,
        });
    }

    Some(VcCapability {
        evc_count: (cr1 & 0x0000_0007) as u8,
        lpevc,
        reference_clock: ((cr1 >> 8) & 0x0000_0003) as u8,
        pat_entry_bits: ((cr1 >> 10) & 0x0000_0003) as u8,
        arbitration_table_position: ((cr2 >> 24) & 0x0000_00ff) as u8,
        port_control,
        port_status,
        resources,
    })
}
