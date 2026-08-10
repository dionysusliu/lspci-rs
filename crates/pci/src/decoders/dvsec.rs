use super::read_dword;
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DvsecCapability {
    pub vendor_id: u16,
    pub revision: u8,
    pub dvsec_id: u16,
    pub length: u16,
    pub data: Vec<u8>,
}

pub fn decode_dvsec(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<DvsecCapability> {
    let base = u32::from(offset);
    let header = read_dword(snapshot, base + 4).ok()?;
    let dvsec_id = read_dword(snapshot, base + 8).ok()? as u16;

    let vendor_id = (header & 0x0000_ffff) as u16;
    let revision = ((header >> 16) & 0x0000_000f) as u8;
    let length = ((header >> 20) & 0x0000_0fff) as u16;

    let payload = length.saturating_sub(8);
    let data = if payload == 0 {
        Vec::new()
    } else {
        snapshot.read(base + 8, u32::from(payload)).ok()?
    };

    Some(DvsecCapability {
        vendor_id,
        revision,
        dvsec_id,
        length,
        data,
    })
}
