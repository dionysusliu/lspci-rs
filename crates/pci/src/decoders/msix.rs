use super::{read_dword, read_word};
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MsiXCapability {
    pub enable: bool,
    pub count: u16,
    pub masked: bool,
    pub table_bar: u8,
    pub table_offset: u32,
    pub pba_bar: u8,
    pub pba_offset: u32,
}

pub fn decode_msix(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<MsiXCapability> {
    let base = u32::from(offset);
    let control = read_word(snapshot, base + 2).ok()?;
    let table = read_dword(snapshot, base + 4).ok()?;
    let pba = read_dword(snapshot, base + 8).ok()?;

    Some(MsiXCapability {
        enable: control & 0x8000 != 0,
        count: (control & 0x07ff) + 1,
        masked: control & 0x4000 != 0,
        table_bar: (table & 0x7) as u8,
        table_offset: table & !0x7,
        pba_bar: (pba & 0x7) as u8,
        pba_offset: pba & !0x7,
    })
}
