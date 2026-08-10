use super::read_word;
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcsCapability {
    pub capability: u16,
    pub control: u16,
    pub egress_vector: Vec<u8>,
}

pub fn decode_acs(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<AcsCapability> {
    let base = u32::from(offset);
    let capability = read_word(snapshot, base + 4).ok()?;
    let control = read_word(snapshot, base + 6).ok()?;

    let mut egress_vector = Vec::new();
    if capability & 0x0020 != 0 {
        let bits = ((capability >> 8) & 0x00ff) as usize;
        let bytes = bits.div_ceil(8);
        for index in 0..bytes {
            egress_vector.push(snapshot.read(base + 8 + index as u32, 1).ok()?[0]);
        }
    }

    Some(AcsCapability {
        capability,
        control,
        egress_vector,
    })
}
