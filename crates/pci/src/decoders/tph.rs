use super::read_dword;
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TphCapability {
    pub interrupt_vector_supported: bool,
    pub device_specific_supported: bool,
    pub extended_requester_supported: bool,
    pub st_table_location: u8,
    pub st_table_size: u16,
    pub st_mode_select: u8,
    pub st_table: Vec<u16>,
}

pub fn decode_tph(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<TphCapability> {
    let base = u32::from(offset);
    let capability = read_dword(snapshot, base + 4).ok()?;
    let control = read_dword(snapshot, base + 8).ok()?;

    let st_table_location = ((capability >> 9) & 0x0000_0003) as u8;
    let st_table_size = ((capability >> 16) & 0x0000_07ff) as u16;

    let mut st_table = Vec::new();
    if st_table_location == 1 {
        for index in 0..st_table_size {
            let entry_offset = base + 12 + u32::from(index) * 2;
            let bytes = snapshot.read(entry_offset, 2).ok()?;
            st_table.push(u16::from_le_bytes([bytes[0], bytes[1]]));
        }
    }

    Some(TphCapability {
        interrupt_vector_supported: capability & 0x0000_0002 != 0,
        device_specific_supported: capability & 0x0000_0004 != 0,
        extended_requester_supported: capability & 0x0000_0100 != 0,
        st_table_location,
        st_table_size,
        st_mode_select: (control & 0x0000_0007) as u8,
        st_table,
    })
}
