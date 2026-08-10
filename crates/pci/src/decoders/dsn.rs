use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DsnCapability {
    pub serial: [u8; 8],
}

pub fn decode_dsn(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<DsnCapability> {
    let base = u32::from(offset);
    let bytes = snapshot.read(base + 4, 8).ok()?;
    let mut serial = [0u8; 8];
    serial.copy_from_slice(&bytes);
    Some(DsnCapability { serial })
}
