use super::{read_dword, read_word};
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MsiCapability {
    pub enable: bool,
    /// log2 count; vectors = 1 << n
    pub multiple_message_capable: u8,
    pub multiple_message_enable: u8,
    pub is_64_bit: bool,
    pub per_vector_masking: bool,
    pub address: u64,
    pub data: u16,
}

pub fn decode_msi(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<MsiCapability> {
    let base = u32::from(offset);
    let control = read_word(snapshot, base + 2).ok()?;

    let is_64_bit = control & 0x0080 != 0;
    let (address, data) = if is_64_bit {
        let low = read_dword(snapshot, base + 4).ok()?;
        let high = read_dword(snapshot, base + 8).ok()?;
        let data = read_word(snapshot, base + 12).ok()?;
        (u64::from(low) | (u64::from(high) << 32), data)
    } else {
        let low = read_dword(snapshot, base + 4).ok()?;
        let data = read_word(snapshot, base + 8).ok()?;
        (u64::from(low), data)
    };

    Some(MsiCapability {
        enable: control & 0x0001 != 0,
        multiple_message_capable: ((control >> 1) & 0x0007) as u8,
        multiple_message_enable: ((control >> 4) & 0x0007) as u8,
        is_64_bit,
        per_vector_masking: control & 0x0100 != 0,
        address,
        data,
    })
}
