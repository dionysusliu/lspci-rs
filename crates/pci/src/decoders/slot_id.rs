use crate::ConfigSpaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotIdCapability {
    pub slots: u8,
    pub first: bool,
    pub chassis: u8,
}

pub fn decode_slot_id(snapshot: &ConfigSpaceSnapshot, offset: u16) -> Option<SlotIdCapability> {
    let base = u32::from(offset);
    let slot = snapshot.read(base + 2, 1).ok()?[0];
    let chassis = snapshot.read(base + 3, 1).ok()?[0];

    Some(SlotIdCapability {
        slots: slot & 0x7f,
        first: slot & 0x80 != 0,
        chassis,
    })
}
