use super::read_word;
use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtsCapability {
    pub invalidate_queue_depth: u8,
    pub enable: bool,
    pub page_aligned: bool,
    pub smallest_translation_unit: u8,
}

pub fn decode_ats(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<AtsCapability> {
    let base = u32::from(offset);
    let capability = read_word(snapshot, base + 4).ok()?;
    let control = read_word(snapshot, base + 6).ok()?;

    Some(AtsCapability {
        invalidate_queue_depth: (capability & 0x001f) as u8,
        enable: control & 0x8000 != 0,
        page_aligned: control & 0x1000 != 0,
        smallest_translation_unit: (control & 0x001f) as u8,
    })
}
