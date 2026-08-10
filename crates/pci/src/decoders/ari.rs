use super::read_word;
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AriCapability {
    pub capability: u16,
    pub control: u16,
}

pub fn decode_ari(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<AriCapability> {
    let base = u32::from(offset);
    let capability = read_word(snapshot, base + 4).ok()?;
    let control = read_word(snapshot, base + 6).ok()?;

    Some(AriCapability {
        capability,
        control,
    })
}
